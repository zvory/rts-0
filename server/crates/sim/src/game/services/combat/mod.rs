//! Combat service.
//!
//! This module owns target acquisition and approach, weapon facing/setup, damage, and combat
//! events for a tick. It depends on read-only rules and derived spatial/LOS helpers.

use rts_rules::math::GameplayMath as _;
use std::collections::HashMap;

use crate::config;
use crate::game::entity::{AttackPhase, EntityKind, EntityStore, Order};
use crate::game::firing_reveal::FiringRevealSource;
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::panzerfaust_shot::PanzerfaustShotStore;
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::spatial::SpatialIndex;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::protocol::Event;
use rand::Rng;

mod acquisition;
mod acquisition_pass;
mod activation;
mod coax;
mod damage;
mod events;
mod machine_gun;
mod panzerfaust;
mod pre_movement;
mod priority;
mod projection;
mod shot;
mod shot_blocker_index;
mod target_legality;
mod target_policy;
mod weapons;

#[cfg(test)]
mod tests;
#[cfg(test)]
use acquisition::{combat_mode, resolve_target as resolve_target_for_test};
use acquisition::{combat_mode_with_moving_fire, CombatMode};
use projection::primary_target_distance_sq;
use shot_blocker_index::ShotBlockerIndex;
use target_legality::{direct_fire_target_legal, DirectFireLegality};
use weapons::{
    begin_idle_deployed_weapon_setup, can_fire_while_moving, deployed_weapon_ready_to_fire,
    effective_attack_profile, mirror_weapon_to_body, moving_attack_cooldown,
    moving_fire_move_order_holds_path, relax_vehicle_weapon_toward_body,
    rotate_anti_tank_gun_for_combat, rotate_vehicle_weapon_for_combat, tick_deployed_weapon_setup,
    update_attack_move_no_target_teardown, uses_vehicle_weapon_policy,
};

pub(in crate::game) use pre_movement::hold_attack_move_engagements;

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
    let blockers = ShotBlockerIndex::build(map, entities);
    let attacker_can_fire_while_moving = entities
        .get(self_id)
        .map(|e| can_fire_while_moving(e, false))
        .unwrap_or(false);
    resolve_target_for_test(
        map,
        entities,
        &blockers,
        teams,
        spatial,
        los,
        fog,
        smokes,
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
pub(super) const ANTI_TANK_GUN_TURN_RATE_RAD_PER_TICK: f32 = 0.070;
pub(super) const ANTI_TANK_GUN_FIRE_TOLERANCE_RAD: f32 = 0.12;
const FIRING_REVEAL_RESPONSE_DELAY_TICKS: u32 = config::TICK_HZ / 2;

/// Acquire combat targets, apply damage, and emit attack events for one tick.
#[allow(clippy::too_many_arguments)]
pub(in crate::game) fn combat_system(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    methamphetamines_researched: &dyn Fn(u32) -> bool,
    charge_researched: &dyn Fn(u32) -> bool,
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    panzerfaust_shots: &mut PanzerfaustShotStore,
    disciple_shots: &mut crate::game::disciple_shot::DiscipleShotStore,
    rng: &mut impl Rng,
    events: &mut HashMap<u32, Vec<Event>>,
    firing_reveals: &mut Vec<FiringRevealSource>,
    tick: u32,
) {
    shot::resolve_orbs(map, entities, teams, fog, events, disciple_shots, tick);
    let mut body_blockers = crate::game::services::standability::UnitBlockers::new(entities);
    let los = LineOfSight::with_smoke(map, smokes);
    for id in entities.ids() {
        if let Some(e) = entities.get_mut(id) {
            e.tick_weapon_cooldowns();
            tick_deployed_weapon_setup(e, |e, facing| {
                body_blockers.motion_clear(e, (e.pos_x(), e.pos_y()), facing, teams)
            });
            body_blockers.update(e);
            weapons::tick_tank_stationary_range(e);
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
    // Immediate damage makes turn order observable in lethal same-tick exchanges. Reversing the
    // complete order each tick flips precedence for every entity pair instead of permanently
    // favoring older, lower-id entities.
    let mut combat_order = entities.ids();
    if tick % 2 == 1 {
        combat_order.reverse();
    }
    for id in combat_order.iter().copied() {
        if entities
            .get(id)
            .is_some_and(|entity| entity.kind == EntityKind::MortarTeam)
        {
            if let Some(entity) = entities.get_mut(id) {
                entity.set_target_id(None);
                entity.mark_attack_phase(AttackPhase::Waiting);
            }
            continue;
        }
        if panzerfaust::handle_combat_if_panzerfaust(
            map,
            entities,
            &blockers,
            teams,
            methamphetamines_researched,
            spatial,
            &mut |entities, attacker, target, min_range_px, max_range_px| {
                coordinator.request_direct_attack_path(
                    entities,
                    attacker,
                    target,
                    min_range_px,
                    max_range_px,
                );
            },
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
            min_range_px,
            range_px,
            acquire_px,
            weapon_profile,
            dmg,
            cd_reset,
            mode,
            can_move_fire,
        ) = {
            let e = match entities.get(id) {
                Some(e) => e,
                None => continue,
            };
            if e.hp == 0 || !e.can_attack() {
                continue;
            }
            // Workers executing a Gather order ignore nearby enemies so defensive fire cannot
            // distract them from the resource node. An explicit Attack
            // order overrides Gather upstream, so this only suppresses auto-acquisition.
            if matches!(e.order(), Order::Gather(_)) {
                continue;
            }
            let profile = effective_attack_profile(e);
            let Some(weapon_profile) = profile.weapon else {
                continue;
            };
            let (range_tiles, dmg, cd) = (profile.range_tiles, profile.dmg, profile.cooldown);
            let owner_has_meth = methamphetamines_researched(e.owner);
            let can_move_fire = can_fire_while_moving(e, owner_has_meth);
            let cd = if crate::rules::is_rifle_infantry(e.kind) && owner_has_meth {
                config::METHAMPHETAMINES_RIFLE_ATTACK_COOLDOWN_TICKS
            } else {
                cd
            };
            let cd = moving_attack_cooldown(e, e.shop_item_reload(cd));
            let range_px = range_tiles * config::TILE_SIZE as f32 + e.radius() + RANGE_SLACK;
            let min_range_px = profile.min_range_tiles * config::TILE_SIZE as f32;
            let mode = combat_mode_with_moving_fire(e, can_move_fire);
            // Only aggressive orders may use an engagement range beyond weapon reach.
            // Hold/Point and plain Move keep their existing in-range behavior.
            let acquire_px = if mode == CombatMode::Aggressive {
                crate::rules::combat::engagement_range_tiles(e.kind, range_tiles)
                    * config::TILE_SIZE as f32
                    + e.radius()
                    + RANGE_SLACK
            } else {
                range_px
            };
            (
                e.owner,
                e.pos_x(),
                e.pos_y(),
                min_range_px,
                range_px,
                acquire_px,
                weapon_profile,
                dmg,
                cd,
                mode,
                can_move_fire,
            )
        };
        if dmg == 0 {
            continue;
        }

        let auto_pursuit = mode == CombatMode::Aggressive && acquire_px > range_px;

        // Resolve / acquire a target id based on the current order semantics.
        let target = acquisition_pass::select(
            map,
            entities,
            &blockers,
            teams,
            spatial,
            &los,
            fog,
            smokes,
            id,
            owner,
            px,
            py,
            acquire_px,
            mode,
            can_move_fire,
            weapon_profile.id,
            range_px,
            tick,
        );
        let Some(tid) = target else {
            if let Some(e) = entities.get_mut(id) {
                e.end_war_charge();
                if matches!(
                    e.order(),
                    Order::Attack(_)
                        | Order::AttackMove(_)
                        | Order::Move(_)
                        | Order::Idle
                        | Order::Point { .. }
                        | Order::HoldPosition
                ) {
                    // Ranged evolution can leave an earlier sword approach behind.
                    // Idle has no commanded route to preserve after its target is lost.
                    if auto_pursuit || (e.kind.is_disciple() && matches!(e.order(), Order::Idle)) {
                        if matches!(e.order(), Order::Idle) {
                            e.clear_path();
                            e.set_path_goal(None);
                        } else if e
                            .path_goal()
                            .is_some_and(|goal| Some(goal) != e.move_intent())
                        {
                            // Cancel the temporary approach even if repathing is throttled.
                            e.clear_path();
                            e.set_path_goal(e.move_intent());
                            e.mark_move_phase(crate::game::entity::MovePhase::AwaitingPath);
                        }
                    }
                    e.set_target_id(None);
                    e.mark_attack_phase(AttackPhase::Waiting);
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
                        coordinator.request_attack_move_path(entities, id, goal);
                    }
                }
            }
            continue;
        };

        // Distance to chosen target.
        let (tx, ty, t_owner, neutral_obstacle, target_distance_sq) = match entities.get(tid) {
            Some(t) => (
                t.pos_x(),
                t.pos_y(),
                t.owner,
                t.is_neutral_obstacle(),
                primary_target_distance_sq(map, weapon_profile.id, (px, py), t),
            ),
            None => continue,
        };
        if !(neutral_obstacle
            || teams.is_enemy_owner(owner, t_owner)
            || mode == CombatMode::Ordered && t_owner == owner)
        {
            continue; // Auto-acquisition stays hostile/obstacle-only; self-attacks are ordered.
        }
        if charge_researched(owner)
            && entities.get(id).is_some_and(|e| {
                matches!(e.kind, EntityKind::Warrior | EntityKind::WarAdept)
                    && e.ability_cooldown_ticks(crate::game::ability::AbilityKind::Charge) == 0
            })
        {
            if let Some((position, movement, hostile_unit)) = entities.get(tid).map(|target| {
                (
                    (target.pos_x(), target.pos_y()),
                    target.movement_delta(),
                    target.is_unit() && target.hp > 0 && teams.is_enemy_owner(owner, t_owner),
                )
            }) {
                if let Some(attacker) = entities.get_mut(id) {
                    attacker.try_start_novice_charge(position, movement, hostile_unit);
                }
            }
        }
        let dist = target_distance_sq.det_sqrt();
        let clear_area_priority_target = entities.get(id).is_some_and(|entity| {
            entity
                .order()
                .clear_obstacle_area()
                .is_some_and(|objective| {
                    neutral_obstacle
                        && entities.get(tid).is_some_and(|target| {
                            let dx = target.pos_x() - objective.center_x;
                            let dy = target.pos_y() - objective.center_y;
                            let radius = config::TANK_TRAP_CLUSTER_ATTACK_RADIUS_TILES
                                * config::TILE_SIZE as f32;
                            dx.det_mul_add(dx, dy * dy) <= radius * radius
                        })
                })
        });
        let commanded_direct_target = clear_area_priority_target
            || (mode == CombatMode::Ordered
                && entities
                    .get(id)
                    .and_then(|entity| entity.order().attack_target())
                    == Some(tid));
        let target_in_weapon_range = dist >= min_range_px && dist <= range_px;
        let target_angle = (ty - py).det_atan2(tx - px);
        let holds_commanded_movement_path = entities
            .get(id)
            .map(|e| moving_fire_move_order_holds_path(e, can_move_fire))
            .unwrap_or(false);
        let clear_shot = if mode == CombatMode::Ordered || clear_area_priority_target {
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
                DirectFireLegality::IntendedTarget,
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
                DirectFireLegality::AutoAcquire,
            )
        };
        // A direct tank order can wait for its stationary range instead of spending that
        // range by starting a chase. Fire eligibility still uses the current ramped range.
        let wait_for_stationary_range = clear_shot
            && commanded_direct_target
            && entities.get(id).is_some_and(|e| {
                e.kind == EntityKind::Tank
                    && dist
                        <= projection::TANK_STATIONARY_RANGE_MAX_TILES * config::TILE_SIZE as f32
                            + e.radius()
                            + RANGE_SLACK
            });
        // Wind and Haste carriers may finish their approach while striking. An Attack Move's
        // original travel route must still stop for an in-range engagement.
        let moving_attack_approach = entities.get(id).is_some_and(|e| {
            (e.kind == EntityKind::WindAdept
                || e.shop_item_active(crate::game::ability::AbilityKind::Haste))
                && (commanded_direct_target
                    || (auto_pursuit
                        && e.path_goal()
                            .is_some_and(|goal| Some(goal) != e.move_intent())))
        });
        if commanded_direct_target
            && !moving_attack_approach
            && clear_shot
            && (target_in_weapon_range || wait_for_stationary_range)
        {
            if let Some(e) = entities.get_mut(id) {
                e.clear_path();
                e.set_path_goal(None);
            }
        }
        let mut fired = false;

        if target_in_weapon_range && clear_shot {
            // In range: aim, stop, deploy if needed, and fire if off cooldown.
            let mut weapon_aligned = true;
            if let Some(e) = entities.get_mut(id) {
                if uses_vehicle_weapon_policy(e) {
                    weapon_aligned = rotate_vehicle_weapon_for_combat(e, target_angle);
                } else if e.kind == EntityKind::AntiTankGun {
                    weapon_aligned =
                        rotate_anti_tank_gun_for_combat(e, target_angle, |e, facing| {
                            body_blockers.motion_clear(e, (e.pos_x(), e.pos_y()), facing, teams)
                        });
                    body_blockers.update(e);
                } else if target_angle.is_finite() {
                    e.set_facing(target_angle);
                    mirror_weapon_to_body(e, target_angle);
                }
                e.set_target_id(Some(tid));
                e.mark_attack_phase(AttackPhase::Firing);
                // Moving-fire Move orders and eligible attack approaches keep advancing.
                if !holds_commanded_movement_path && !moving_attack_approach {
                    e.clear_path();
                }
            }
            if !weapon_aligned {
                continue;
            }
            if !deployed_weapon_ready_to_fire(entities, id) {
                continue;
            }
            fired = shot::try_fire(
                map,
                entities,
                &blockers,
                teams,
                spatial,
                &los,
                fog,
                smokes,
                disciple_shots,
                rng,
                events,
                firing_reveals,
                id,
                tid,
                &weapon_profile,
                range_px,
                dmg,
                cd_reset,
                tick,
            );
        } else if mode == CombatMode::Ordered || clear_area_priority_target || auto_pursuit {
            if let Some(e) = entities.get_mut(id) {
                e.mark_attack_phase(AttackPhase::Waiting);
            }
            if auto_pursuit {
                if let Some(e) = entities.get_mut(id) {
                    e.set_target_id(Some(tid));
                }
            }
            if (commanded_direct_target || auto_pursuit) && !wait_for_stationary_range {
                // A blocked tank must close for a clear shot even inside its range annulus.
                // Pursue the target until normal fire eligibility can stop the approach.
                let pursuit_range_px = if !clear_shot
                    && entities.get(id).is_some_and(|e| e.kind == EntityKind::Tank)
                {
                    0.0
                } else {
                    range_px
                };
                coordinator.request_direct_attack_path(
                    entities,
                    id,
                    tid,
                    min_range_px,
                    pursuit_range_px,
                );
            }
        }
        if fired {
            if let Some(e) = entities.get_mut(id).filter(|e| {
                e.kind == EntityKind::BowNovice
                    || (e.kind == EntityKind::Rifleman && methamphetamines_researched(owner))
            }) {
                if let Some(combat) = e.combat.as_mut() {
                    combat.rifle_post_shot_tick = Some(tick);
                }
            }
            // Keep the shot's target through the reload if it remains legal. Re-ranking here
            // made dense concealed engagements churn between freshly revealed targets, which
            // repeatedly incurred their firing-reveal reaction delay instead of letting a
            // deployed weapon complete its normal firing cycle. `select` retains this target
            // through reload, and falls back to priority acquisition if it died or disappeared.
            let target_survived = entities.get(tid).is_some_and(|target| target.hp > 0);
            let next_target = if target_survived {
                acquisition_pass::select(
                    map,
                    entities,
                    &blockers,
                    teams,
                    spatial,
                    &los,
                    fog,
                    smokes,
                    id,
                    owner,
                    px,
                    py,
                    acquire_px,
                    mode,
                    can_move_fire,
                    weapon_profile.id,
                    range_px,
                    tick,
                )
            } else {
                acquisition_pass::acquire(
                    map,
                    entities,
                    &blockers,
                    teams,
                    spatial,
                    &los,
                    fog,
                    smokes,
                    id,
                    owner,
                    px,
                    py,
                    acquire_px,
                    mode,
                    can_move_fire,
                )
            };
            if let Some(e) = entities.get_mut(id) {
                e.set_target_id(next_target);
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
        disciple_shots,
        rng,
        events,
        firing_reveals,
        tick,
        &combat_order,
    );
}
