//! Combat service.
//!
//! This module owns target acquisition, chase orders, weapon facing/setup, damage application,
//! and combat events for a tick. It depends on read-only rules and derived spatial/LOS helpers,
//! but all entity mutation for attacks flows through this service.

use std::collections::HashMap;

use crate::config;
use crate::game::ability::AbilityKind;
use crate::game::entity::{fires_while_moving, AttackPhase, EntityKind, EntityStore, Order};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::mortar::{rotate_mortar_for_fire, MortarShellStore};
use crate::game::services::dist2;
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::protocol::Event;
use rand::rngs::SmallRng;

mod acquisition;
mod chase;
mod damage;
mod events;
mod projection;
mod weapons;

#[cfg(test)]
mod tests;

use acquisition::{combat_mode, resolve_target, CombatMode};
use chase::{chase_goal_for_target, chase_path_needs_refresh};
use damage::apply_damage;
use projection::friendly_hard_blocker_between;
use weapons::{
    at_gun_can_chase, begin_idle_deployed_weapon_setup, can_fire_while_moving,
    deployed_weapon_ready_to_fire, deployed_weapon_ready_to_move, effective_attack_profile,
    mirror_weapon_to_body, moving_fire_miss_chance, relax_vehicle_weapon_toward_body,
    rotate_at_gun_for_combat, rotate_vehicle_weapon_for_combat, tick_deployed_weapon_setup,
    uses_stationary_weapon_aggro,
};

/// Extra slack (px) added to attack range checks so units don't dance at the exact boundary.
pub(super) const RANGE_SLACK: f32 = 4.0;
pub(super) const TANK_TURRET_TURN_RATE_RAD_PER_TICK: f32 = 0.070;
pub(super) const TANK_TURRET_FIRE_TOLERANCE_RAD: f32 = 0.18;
pub(super) const AT_GUN_TURN_RATE_RAD_PER_TICK: f32 = 0.035;
pub(super) const AT_GUN_FIRE_TOLERANCE_RAD: f32 = 0.12;
pub(super) const TANK_STANDOFF_BUFFER_PX: f32 = config::TILE_SIZE as f32;
pub(super) const TANK_STANDOFF_REPATH_DELTA_PX: f32 = config::TILE_SIZE as f32;

/// Combat: acquire targets for aggressive / attack-move units, let eligible idle units
/// auto-acquire enemies, and deal damage when off cooldown. Damage is applied immediately and
/// emits an `Attack` event (for tracers). Cooldowns tick down here too.
#[allow(clippy::too_many_arguments)]
pub(crate) fn combat_system(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    mortar_autocast_researched: &dyn Fn(u32) -> bool,
    _occ: &Occupancy,
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    mortar_shells: &mut MortarShellStore,
    rng: &mut SmallRng,
    events: &mut HashMap<u32, Vec<Event>>,
    tick: u32,
) {
    let los = LineOfSight::with_smoke(map, smokes);
    // Tick down cooldowns first.
    for id in entities.ids() {
        if let Some(e) = entities.get_mut(id) {
            e.tick_attack_cd();
            tick_deployed_weapon_setup(e);
        }
    }

    for id in entities.ids() {
        // Determine this attacker's combat parameters.
        let (owner, px, py, range_px, acquire_px, dmg, cd_reset, mode, is_unit, is_mortar_team) = {
            let e = match entities.get(id) {
                Some(e) => e,
                None => continue,
            };
            if e.hp == 0 || !e.can_attack() {
                continue;
            }
            // Workers executing a Gather order ignore nearby enemies: chasing aggro would
            // drag them off the resource node and stall the economy. An explicit Attack
            // order overrides Gather upstream, so this only suppresses auto-acquisition.
            if matches!(e.order(), Order::Gather(_)) {
                continue;
            }
            if e.kind == EntityKind::MortarTeam && matches!(e.order(), Order::Ability(_)) {
                continue;
            }
            let profile = effective_attack_profile(e);
            let (range_tiles, dmg, cd) = (profile.range_tiles, profile.dmg, profile.cooldown);
            let cd = if e.kind == EntityKind::Rifleman && e.charge_ticks() > 0 {
                cd.saturating_mul(config::METHAMPHETAMINES_ATTACK_COOLDOWN_NUMERATOR)
                    / config::METHAMPHETAMINES_ATTACK_COOLDOWN_DENOMINATOR
            } else {
                cd
            };
            let range_px = range_tiles as f32 * config::TILE_SIZE as f32 + e.radius() + RANGE_SLACK;
            // Aggro radius: mobile units detect and chase enemies out to their sight radius so
            // attack-move / auto-defend actually close the gap. Idle deployed weapons are the
            // exception: they hold position and only auto-acquire enemies already in weapon
            // range. Buildings never move, so they only ever engage within their firing range.
            let aggro_px = if e.is_unit() {
                if uses_stationary_weapon_aggro(e) && matches!(e.order(), Order::Idle) {
                    range_px
                } else {
                    (e.sight_tiles() as f32 * config::TILE_SIZE as f32).max(range_px)
                }
            } else {
                range_px
            };
            let mode = combat_mode(e);
            let acquire_px = if mode == CombatMode::Opportunistic {
                range_px
            } else {
                aggro_px
            };
            (
                e.owner,
                e.pos_x,
                e.pos_y,
                range_px,
                acquire_px,
                dmg,
                cd,
                mode,
                e.is_unit(),
                e.kind == EntityKind::MortarTeam,
            )
        };
        if dmg == 0 {
            continue;
        }

        // Resolve / acquire a target id based on the current order semantics.
        let target = resolve_target(
            map, entities, teams, spatial, &los, fog, smokes, id, owner, px, py, acquire_px, mode,
        );
        let Some(tid) = target else {
            // No target: clear stale combat target id for opportunistic-combat orders.
            if let Some(e) = entities.get_mut(id) {
                if matches!(
                    e.order(),
                    Order::Attack(_) | Order::AttackMove(_) | Order::Move(_) | Order::Idle
                ) {
                    e.set_target_id(None);
                    begin_idle_deployed_weapon_setup(e);
                }
                if fires_while_moving(e.kind) {
                    relax_vehicle_weapon_toward_body(e);
                }
            }
            if matches!(mode, CombatMode::Aggressive) {
                if let Some(goal) = entities.get(id).and_then(|e| e.move_intent()) {
                    let needs_resume = entities
                        .get(id)
                        .map(|e| {
                            let stale_goal = e.path_goal().is_none_or(|path_goal| {
                                (path_goal.0 - goal.0).abs() > f32::EPSILON
                                    || (path_goal.1 - goal.1).abs() > f32::EPSILON
                            });
                            let interrupted_before_arrival = e.path_is_empty()
                                && e.move_phase() != Some(crate::game::entity::MovePhase::Arrived);
                            stale_goal || interrupted_before_arrival
                        })
                        .unwrap_or(true);
                    if needs_resume {
                        if let Some(e) = entities.get_mut(id) {
                            e.set_target_id(None);
                        }
                        coordinator.request_chase_path(entities, id, goal);
                    }
                }
            }
            continue;
        };

        // Distance to chosen target.
        let (tx, ty, t_owner) = match entities.get(tid) {
            Some(t) => (t.pos_x, t.pos_y, t.owner),
            None => continue,
        };
        if !teams.is_enemy_owner(owner, t_owner) {
            continue; // never intentionally fire on non-hostile players
        }
        let dist = dist2(px, py, tx, ty).sqrt();
        let target_angle = (ty - py).atan2(tx - px);
        let terrain_clear = los.clear_between_world_points((px, py), (tx, ty));
        let friendly_blocked = terrain_clear
            && friendly_hard_blocker_between(map, entities, id, owner, (px, py), (tx, ty));
        let clear_shot = is_mortar_team || (terrain_clear && !friendly_blocked);

        if friendly_blocked && matches!(mode, CombatMode::Ordered) {
            if let Some(e) = entities.get_mut(id) {
                if fires_while_moving(e.kind) {
                    rotate_vehicle_weapon_for_combat(e, target_angle);
                } else if e.kind == EntityKind::AtTeam {
                    rotate_at_gun_for_combat(e, target_angle);
                } else if e.kind == EntityKind::MortarTeam {
                    rotate_mortar_for_fire(e, target_angle);
                } else if target_angle.is_finite() {
                    e.set_facing(target_angle);
                    mirror_weapon_to_body(e, target_angle);
                }
                e.set_target_id(Some(tid));
                e.mark_attack_phase(AttackPhase::Firing);
                e.clear_path();
            }
            continue;
        }

        if dist <= range_px && clear_shot {
            // In range: aim, stop, deploy if needed, and fire if off cooldown.
            let mut weapon_aligned = true;
            if let Some(e) = entities.get_mut(id) {
                if fires_while_moving(e.kind) {
                    weapon_aligned = rotate_vehicle_weapon_for_combat(e, target_angle);
                } else if e.kind == EntityKind::AtTeam {
                    weapon_aligned = rotate_at_gun_for_combat(e, target_angle);
                } else if e.kind == EntityKind::MortarTeam {
                    weapon_aligned = rotate_mortar_for_fire(e, target_angle);
                } else if target_angle.is_finite() {
                    e.set_facing(target_angle);
                    mirror_weapon_to_body(e, target_angle);
                }
                e.set_target_id(Some(tid));
                e.mark_attack_phase(AttackPhase::Firing);
                // Most units hold position while firing. Vehicle weapons can track independently,
                // and charging riflemen accept lower accuracy to keep advancing.
                if !can_fire_while_moving(e) {
                    e.clear_path();
                }
            }
            if !weapon_aligned {
                continue;
            }
            if !deployed_weapon_ready_to_fire(entities, id) {
                continue;
            }
            let ready = matches!(entities.get(id), Some(e) if e.attack_cd() == 0);
            if ready {
                if matches!(
                    entities.get(id).map(|e| e.kind),
                    Some(EntityKind::MortarTeam)
                ) {
                    if !matches!(
                        entities
                            .get(id)
                            .and_then(|e| e.autocast_enabled(AbilityKind::MortarFire)),
                        Some(true)
                    ) || !mortar_autocast_researched(owner)
                    {
                        continue;
                    }
                    let (mx, my) = mortar_aim_point(entities, tid, tick);
                    if mortar_autocast_would_hit_same_team_entity(entities, teams, owner, mx, my) {
                        continue;
                    }
                    mortar_shells.schedule(events, fog, owner, id, px, py, mx, my, tick, true);
                    if let Some(e) = entities.get_mut(id) {
                        e.set_attack_cd(cd_reset);
                    }
                    continue;
                }
                let extra_miss_chance =
                    entities.get(id).map(moving_fire_miss_chance).unwrap_or(0.0);
                apply_damage(
                    map,
                    entities,
                    teams,
                    events,
                    fog,
                    smokes,
                    rng,
                    id,
                    tid,
                    dmg,
                    owner,
                    px,
                    py,
                    tx,
                    ty,
                    range_px,
                    extra_miss_chance,
                    tick,
                );
                if let Some(e) = entities.get_mut(id) {
                    e.set_attack_cd(cd_reset);
                }
            }
        } else if is_unit {
            // Out of weapon range but within aggro: chase. Tanks route to a standoff point
            // inside firing range; other units still route toward the target center.
            let chase_goal =
                chase_goal_for_target(map, entities, id, (px, py), (tx, ty), range_px, dist);
            let want_repath = entities
                .get(id)
                .map(|e| chase_path_needs_refresh(e, chase_goal))
                .unwrap_or(false);
            let mut can_chase = true;
            if let Some(e) = entities.get_mut(id) {
                if fires_while_moving(e.kind) {
                    rotate_vehicle_weapon_for_combat(e, target_angle);
                } else if e.kind == EntityKind::AtTeam {
                    rotate_at_gun_for_combat(e, target_angle);
                } else if e.kind == EntityKind::MortarTeam {
                    rotate_mortar_for_fire(e, target_angle);
                } else if target_angle.is_finite() {
                    mirror_weapon_to_body(e, e.facing());
                }
                e.set_target_id(Some(tid));
                e.mark_attack_phase(AttackPhase::Chasing);
                can_chase = at_gun_can_chase(e);
            }
            if !can_chase {
                continue;
            }
            if !deployed_weapon_ready_to_move(entities, id) {
                continue;
            }
            if want_repath {
                coordinator.request_chase_path(entities, id, chase_goal);
            }
        }
    }
}

fn mortar_aim_point(entities: &EntityStore, target: u32, tick: u32) -> (f32, f32) {
    let Some(t) = entities.get(target) else {
        return (0.0, 0.0);
    };
    let mut x = t.pos_x;
    let mut y = t.pos_y;
    if let Some((gx, gy)) = t.move_intent() {
        let dx = gx - t.pos_x;
        let dy = gy - t.pos_y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > f32::EPSILON && dist.is_finite() {
            let speed = config::unit_stats(t.kind)
                .map(|stats| stats.speed)
                .unwrap_or(0.0);
            let lead = (speed * config::MORTAR_SHELL_DELAY_TICKS as f32).min(dist);
            x += dx / dist * lead;
            y += dy / dist * lead;
        }
    }
    let error = config::MORTAR_AUTOFIRE_ERROR_TILES * config::TILE_SIZE as f32;
    if error > 0.0 {
        let angle = ((target ^ tick) as f32 * 1.618_034).rem_euclid(std::f32::consts::TAU);
        let radius = ((((target.wrapping_mul(1103515245).wrapping_add(tick)) >> 8) & 1023) as f32
            / 1023.0)
            * error;
        x += angle.cos() * radius;
        y += angle.sin() * radius;
    }
    (x, y)
}

fn mortar_autocast_would_hit_same_team_entity(
    entities: &EntityStore,
    teams: &TeamRelations,
    owner: u32,
    x: f32,
    y: f32,
) -> bool {
    let outer_radius = config::MORTAR_OUTER_RADIUS_TILES * config::TILE_SIZE as f32;
    let outer2 = outer_radius * outer_radius;
    entities.ids().into_iter().any(|id| {
        entities.get(id).is_some_and(|e| {
            teams.same_team_or_same_owner(owner, e.owner)
                && e.hp > 0
                && !e.is_node()
                && dist2(x, y, e.pos_x, e.pos_y) <= outer2
        })
    })
}
