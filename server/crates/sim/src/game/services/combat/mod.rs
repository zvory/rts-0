//! Combat service.
//!
//! This module owns target acquisition, chase orders, weapon facing/setup, damage, and combat
//! events for a tick. It depends on read-only rules and derived spatial/LOS helpers.

use std::collections::HashMap;

use crate::config;
use crate::game::ability::AbilityKind;
use crate::game::entity::{AttackPhase, Entity, EntityKind, EntityStore, Order};
use crate::game::firing_reveal::{record_firing_reveals_for_victim_team, FiringRevealSource};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::mortar::{rotate_mortar_for_fire, MortarShellStore};
use crate::game::mortar_scatter::predicted_mortar_impact;
use crate::game::panzerfaust_shot::PanzerfaustShotStore;
use crate::game::services::dist2;
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::{Occupancy, StaticPathingRelation};
use crate::game::services::spatial::SpatialIndex;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::protocol::Event;
use rand::Rng;

mod acquisition;
mod activation;
mod chase;
mod coax;
mod damage;
mod events;
mod panzerfaust;
mod priority;
mod projection;
mod shot_blocker_index;
mod target_policy;
mod weapons;

#[cfg(test)]
mod tests;
#[cfg(test)]
use acquisition::combat_mode;
use acquisition::{
    combat_mode_with_moving_fire, direct_fire_target_legal,
    resolve_target as resolve_target_with_obstruction, CombatMode, DirectFireLegality,
    DirectFireVisibility,
};
use chase::{chase_goal_for_target, chase_path_needs_refresh};
use damage::apply_damage;
use shot_blocker_index::ShotBlockerIndex;
use weapons::{
    anti_tank_gun_can_chase, begin_idle_deployed_weapon_setup, can_fire_while_moving,
    deployed_weapon_ready_to_fire, deployed_weapon_ready_to_move, effective_attack_profile,
    mirror_weapon_to_body, moving_fire_miss_chance, moving_fire_move_order_holds_path,
    relax_vehicle_weapon_toward_body, rotate_anti_tank_gun_for_combat,
    rotate_vehicle_weapon_for_combat, tick_deployed_weapon_setup,
    update_attack_move_no_target_teardown, uses_stationary_weapon_aggro,
    uses_vehicle_weapon_policy,
};

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn resolve_target(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    mode: CombatMode,
) -> Option<u32> {
    let occ = Occupancy::build(map, entities);
    let blockers = ShotBlockerIndex::build(map, entities);
    let tank_trap_relation = StaticPathingRelation::for_player(owner, teams);
    let tank_trap_obstructs_vehicle_route = |attacker: &Entity, target: &Entity| {
        occ.tank_trap_obstructs_vehicle_route(attacker, target, &tank_trap_relation)
    };
    let attacker_can_fire_while_moving = entities
        .get(self_id)
        .map(|e| can_fire_while_moving(e, false))
        .unwrap_or(false);
    resolve_target_with_obstruction(
        map,
        entities,
        &blockers,
        teams,
        spatial,
        los,
        fog,
        smokes,
        &tank_trap_obstructs_vehicle_route,
        self_id,
        owner,
        px,
        py,
        acquire_px,
        mode,
        attacker_can_fire_while_moving,
        &|_| true,
    )
}

/// Extra slack (px) added to attack range checks so units don't dance at the exact boundary.
pub(super) const RANGE_SLACK: f32 = 4.0;
pub(super) const TANK_TURRET_TURN_RATE_RAD_PER_TICK: f32 = 0.070;
pub(super) const TANK_TURRET_FIRE_TOLERANCE_RAD: f32 = 0.18;
pub(super) const ANTI_TANK_GUN_TURN_RATE_RAD_PER_TICK: f32 = 0.035;
pub(super) const ANTI_TANK_GUN_FIRE_TOLERANCE_RAD: f32 = 0.12;
pub(super) const TANK_STANDOFF_BUFFER_PX: f32 = config::TILE_SIZE as f32;
pub(super) const TANK_STANDOFF_REPATH_DELTA_PX: f32 = config::TILE_SIZE as f32;
const FIRING_REVEAL_RESPONSE_DELAY_TICKS: u32 = config::TICK_HZ;

/// Acquire combat targets, apply damage, and emit attack events for one tick.
#[allow(clippy::too_many_arguments)]
pub(in crate::game) fn combat_system(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    mortar_autocast_researched: &dyn Fn(u32) -> bool,
    methamphetamines_researched: &dyn Fn(u32) -> bool,
    occ: &Occupancy,
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    mortar_shells: &mut MortarShellStore,
    panzerfaust_shots: &mut PanzerfaustShotStore,
    rng: &mut impl Rng,
    events: &mut HashMap<u32, Vec<Event>>,
    firing_reveals: &mut Vec<FiringRevealSource>,
    tick: u32,
) {
    let los = LineOfSight::with_smoke(map, smokes);
    for id in entities.ids() {
        if let Some(e) = entities.get_mut(id) {
            e.tick_weapon_cooldowns();
            tick_deployed_weapon_setup(e);
            weapons::tick_tank_stationary_range(e);
            e.retain_firing_reveal_reaction_gates(
                |target_id, reveal_viewer, episode_started_at_tick| {
                    fog.active_firing_reveal_episode(reveal_viewer, target_id)
                        == Some(episode_started_at_tick)
                },
            );
        }
    }
    let blockers = ShotBlockerIndex::build(map, entities);
    panzerfaust::tick_states(
        map,
        entities,
        &blockers,
        teams,
        methamphetamines_researched,
        fog,
        smokes,
        panzerfaust_shots,
        events,
        tick,
    );
    for id in entities.ids() {
        if panzerfaust::handle_combat_if_panzerfaust(
            map,
            entities,
            &blockers,
            teams,
            methamphetamines_researched,
            occ,
            spatial,
            coordinator,
            fog,
            smokes,
            id,
        ) {
            continue;
        }
        // Determine this attacker's combat parameters.
        let (
            owner,
            px,
            py,
            range_px,
            acquire_px,
            weapon_profile,
            dmg,
            cd_reset,
            mode,
            is_unit,
            is_mortar_team,
            can_move_fire,
        ) = {
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
            let Some(weapon_profile) = profile.weapon else {
                continue;
            };
            let (range_tiles, dmg, cd) = (profile.range_tiles, profile.dmg, profile.cooldown);
            let owner_has_meth = methamphetamines_researched(e.owner);
            let can_move_fire = can_fire_while_moving(e, owner_has_meth);
            let cd = if e.kind == EntityKind::Rifleman && owner_has_meth {
                cd.saturating_mul(config::METHAMPHETAMINES_ATTACK_COOLDOWN_NUMERATOR)
                    / config::METHAMPHETAMINES_ATTACK_COOLDOWN_DENOMINATOR
            } else {
                cd
            };
            let range_px = range_tiles * config::TILE_SIZE as f32 + e.radius() + RANGE_SLACK;
            // Aggro radius: mobile units detect and chase enemies out to their sight radius so
            // attack-move / auto-defend actually close the gap. Idle deployed weapons are the
            // exception: they hold position and only auto-acquire enemies already in weapon
            // range. Buildings never move, so they only ever engage within their firing range.
            let aggro_px = if e.is_unit() {
                if matches!(e.order(), Order::HoldPosition)
                    || (uses_stationary_weapon_aggro(e) && matches!(e.order(), Order::Idle))
                {
                    range_px
                } else {
                    (e.sight_tiles() as f32 * config::TILE_SIZE as f32).max(range_px)
                }
            } else {
                range_px
            };
            let mode = combat_mode_with_moving_fire(e, can_move_fire);
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
                weapon_profile,
                dmg,
                cd,
                mode,
                e.is_unit(),
                e.kind == EntityKind::MortarTeam,
                can_move_fire,
            )
        };
        if dmg == 0 {
            continue;
        }

        // Resolve / acquire a target id based on the current order semantics.
        let tank_trap_relation = StaticPathingRelation::for_player(owner, teams);
        let tank_trap_obstructs_vehicle_route = |attacker: &Entity, target: &Entity| {
            occ.tank_trap_obstructs_vehicle_route(attacker, target, &tank_trap_relation)
        };
        let require_safe_mortar_autocast_target = is_mortar_team
            && matches!(
                entities
                    .get(id)
                    .and_then(|e| e.autocast_enabled(AbilityKind::MortarFire)),
                Some(true)
            )
            && mortar_autocast_researched(owner);
        let target = resolve_target_with_obstruction(
            map,
            entities,
            &blockers,
            teams,
            spatial,
            &los,
            fog,
            smokes,
            &tank_trap_obstructs_vehicle_route,
            id,
            owner,
            px,
            py,
            acquire_px,
            mode,
            can_move_fire,
            &|target_id| {
                !require_safe_mortar_autocast_target
                    || mortar_autocast_target_safe(
                        entities, teams, fog, spatial, owner, id, target_id, tick,
                    )
            },
        );
        let Some(tid) = target else {
            if let Some(e) = entities.get_mut(id) {
                if matches!(
                    e.order(),
                    Order::Attack(_)
                        | Order::AttackMove(_)
                        | Order::Move(_)
                        | Order::Idle
                        | Order::HoldPosition
                ) {
                    e.set_target_id(None);
                    begin_idle_deployed_weapon_setup(e);
                }
                if uses_vehicle_weapon_policy(e) {
                    relax_vehicle_weapon_toward_body(e);
                }
            }
            update_attack_move_no_target_teardown(entities, id);
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
        if !(teams.is_enemy_owner(owner, t_owner)
            || mode == CombatMode::Ordered && t_owner == owner)
        {
            continue; // auto-acquisition stays hostile-only; explicit self-attacks are ordered.
        }
        let dist = dist2(px, py, tx, ty).sqrt();
        let target_angle = (ty - py).atan2(tx - px);
        let holds_commanded_movement_path = entities
            .get(id)
            .map(|e| moving_fire_move_order_holds_path(e, can_move_fire))
            .unwrap_or(false);
        let clear_shot = if is_mortar_team {
            true
        } else if mode == CombatMode::Ordered {
            direct_fire_target_legal(
                map,
                entities,
                &blockers,
                teams,
                &los,
                fog,
                smokes,
                id,
                owner,
                (px, py),
                tid,
                DirectFireLegality::intended_target(DirectFireVisibility::Team),
            )
        } else {
            direct_fire_target_legal(
                map,
                entities,
                &blockers,
                teams,
                &los,
                fog,
                smokes,
                id,
                owner,
                (px, py),
                tid,
                DirectFireLegality::auto_acquire(),
            )
        };

        if dist <= range_px && clear_shot {
            // In range: aim, stop, deploy if needed, and fire if off cooldown.
            let mut weapon_aligned = true;
            if let Some(e) = entities.get_mut(id) {
                if uses_vehicle_weapon_policy(e) {
                    weapon_aligned = rotate_vehicle_weapon_for_combat(e, target_angle);
                } else if e.kind == EntityKind::AntiTankGun {
                    weapon_aligned = rotate_anti_tank_gun_for_combat(e, target_angle);
                } else if e.kind == EntityKind::MortarTeam {
                    weapon_aligned = rotate_mortar_for_fire(e, target_angle);
                } else if target_angle.is_finite() {
                    e.set_facing(target_angle);
                    mirror_weapon_to_body(e, target_angle);
                }
                e.set_target_id(Some(tid));
                e.mark_attack_phase(AttackPhase::Firing);
                // Plain Move keeps advancing while firing. Attack Move stops to engage.
                if !holds_commanded_movement_path {
                    e.clear_path();
                }
            }
            if !weapon_aligned {
                continue;
            }
            if !deployed_weapon_ready_to_fire(entities, id) {
                continue;
            }
            let reveal_only_source = if mode == CombatMode::Ordered {
                fog.team_firing_reveal_only_source(owner, (tx, ty), teams)
            } else {
                fog.firing_reveal_only_source_at_world(owner, tx, ty)
            };
            if let Some(episode) = reveal_only_source {
                let reaction_ready = entities.get_mut(id).is_some_and(|e| {
                    e.weapon_firing_reveal_reaction_ready(
                        weapon_profile.id,
                        tid,
                        episode,
                        tick,
                        FIRING_REVEAL_RESPONSE_DELAY_TICKS,
                    )
                });
                if !reaction_ready {
                    continue;
                }
            }
            let ready =
                matches!(entities.get(id), Some(e) if e.weapon_cooldown(weapon_profile.id) == 0);
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
                    let (impact_x, impact_y) =
                        predicted_mortar_impact(fog, teams, owner, id, mx, my, tick);
                    if mortar_autocast_would_hit_same_team_entity(
                        entities, teams, spatial, owner, impact_x, impact_y,
                    ) {
                        continue;
                    }
                    mortar_shells
                        .schedule(events, fog, teams, owner, id, px, py, mx, my, tick, true);
                    if let Some(e) = entities.get_mut(id) {
                        e.set_weapon_cooldown(weapon_profile.id, cd_reset);
                    }
                    continue;
                }
                let extra_miss_chance =
                    entities.get(id).map(moving_fire_miss_chance).unwrap_or(0.0);
                let shot_victim_owner = apply_damage(
                    map,
                    entities,
                    &blockers,
                    teams,
                    events,
                    fog,
                    smokes,
                    rng,
                    id,
                    tid,
                    weapon_profile,
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
                if is_unit {
                    if let Some(victim_owner) = shot_victim_owner {
                        let player_ids = events.keys().copied().collect::<Vec<_>>();
                        record_firing_reveals_for_victim_team(
                            firing_reveals,
                            player_ids,
                            fog,
                            teams,
                            victim_owner,
                            owner,
                            id,
                            (px, py),
                            tick,
                            cd_reset,
                        );
                    }
                }
                if let Some(e) = entities.get_mut(id) {
                    e.set_weapon_cooldown(weapon_profile.id, cd_reset);
                }
            }
        } else if is_unit && mode != CombatMode::Opportunistic && !holds_commanded_movement_path {
            // Out of weapon range but within aggro: chase. Tanks route to a standoff point,
            // and statically blocked targets route to a passable perimeter tile.
            let chase_goal =
                chase_goal_for_target(map, entities, id, (px, py), (tx, ty), range_px, dist);
            let chase_goal = coordinator.attack_chase_goal(entities, id, tid, chase_goal, range_px);
            let want_repath = entities
                .get(id)
                .map(|e| chase_path_needs_refresh(e, chase_goal))
                .unwrap_or(false);
            let mut can_chase = true;
            if let Some(e) = entities.get_mut(id) {
                if uses_vehicle_weapon_policy(e) {
                    rotate_vehicle_weapon_for_combat(e, target_angle);
                } else if e.kind == EntityKind::AntiTankGun {
                    rotate_anti_tank_gun_for_combat(e, target_angle);
                } else if e.kind == EntityKind::MortarTeam {
                    rotate_mortar_for_fire(e, target_angle);
                } else if target_angle.is_finite() {
                    mirror_weapon_to_body(e, e.facing());
                }
                e.set_target_id(Some(tid));
                e.mark_attack_phase(AttackPhase::Chasing);
                can_chase = anti_tank_gun_can_chase(e);
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
    coax::fire_tank_coax_system(
        map,
        entities,
        &blockers,
        teams,
        spatial,
        &los,
        fog,
        smokes,
        rng,
        events,
        firing_reveals,
        tick,
    );
}

#[allow(clippy::too_many_arguments)]
fn mortar_autocast_target_safe(
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    spatial: &SpatialIndex,
    owner: u32,
    attacker: u32,
    target: u32,
    tick: u32,
) -> bool {
    let (x, y) = mortar_aim_point(entities, target, tick);
    let (impact_x, impact_y) = predicted_mortar_impact(fog, teams, owner, attacker, x, y, tick);
    !mortar_autocast_would_hit_same_team_entity(entities, teams, spatial, owner, impact_x, impact_y)
}

fn mortar_aim_point(entities: &EntityStore, target: u32, _tick: u32) -> (f32, f32) {
    let Some(t) = entities.get(target) else {
        return (0.0, 0.0);
    };
    let mut x = t.pos_x;
    let mut y = t.pos_y;
    if let Some((dx, dy)) = mortar_lead_delta(t) {
        x += dx;
        y += dy;
    }
    (x, y)
}

fn mortar_lead_delta(target: &Entity) -> Option<(f32, f32)> {
    let (vx, vy) = target.movement_delta();
    if !vx.is_finite() || !vy.is_finite() {
        return None;
    }
    let lead_ticks = config::MORTAR_SHELL_DELAY_TICKS as f32;
    let mut dx = vx * lead_ticks;
    let mut dy = vy * lead_ticks;
    let lead_dist = (dx * dx + dy * dy).sqrt();
    if lead_dist <= f32::EPSILON || !lead_dist.is_finite() {
        return None;
    }
    let max_lead = config::unit_stats(target.kind)
        .map(|stats| stats.speed * lead_ticks)
        .unwrap_or(0.0);
    if max_lead <= f32::EPSILON || !max_lead.is_finite() {
        return None;
    }
    if lead_dist > max_lead {
        let scale = max_lead / lead_dist;
        dx *= scale;
        dy *= scale;
    }
    Some((dx, dy))
}

fn mortar_autocast_would_hit_same_team_entity(
    entities: &EntityStore,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    owner: u32,
    x: f32,
    y: f32,
) -> bool {
    let outer_radius = config::MORTAR_OUTER_RADIUS_TILES * config::TILE_SIZE as f32;
    let outer2 = outer_radius * outer_radius;
    spatial.ids_in_circle_bbox(x, y, outer_radius).any(|id| {
        entities.get(id).is_some_and(|e| {
            teams.same_team_or_same_owner(owner, e.owner)
                && e.hp > 0
                && !e.is_node()
                && dist2(x, y, e.pos_x, e.pos_y) <= outer2
        })
    })
}
