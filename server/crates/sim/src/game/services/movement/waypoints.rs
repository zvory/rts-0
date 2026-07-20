use std::collections::HashMap;

use crate::config;
use crate::game::ability_runtime::AbilityRuntime;
use crate::game::entity::{
    uses_car_movement_semantics, uses_oriented_vehicle_body, uses_pivot_vehicle_movement,
    AttackPhase, Entity, EntityKind, EntityStore, MovePhase, Order, PanzerfaustState, WeaponSetup,
};
use crate::game::map::Map;
use crate::game::services::geometry::{
    tile_rect, unit_body_for_entity, unit_body_intersects_rect, unit_body_with_facing,
};
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::standability as static_standability;
use crate::game::upgrade::UpgradeKind;
use crate::game::PlayerState;
use crate::protocol::Event;
use crate::rules::terrain::{movement_speed_multiplier, TerrainKind};

use super::pivot_drive::{
    angle_delta, distance_between, normalize_angle, pivot_drive_intent, pivot_drive_speed_scale,
    rotate_toward, vehicle_body_turn_rate, vehicle_oil_starves_movement,
    vehicle_traffic_adjustment,
};
use super::scout_car::{plan_scout_car_motion, route_accepts_waypoint};
use super::standability::{
    footing_profile, requires_weapon_setup, unit_static_standable, FootingProfile,
};
use super::steering::{inject_sidestep, steered_candidate, steering_path_dir};
use super::{ARRIVE_EPS, MAX_UNIT_BOUNDING_RADIUS_PX};

const TANK_ROTATION_UNJAM_EPS: f32 = 1.0e-4;
const PIVOT_ROTATION_ASSIST_STEP_SCALE: f32 = 0.25;
const PIVOT_ROTATION_ASSIST_MAX_STEPS: u32 = 4;
const SCOUT_CAR_RECOVERY_SEARCH_STEP_PX: f32 = config::TILE_SIZE as f32 * 0.5;

fn panzerfaust_movement_locked(e: &Entity) -> bool {
    matches!(
        e.combat.as_ref().and_then(|combat| combat.panzerfaust),
        Some(PanzerfaustState::Windup { .. })
    )
}

/// Advance moving units along waypoint paths, preserving passable landings and Move arrival.
/// Unit overlap is handled later by collision resolution.
#[allow(clippy::too_many_arguments)]
pub(super) fn advance_moving_units(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
    spatial: &SpatialIndex,
    tick: u32,
    events: &mut HashMap<u32, Vec<Event>>,
    ability_runtime: &AbilityRuntime,
) {
    for id in entities.ids() {
        let (
            kind,
            owner,
            breakthrough_ticks,
            recent_smoke_ticks,
            has_meth,
            mut x,
            mut y,
            can_local_steer,
            movement_target,
        ) = {
            let e = match entities.get(id) {
                Some(e) if e.is_unit() && !e.path_is_empty() => e,
                _ => continue,
            };
            if panzerfaust_movement_locked(e) {
                continue;
            }
            if requires_weapon_setup(e.kind) && !matches!(e.weapon_setup(), WeaponSetup::Packed) {
                continue;
            }
            let has_meth = players
                .iter()
                .any(|p| p.id == e.owner && p.has_upgrade(UpgradeKind::Methamphetamines));
            (
                e.kind,
                e.owner,
                e.breakthrough_ticks(),
                e.recent_smoke_ticks(),
                has_meth,
                e.pos_x,
                e.pos_y,
                !uses_oriented_vehicle_body(e.kind)
                    && matches!(e.order(), Order::Move(_) | Order::Attack(_))
                    && footing_profile(e) != FootingProfile::Ghost,
                e.next_waypoint(),
            )
        };
        let breakthrough_multiplier = breakthrough_speed_multiplier(
            entities,
            spatial,
            owner,
            x,
            y,
            breakthrough_ticks,
            recent_smoke_ticks,
        );
        let speed_multiplier = if crate::rules::is_rifle_infantry(kind) && has_meth {
            config::METHAMPHETAMINES_SPEED_MULTIPLIER
        } else if breakthrough_multiplier > 1.0 {
            breakthrough_multiplier
        } else if kind == EntityKind::MachineGunner && has_meth {
            config::METHAMPHETAMINES_SPEED_MULTIPLIER
        } else {
            1.0
        };
        let terrain_speed_multiplier = {
            let (tx, ty) = map.tile_of(x, y);
            TerrainKind::from_map_code(map.terrain_at(tx, ty))
                .map(|terrain| movement_speed_multiplier(kind, terrain))
                .unwrap_or(1.0)
        };
        let mut speed = config::unit_stats(kind)
            .map(|s| s.speed * speed_multiplier * terrain_speed_multiplier)
            .unwrap_or(0.0);
        if let Some((wx, wy)) = movement_target {
            speed *= ability_runtime.magic_anchor_movement_multiplier(x, y, (wx - x, wy - y), tick);
        }
        if speed <= 0.0 {
            continue;
        }

        let uses_vehicle_movement = uses_oriented_vehicle_body(kind);
        let is_pivot_vehicle = uses_pivot_vehicle_movement(kind);
        let is_car = uses_car_movement_semantics(kind);
        let vehicle_oil_cost_per_px = match kind {
            EntityKind::Tank => Some(config::TANK_OIL_COST_PER_PX),
            EntityKind::ScoutCar | EntityKind::CommandCar => {
                Some(config::SCOUT_CAR_OIL_COST_PER_PX)
            }
            _ => None,
        };
        // Oil-starved vehicles pause before retrying instead of lurching on sparse income ticks.
        if vehicle_oil_cost_per_px.is_some()
            && vehicle_oil_starves_movement(entities, players, events, id)
        {
            continue;
        }
        let orig_x = x;
        let orig_y = y;
        let mut budget = speed;
        let mut new_facing = None;
        let original_facing = entities.get(id).map(|e| e.facing()).unwrap_or(0.0);
        let mut body_facing = original_facing;
        let mut scout_car_reverse_waypoint = None;
        let mut static_blocked_this_tick = false;
        if is_pivot_vehicle {
            if let Some(e) = entities.get(id) {
                if let Some(mut intent) = pivot_drive_intent(map, occ, e, x, y) {
                    let traffic = vehicle_traffic_adjustment(
                        entities,
                        spatial,
                        id,
                        kind,
                        x,
                        y,
                        intent.traffic_facing,
                    );
                    intent.desired_facing =
                        normalize_angle(intent.desired_facing + traffic.turn_bias);
                    if intent.desired_facing.is_finite() {
                        let rotated = rotate_toward(
                            e.facing(),
                            intent.desired_facing,
                            vehicle_body_turn_rate(kind),
                        );
                        if rotated.is_finite() {
                            let error = angle_delta(rotated, intent.desired_facing).abs();
                            budget *= pivot_drive_speed_scale(error);
                            budget *= traffic.throttle_scale;
                            new_facing = Some(rotated);
                            body_facing = rotated;
                        }
                    }
                }
            }
        } else if is_car {
            if let Some(snapshot) = entities.get(id).cloned() {
                if let Some(plan) = plan_scout_car_motion(
                    map,
                    occ,
                    entities,
                    spatial,
                    id,
                    &snapshot,
                    (x, y),
                    budget,
                ) {
                    x = plan.pos.0;
                    y = plan.pos.1;
                    static_blocked_this_tick = plan.static_blocked;
                    scout_car_reverse_waypoint = plan.reverse_waypoint;
                    if let Some(facing) = plan.facing {
                        new_facing = Some(facing);
                        body_facing = facing;
                    }
                    if plan.pop_waypoints > 0 {
                        if let Some(e) = entities.get_mut(id) {
                            for _ in 0..plan.pop_waypoints {
                                e.pop_waypoint();
                                e.mark_move_phase(MovePhase::Moving);
                            }
                        }
                    }
                }
            }
        }
        // Consume waypoints (stored reversed, next = last element) within this tick's budget.
        if !is_car {
            loop {
                let (next, path_len, next_next) = {
                    let Some(e) = entities.get(id) else { break };
                    let path_len = e.movement.as_ref().map(|m| m.path.len()).unwrap_or(0);
                    // next_next: the waypoint after the current one (path is reversed, so index len-2).
                    let next_next = e.movement.as_ref().and_then(|m| {
                        if m.path.len() >= 2 {
                            m.path.get(m.path.len() - 2).copied()
                        } else {
                            None
                        }
                    });
                    (e.next_waypoint(), path_len, next_next)
                };
                let Some((wx, wy)) = next else { break };
                let dx = wx - x;
                let dy = wy - y;
                let dist = (dx * dx + dy * dy).sqrt();

                if path_len > 1 {
                    // Intermediate waypoint: follow the route corridor when this unit's static
                    // swept body can reach the next segment. Vehicles also keep their
                    // facing-specific guard so reverse/recovery waypoints are physically reached.
                    let route_accepts = entities.get(id).is_some_and(|e| {
                        e.kind != EntityKind::Worker
                            && route_accepts_waypoint(map, occ, e, (x, y), (wx, wy), next_next)
                    });
                    let legacy_infantry_accepts = if !uses_vehicle_movement && !route_accepts {
                        let radius_hit = dist <= config::ARRIVE_RADIUS_INTERMEDIATE_PX;
                        let passed = next_next.is_some_and(|(nnx, nny)| {
                            // Positive projection of (pos - waypoint) onto (next_next - waypoint) means
                            // the unit is on the far side of the waypoint relative to where it came from.
                            (x - wx) * (nnx - wx) + (y - wy) * (nny - wy) > 0.0
                        });
                        radius_hit || passed
                    } else {
                        false
                    };
                    let accepts_waypoint = route_accepts || legacy_infantry_accepts;
                    if accepts_waypoint {
                        if let Some(e) = entities.get_mut(id) {
                            e.pop_waypoint();
                            e.mark_move_phase(MovePhase::Moving);
                        }
                        // No position snap — steer toward the new next waypoint from current position.
                        continue;
                    }
                } else {
                    // Final waypoint: require exact arrival.
                    if dist <= ARRIVE_EPS {
                        if let Some(e) = entities.get_mut(id) {
                            e.pop_waypoint();
                            e.mark_move_phase(MovePhase::Moving);
                        }
                        x = wx;
                        y = wy;
                        continue;
                    }
                }

                if !uses_vehicle_movement {
                    let facing = dy.atan2(dx);
                    if facing.is_finite() {
                        new_facing = Some(facing);
                    }
                }
                let can_reach_waypoint = if is_car {
                    path_len == 1 && dist <= budget
                } else {
                    dist <= budget
                };
                if can_reach_waypoint {
                    // We can reach this waypoint this tick.
                    if !unit_static_standable(occ, map, kind, wx, wy, body_facing) {
                        static_blocked_this_tick = true;
                        break;
                    }
                    x = wx;
                    y = wy;
                    budget -= dist;
                    if let Some(e) = entities.get_mut(id) {
                        e.pop_waypoint();
                        e.mark_move_phase(MovePhase::Moving);
                    }
                } else {
                    // Partial step toward the waypoint.
                    let path_dir = (dx / dist, dy / dist);
                    let step_dir = path_dir;
                    let direct_nx = x + step_dir.0 * budget;
                    let direct_ny = y + step_dir.1 * budget;
                    let steered = if can_local_steer {
                        let steering_path_dir = entities
                            .get(id)
                            .map(|e| steering_path_dir(e, x, y, path_dir))
                            .unwrap_or(path_dir);
                        steered_candidate(
                            entities,
                            spatial,
                            occ,
                            map,
                            id,
                            kind,
                            x,
                            y,
                            steering_path_dir,
                            budget,
                        )
                    } else {
                        None
                    };
                    let (nx, ny) = if let Some((sx, sy)) = steered {
                        (sx, sy)
                    } else {
                        (direct_nx, direct_ny)
                    };
                    // Clamp landing to a body-legal static position.
                    if unit_static_standable(occ, map, kind, nx, ny, body_facing) {
                        x = nx;
                        y = ny;
                    } else {
                        // The planned step was rejected by static geometry. Keep that signal even
                        // when an axis-only fallback can wall-slide this tick: shallow approach
                        // angles can otherwise make microscopic sideways progress forever and
                        // reset the stale-route debounce without ever clearing the obstacle.
                        static_blocked_this_tick = true;
                        // Wall-slide: try each axis independently so a unit pressed against a
                        // building edge can slide along it rather than freezing. Guard each axis
                        // against zero movement (dy=0 ⟹ y-only slide is a no-op).
                        let slide_x = dx.abs() > 1e-4
                            && unit_static_standable(occ, map, kind, nx, y, body_facing);
                        let slide_y = dy.abs() > 1e-4
                            && unit_static_standable(occ, map, kind, x, ny, body_facing);
                        if slide_x {
                            x = nx;
                        } else if slide_y {
                            y = ny;
                        }
                    }
                    break;
                }
            }
        }

        if is_car && distance_between((orig_x, orig_y), (x, y)) <= 0.01 {
            // This is still a path-following approximation, not tire physics. The important
            // car rule is that steering comes from translation, so blocked scout cars do not
            // pivot their oriented body in place like tanks.
            new_facing = None;
        }

        if uses_vehicle_movement {
            if let Some(facing) = new_facing {
                if !unit_static_standable(occ, map, kind, x, y, facing) {
                    if unit_static_standable(occ, map, kind, x, y, original_facing) {
                        let route_target = entities
                            .get(id)
                            .and_then(|e| e.next_waypoint().or_else(|| e.path_goal()));
                        if is_pivot_vehicle {
                            if let Some((ux, uy)) = pivot_vehicle_rotation_unjam_candidate(
                                occ,
                                map,
                                kind,
                                PivotVehicleRotationUnjamProbe {
                                    pos: (x, y),
                                    original_facing,
                                    blocked_facing: facing,
                                    step_px: speed,
                                    route_target,
                                },
                            ) {
                                x = ux;
                                y = uy;
                                static_blocked_this_tick = false;
                            } else {
                                new_facing = None;
                            }
                        } else {
                            new_facing = None;
                        }
                    } else {
                        x = orig_x;
                        y = orig_y;
                        new_facing = None;
                        static_blocked_this_tick = true;
                    }
                }
            }
        }

        // Compute neighbor repulsion before taking the mutable borrow.
        let repulsion_dir: (f32, f32) = {
            let unit_radius = entities
                .get(id)
                .and_then(|e| unit_body_for_entity(e).map(|body| body.bounding_radius()))
                .unwrap_or(9.0);
            let repulsion_range = unit_radius * 2.0 + MAX_UNIT_BOUNDING_RADIUS_PX;
            let mut rx = 0.0_f32;
            let mut ry = 0.0_f32;
            for bid in spatial.ids_in_circle_bbox(x, y, repulsion_range) {
                if bid == id {
                    continue;
                }
                if let Some(nb) = entities.get(bid) {
                    let dx = x - nb.pos_x;
                    let dy = y - nb.pos_y;
                    let d = (dx * dx + dy * dy).sqrt();
                    if d > 1e-4 {
                        rx += dx / d;
                        ry += dy / d;
                    }
                }
            }
            let rlen = (rx * rx + ry * ry).sqrt();
            if rlen > 1e-4 {
                (rx / rlen, ry / rlen)
            } else {
                (0.0, 0.0)
            }
        };

        if let Some(e) = entities.get_mut(id) {
            e.pos_x = x.clamp(0.0, map.world_size_px() - 0.01);
            e.pos_y = y.clamp(0.0, map.world_size_px() - 0.01);
            e.set_movement_delta(e.pos_x - orig_x, e.pos_y - orig_y);
            let moved_by_path = distance_between((orig_x, orig_y), (e.pos_x, e.pos_y)) > 0.01;
            let rotated_by_path = new_facing
                .map(|facing| angle_delta(original_facing, facing).abs() > 1.0e-4)
                .unwrap_or(false);
            if kind == EntityKind::Tank && (moved_by_path || rotated_by_path) {
                if let Some(combat) = e.combat.as_mut() {
                    combat.tank_stationary_range_ticks = 0;
                    combat.tank_stationary_range_reset_this_tick = true;
                }
            }
            if let Some(f) = new_facing {
                e.set_facing(f);
            }
            if is_car {
                let active_reverse_waypoint = scout_car_reverse_waypoint.filter(|wp| {
                    e.next_waypoint()
                        .is_some_and(|next| distance_between(next, *wp) <= ARRIVE_EPS)
                });
                if let Some(m) = e.movement.as_mut() {
                    m.scout_car_reverse_waypoint = active_reverse_waypoint;
                }
            }
            // A plain Move with an empty path has arrived → go idle so normal auto-acquire
            // resumes after the destination is reached.
            if e.path_is_empty() {
                e.mark_move_phase(MovePhase::Arrived);
                if let Some(m) = e.movement.as_mut() {
                    m.static_blocked_ticks = 0;
                }
                if matches!(e.order(), Order::Move(_)) {
                    e.set_order(Order::Idle);
                }
            } else if matches!(e.move_phase(), Some(MovePhase::Moving))
                || matches!(e.order(), Order::Attack(_))
            {
                // Decrement local recovery cooldowns each tick.
                if let Some(m) = e.movement.as_mut() {
                    m.sidestep_cooldown = m.sidestep_cooldown.saturating_sub(1);
                    m.scout_car_recovery_cooldown = m.scout_car_recovery_cooldown.saturating_sub(1);
                }

                if static_blocked_this_tick {
                    if let Some(m) = e.movement.as_mut() {
                        m.static_blocked_ticks = m.static_blocked_ticks.saturating_add(1);
                    }
                } else if let Some(m) = e.movement.as_mut() {
                    m.static_blocked_ticks = 0;
                }

                let static_blocked_ticks = e
                    .movement
                    .as_ref()
                    .map(|m| m.static_blocked_ticks)
                    .unwrap_or(0);
                if static_blocked_ticks >= config::STATIC_BLOCKED_REPATH_TICKS
                    && matches!(
                        e.order(),
                        Order::Move(_)
                            | Order::AttackMove(_)
                            | Order::Ability(_)
                            | Order::Attack(_)
                    )
                {
                    e.set_path(Vec::new());
                    if matches!(e.order(), Order::Attack(_)) {
                        e.mark_attack_phase(AttackPhase::Pursuing);
                    } else {
                        e.mark_move_phase(MovePhase::AwaitingPath);
                    }
                    let (px, py) = (e.pos_x, e.pos_y);
                    e.reset_stuck(px, py);
                    continue;
                }

                // Tolerant arrival: unit has a path but may be making no progress.
                let (lx, ly) = e
                    .movement
                    .as_ref()
                    .map(|m| m.last_progress_pos)
                    .unwrap_or((x, y));
                let dx = x - lx;
                let dy = y - ly;
                let moved = (dx * dx + dy * dy).sqrt();
                if moved < config::STUCK_EPS_PX {
                    if let Some(m) = e.movement.as_mut() {
                        m.stuck_ticks = m.stuck_ticks.saturating_add(1);
                    }
                } else if let Some(m) = e.movement.as_mut() {
                    m.stuck_ticks = 0;
                    m.last_progress_pos = (x, y);
                }
                let stuck_ticks = e.movement.as_ref().map(|m| m.stuck_ticks).unwrap_or(0);
                // Tolerant arrival: stuck and near goal.
                if stuck_ticks >= config::STUCK_ARRIVAL_TICKS {
                    if let Some((gx, gy)) = e.path_goal() {
                        let dx = x - gx;
                        let dy = y - gy;
                        let dist_to_goal = (dx * dx + dy * dy).sqrt();
                        if dist_to_goal <= config::TOLERANT_ARRIVAL_RADIUS_PX {
                            e.clear_path();
                            e.mark_move_phase(MovePhase::Arrived);
                            if let Some(m) = e.movement.as_mut() {
                                m.stuck_ticks = 0;
                            }
                            if matches!(e.order(), Order::Move(_)) {
                                e.set_order(Order::Idle);
                            }
                        }
                    }
                }
                if is_car
                    && stuck_ticks >= config::SCOUT_CAR_STUCK_RECOVERY_TRIGGER_TICKS
                    && matches!(
                        e.order(),
                        Order::Move(_)
                            | Order::AttackMove(_)
                            | Order::Ability(_)
                            | Order::Attack(_)
                    )
                {
                    inject_scout_car_reverse_recovery(e, map, occ);
                }
                // Sidestep: stuck mid-path (far from goal), cooldown elapsed,
                // only for Move/AttackMove orders.
                // Stagger trigger per unit so clustered units don't all sidestep at once.
                let trigger_threshold = config::SIDESTEP_TRIGGER_TICKS + (id % 8) as u16;
                if !uses_oriented_vehicle_body(kind)
                    && stuck_ticks >= trigger_threshold
                    && static_blocked_ticks == 0
                    && matches!(
                        e.order(),
                        Order::Move(_) | Order::AttackMove(_) | Order::Attack(_)
                    )
                {
                    let far_from_goal = e.path_goal().is_some_and(|(gx, gy)| {
                        let dx = x - gx;
                        let dy = y - gy;
                        (dx * dx + dy * dy).sqrt() > config::TOLERANT_ARRIVAL_RADIUS_PX
                    });
                    let sidestep_cooldown = e
                        .movement
                        .as_ref()
                        .map(|m| m.sidestep_cooldown)
                        .unwrap_or(1);
                    if far_from_goal && sidestep_cooldown == 0 {
                        inject_sidestep(e, id, x, y, map, occ, repulsion_dir, tick);
                    }
                }
            }
        }

        // Experimental vehicle fuel: charge oil for the distance actually moved this tick.
        if let Some(oil_cost_per_px) = vehicle_oil_cost_per_px {
            let (final_x, final_y, owner) = match entities.get(id) {
                Some(e) => (e.pos_x, e.pos_y, e.owner),
                None => continue,
            };
            let dx = final_x - orig_x;
            let dy = final_y - orig_y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > 0.0 {
                let cost = dist * oil_cost_per_px;
                if let Some(e) = entities.get_mut(id) {
                    if let Some(m) = e.movement.as_mut() {
                        m.lifetime_oil_used += cost;
                        m.oil_debt += cost;
                        if m.oil_debt >= 1.0 {
                            let whole = m.oil_debt.floor() as u32;
                            m.oil_debt -= whole as f32;
                            if let Some(p) = players.iter_mut().find(|p| p.id == owner) {
                                let charged = whole.min(p.oil);
                                p.spend_resources(0, charged);
                                // If we couldn't pay full amount, drop the unpaid remainder
                                // so debt does not accumulate while the player has no oil.
                                if charged < whole {
                                    m.oil_debt = 0.0;
                                }
                            } else {
                                m.oil_debt = 0.0;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Return the strongest current Command Car speed effect for an owned unit.
///
/// A timed Breakthrough status preserves its existing full-speed/smoke behavior. Otherwise an
/// owned, completed Command Car continuously supplies the smaller passive aura while in range.
fn breakthrough_speed_multiplier(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    owner: u32,
    x: f32,
    y: f32,
    breakthrough_ticks: u16,
    recent_smoke_ticks: u16,
) -> f32 {
    if breakthrough_ticks > 0 {
        return if recent_smoke_ticks > 0 {
            config::BREAKTHROUGH_SMOKE_SPEED_MULTIPLIER
        } else {
            config::BREAKTHROUGH_BASE_SPEED_MULTIPLIER
        };
    }

    let radius_px = config::BREAKTHROUGH_RADIUS_TILES * config::TILE_SIZE as f32;
    let radius2 = radius_px * radius_px;
    let has_owned_command_car = spatial
        .ids_in_circle_bbox(x, y, radius_px)
        .any(|candidate| {
            let Some(car) = entities.get(candidate) else {
                return false;
            };
            if car.kind != EntityKind::CommandCar
                || car.owner != owner
                || car.hp == 0
                || car.under_construction()
            {
                return false;
            }
            let dx = car.pos_x - x;
            let dy = car.pos_y - y;
            dx * dx + dy * dy <= radius2
        });
    if has_owned_command_car {
        config::COMMAND_CAR_AURA_SPEED_MULTIPLIER
    } else {
        1.0
    }
}

fn inject_scout_car_reverse_recovery(e: &mut Entity, map: &Map, occ: &Occupancy) {
    if !matches!(e.kind, EntityKind::ScoutCar | EntityKind::CommandCar) {
        return;
    }
    let Some(movement) = e.movement.as_ref() else {
        return;
    };
    if movement.scout_car_recovery_cooldown > 0 || movement.path.is_empty() {
        return;
    }
    let Some((gx, gy)) = movement.path_goal else {
        return;
    };
    let dx_goal = e.pos_x - gx;
    let dy_goal = e.pos_y - gy;
    if (dx_goal * dx_goal + dy_goal * dy_goal).sqrt() <= config::TOLERANT_ARRIVAL_RADIUS_PX {
        return;
    }
    let facing = e.facing();
    if !e.pos_x.is_finite() || !e.pos_y.is_finite() || !facing.is_finite() {
        return;
    }
    if !unit_static_standable(occ, map, e.kind, e.pos_x, e.pos_y, facing) {
        return;
    }

    let forward = (facing.cos(), facing.sin());
    if !forward.0.is_finite() || !forward.1.is_finite() {
        return;
    }

    let min_distance = (config::SCOUT_CAR_BODY_LENGTH_PX
        + config::VEHICLE_WAYPOINT_ACCEPTANCE_RADIUS_PX)
        .min(config::SCOUT_CAR_REVERSE_RECOVERY_DISTANCE_PX);
    let max_distance = config::SCOUT_CAR_REVERSE_RECOVERY_DISTANCE_PX.max(min_distance);
    let mut distance = min_distance;
    while distance <= max_distance + 0.001 {
        let candidate = (
            e.pos_x - forward.0 * distance,
            e.pos_y - forward.1 * distance,
        );
        if recovery_candidate_is_legal(e, map, occ, candidate) {
            e.push_waypoint(candidate);
            if let Some(m) = e.movement.as_mut() {
                m.stuck_ticks = 0;
                m.last_progress_pos = (e.pos_x, e.pos_y);
                m.static_blocked_ticks = 0;
                m.scout_car_reverse_waypoint = Some(candidate);
                m.scout_car_recovery_cooldown = config::SCOUT_CAR_RECOVERY_COOLDOWN_TICKS;
            }
            return;
        }
        distance += SCOUT_CAR_RECOVERY_SEARCH_STEP_PX;
    }
}

fn recovery_candidate_is_legal(
    e: &Entity,
    map: &Map,
    occ: &Occupancy,
    candidate: (f32, f32),
) -> bool {
    if !candidate.0.is_finite() || !candidate.1.is_finite() {
        return false;
    }
    let world_size = map.world_size_px();
    if candidate.0 < 0.0
        || candidate.1 < 0.0
        || candidate.0 >= world_size
        || candidate.1 >= world_size
    {
        return false;
    }
    if e.next_waypoint()
        .is_some_and(|wp| distance_between(wp, candidate) <= ARRIVE_EPS)
    {
        return false;
    }
    unit_static_standable(occ, map, e.kind, candidate.0, candidate.1, e.facing())
        && static_standability::unit_static_segment_standable(
            map,
            occ,
            e.kind,
            (e.pos_x, e.pos_y),
            candidate,
        )
}

#[derive(Clone, Copy)]
struct PivotVehicleRotationUnjamProbe {
    pos: (f32, f32),
    original_facing: f32,
    blocked_facing: f32,
    step_px: f32,
    route_target: Option<(f32, f32)>,
}

fn pivot_vehicle_rotation_unjam_candidate(
    occ: &Occupancy,
    map: &Map,
    kind: EntityKind,
    probe: PivotVehicleRotationUnjamProbe,
) -> Option<(f32, f32)> {
    if !probe.original_facing.is_finite()
        || !probe.blocked_facing.is_finite()
        || !probe.step_px.is_finite()
        || probe.step_px <= 0.0
    {
        return None;
    }

    let forward = (probe.original_facing.cos(), probe.original_facing.sin());
    if !forward.0.is_finite() || !forward.1.is_finite() {
        return None;
    }
    let blocked_by_building =
        rotation_blocked_by_building(occ, kind, probe.pos, probe.blocked_facing);
    let route_dir = probe
        .route_target
        .and_then(|target| normalized_dir(probe.pos, target));

    let mut best = None;
    let mut best_score = f32::INFINITY;

    if blocked_by_building {
        if let Some(dir) = route_dir {
            for step in 1..=PIVOT_ROTATION_ASSIST_MAX_STEPS {
                let distance = probe.step_px * PIVOT_ROTATION_ASSIST_STEP_SCALE * step as f32;
                let candidate = (
                    probe.pos.0 + dir.0 * distance,
                    probe.pos.1 + dir.1 * distance,
                );
                if !pivot_rotation_assist_candidate_legal(occ, map, kind, probe, candidate) {
                    continue;
                }

                let route_score = probe
                    .route_target
                    .map(|target| distance_between(candidate, target))
                    .unwrap_or(0.0);
                let score = route_score + distance * 0.2;
                if score + TANK_ROTATION_UNJAM_EPS < best_score {
                    best = Some(candidate);
                    best_score = score;
                }
            }
        }
    }

    for sign in [1.0_f32, -1.0] {
        let candidate = (
            probe.pos.0 + forward.0 * probe.step_px * sign,
            probe.pos.1 + forward.1 * probe.step_px * sign,
        );
        if !pivot_rotation_assist_candidate_legal(occ, map, kind, probe, candidate) {
            continue;
        }

        let score = probe
            .route_target
            .map(|target| distance_between(candidate, target))
            .unwrap_or(if sign > 0.0 { 0.0 } else { 1.0 });
        if score + TANK_ROTATION_UNJAM_EPS < best_score {
            best = Some(candidate);
            best_score = score;
        }
    }

    best
}

fn normalized_dir(from: (f32, f32), to: (f32, f32)) -> Option<(f32, f32)> {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let dist = (dx * dx + dy * dy).sqrt();
    (dist.is_finite() && dist > 1.0e-4).then_some((dx / dist, dy / dist))
}

fn pivot_rotation_assist_candidate_legal(
    occ: &Occupancy,
    map: &Map,
    kind: EntityKind,
    probe: PivotVehicleRotationUnjamProbe,
    candidate: (f32, f32),
) -> bool {
    if !candidate.0.is_finite() || !candidate.1.is_finite() {
        return false;
    }

    unit_static_standable(
        occ,
        map,
        kind,
        candidate.0,
        candidate.1,
        probe.original_facing,
    ) && unit_static_standable(
        occ,
        map,
        kind,
        candidate.0,
        candidate.1,
        probe.blocked_facing,
    ) && pivot_rotation_assist_segment_legal(
        occ,
        map,
        kind,
        probe.pos,
        candidate,
        probe.original_facing,
    )
}

fn rotation_blocked_by_building(
    occ: &Occupancy,
    kind: EntityKind,
    pos: (f32, f32),
    facing: f32,
) -> bool {
    let Some(body) = unit_body_with_facing(kind, pos.0, pos.1, facing) else {
        return false;
    };
    let aabb = body.aabb();
    let ts = config::TILE_SIZE as f32;
    let min_tx = (aabb.min_x / ts).floor() as i32;
    let max_tx = (aabb.max_x / ts).floor() as i32;
    let min_ty = (aabb.min_y / ts).floor() as i32;
    let max_ty = (aabb.max_y / ts).floor() as i32;

    for ty in min_ty..=max_ty {
        for tx in min_tx..=max_tx {
            if !occ.building_blocked_at_tile(tx, ty) {
                continue;
            }
            if unit_body_intersects_rect(body, tile_rect(tx, ty)) {
                return true;
            }
        }
    }

    false
}

fn pivot_rotation_assist_segment_legal(
    occ: &Occupancy,
    map: &Map,
    kind: EntityKind,
    from: (f32, f32),
    to: (f32, f32),
    facing: f32,
) -> bool {
    let distance = distance_between(from, to);
    if !distance.is_finite() {
        return false;
    }
    let steps = (distance / probe_step_px()).ceil().max(1.0) as u32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = from.0 + (to.0 - from.0) * t;
        let y = from.1 + (to.1 - from.1) * t;
        if !unit_static_standable(occ, map, kind, x, y, facing) {
            return false;
        }
    }
    true
}

fn probe_step_px() -> f32 {
    config::TILE_SIZE as f32 * 0.125
}
