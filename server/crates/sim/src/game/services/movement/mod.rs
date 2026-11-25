//! Unit movement service.
//!
//! This module owns per-tick waypoint advancement and collision cleanup for mobile entities.
//! Path requests are prepared by `move_coordinator`, but landing legality, vehicle steering,
//! weapon setup restrictions, and final overlap resolution live here so the tick pipeline has a
//! single movement phase.

use crate::config;
use crate::game::ability_runtime::AbilityRuntime;
use crate::game::entity::EntityStore;
use crate::game::entity::{EntityKind, WeaponSetup};
use crate::game::map::Map;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::scout_plane;
use crate::game::services::spatial::SpatialIndex;
use crate::game::smoke::SmokeCloudStore;
use crate::game::upgrade::UpgradeKind;
use crate::game::PlayerState;
use rts_rules::math::GameplayMath as _;
use std::collections::BTreeSet;

mod armor_reaction;
mod barbed_wire;
mod body_growth;
pub(crate) use body_growth::advance_dao_masters;
mod car_drive;
mod collision;
mod pivot_drive;
mod pivot_recovery;
mod standability;
mod steering;
mod tank_point;
mod traffic;
mod vehicle_profiles;
mod vehicle_route;
mod waypoints;

#[cfg(test)]
mod tests;

/// World pixels at which a unit is considered "arrived" at a waypoint / target point.
pub(super) const ARRIVE_EPS: f32 = 2.0;

/// Conservative fallback for broad-phase bounding-box queries if an entity body is unavailable.
pub(super) const MAX_UNIT_BOUNDING_RADIUS_PX: f32 = 32.0;

pub(super) const STEERING_MAX_NEIGHBORS: usize = 16;

#[cfg(test)]
pub(crate) fn resolve_collisions(
    entities: &mut EntityStore,
    spatial: &SpatialIndex,
    map: &Map,
    occ: &Occupancy,
) -> bool {
    resolve_collisions_with_teams(
        entities,
        spatial,
        map,
        occ,
        &crate::game::teams::TeamRelations::from_player_teams([]),
        None,
    )
}

pub(crate) fn resolve_collisions_with_teams(
    entities: &mut EntityStore,
    spatial: &SpatialIndex,
    map: &Map,
    occ: &Occupancy,
    teams: &crate::game::teams::TeamRelations,
    anchor_context: Option<(&AbilityRuntime, u32)>,
) -> bool {
    let before = barbed_wire::positions(map, entities);
    collision::resolve_collisions(entities, spatial, map, occ, teams, anchor_context);
    barbed_wire::apply_entries(map, entities, &before)
}
pub(crate) use pivot_drive::{angle_delta, rotate_toward};
pub(crate) use standability::is_collision_anchored;
#[cfg(test)]
use vehicle_profiles::TANK_BODY_TURN_RATE_RAD_PER_TICK;

/// Advance every moving unit along its waypoint path at its speed. Clamps the final landing
/// tile to passable terrain (soft overlap with other units is allowed, so we don't resolve
/// unit-unit collisions here). Arriving at the last waypoint of a plain Move clears the order.
#[cfg(test)]
pub(crate) fn movement_system(
    map: &Map,
    entities: &mut EntityStore,
    players: &[PlayerState],
    occ: &Occupancy,
    spatial: &SpatialIndex,
    tick: u32,
) {
    let smokes = SmokeCloudStore::new();
    let ability_runtime = AbilityRuntime::new();
    movement_system_with_context(
        map,
        entities,
        players,
        occ,
        spatial,
        tick,
        &smokes,
        &ability_runtime,
    );
}

/// Movement entry point for the real tick loop, with live smoke and ability state.
#[allow(clippy::too_many_arguments)]
pub(crate) fn movement_system_with_context(
    map: &Map,
    entities: &mut EntityStore,
    players: &[PlayerState],
    occ: &Occupancy,
    spatial: &SpatialIndex,
    tick: u32,
    smokes: &SmokeCloudStore,
    ability_runtime: &AbilityRuntime,
) {
    let wire_positions = barbed_wire::positions(map, entities);
    for id in entities.ids() {
        if let Some(e) = entities.get_mut(id) {
            e.reset_movement_delta();
        }
    }
    for id in entities.ids() {
        let Some((kind, owner)) = entities.get(id).map(|e| (e.kind, e.owner)) else {
            continue;
        };
        let has_meth = kind == EntityKind::MachineGunner
            && players
                .iter()
                .any(|p| p.id == owner && p.has_upgrade(UpgradeKind::Methamphetamines));
        if has_meth {
            clamp_meth_machine_gunner_setup(entities, id);
        }
    }
    for id in entities.ids() {
        let in_smoke = entities
            .get(id)
            .is_some_and(|e| e.is_unit() && smokes.point_inside(e.pos_x(), e.pos_y()));
        if in_smoke {
            if let Some(e) = entities.get_mut(id) {
                e.mark_in_smoke_for_breakthrough(config::BREAKTHROUGH_RECENT_SMOKE_TICKS);
            }
        }
    }
    scout_plane::advance_scout_planes(map, entities);
    let teams = crate::game::teams::TeamRelations::from_player_teams(
        players.iter().map(|p| (p.id, p.team_id)),
    );
    tank_point::advance(map, entities, occ, &teams);
    let mut turn_blockers = crate::game::services::standability::UnitBlockers::new(entities);
    armor_reaction::turn_stationary_tanks_toward_locked_ap_source(entities, tick, |e, facing| {
        let allowed =
            standability::unit_static_standable(occ, map, e.kind, e.pos_x(), e.pos_y(), facing)
                && turn_blockers.motion_clear(e, (e.pos_x(), e.pos_y()), facing, &teams);
        if allowed {
            let mut turned = e.clone();
            turned.set_facing(facing);
            turn_blockers.update(&turned);
        }
        allowed
    });
    let integrated = waypoints::advance_moving_units(
        map,
        entities,
        players,
        occ,
        spatial,
        tick,
        ability_runtime,
    );
    apply_magic_anchor_pull(
        map,
        entities,
        players,
        occ,
        tick,
        ability_runtime,
        &integrated,
    );
    barbed_wire::apply_entries(map, entities, &wire_positions);
    for id in entities.ids() {
        if let Some(e) = entities.get_mut(id) {
            if let Some(movement) = e.movement.as_mut() {
                movement.novice_charge_ticks = movement.novice_charge_ticks.saturating_sub(1);
            }
            e.tick_breakthrough_status();
            e.expire_shield(tick);
            e.tick_ability_cooldowns();
            e.tick_ability_charge_recharges();
        }
    }
}

fn clamp_meth_machine_gunner_setup(entities: &mut EntityStore, id: u32) {
    let Some(e) = entities.get_mut(id) else {
        return;
    };
    let boosted_ticks = config::METHAMPHETAMINES_MACHINE_GUNNER_SETUP_TICKS;
    let boosted_setup = match e.weapon_setup() {
        WeaponSetup::SettingUp { ticks } if ticks > boosted_ticks => Some(WeaponSetup::SettingUp {
            ticks: boosted_ticks,
        }),
        WeaponSetup::TearingDown { ticks } if ticks > boosted_ticks => {
            Some(WeaponSetup::TearingDown {
                ticks: boosted_ticks,
            })
        }
        WeaponSetup::TearingDownToRedeploy { ticks } if ticks > boosted_ticks => {
            Some(WeaponSetup::TearingDownToRedeploy {
                ticks: boosted_ticks,
            })
        }
        _ => None,
    };
    if let Some(setup) = boosted_setup {
        e.set_weapon_setup(setup);
    }
}

fn anchor_contact_ids(
    entities: &EntityStore,
    teams: &crate::game::teams::TeamRelations,
    runtime: &AbilityRuntime,
    tick: u32,
) -> Vec<u32> {
    entities
        .iter()
        .filter(|e| {
            e.is_unit()
                && e.kind != EntityKind::ScoutPlane
                && runtime
                    .magic_anchor_pull(e.owner, teams, e.pos_x(), e.pos_y(), tick)
                    .is_some()
        })
        .map(|e| e.id)
        .collect()
}

fn anchor_displacement(
    e: &crate::game::entity::Entity,
    teams: &crate::game::teams::TeamRelations,
    runtime: &AbilityRuntime,
    tick: u32,
) -> Option<(f32, f32)> {
    let (dir, strength, distance) =
        runtime.magic_anchor_pull(e.owner, teams, e.pos_x(), e.pos_y(), tick)?;
    let resistance = standability::footing_resistance(standability::footing_profile(e)).max(1.0);
    let magnitude =
        (e.movement_speed() * config::EKAT_MAGIC_ANCHOR_PULL_SPEED_MULTIPLIER * strength
            / resistance.det_sqrt())
        .min(distance);
    (magnitude.is_finite()).then_some((dir.0 * magnitude, dir.1 * magnitude))
}

fn apply_magic_anchor_pull(
    map: &Map,
    entities: &mut EntityStore,
    players: &[PlayerState],
    occ: &Occupancy,
    tick: u32,
    ability_runtime: &AbilityRuntime,
    integrated: &BTreeSet<u32>,
) {
    let teams = crate::game::teams::TeamRelations::from_player_teams(
        players.iter().map(|p| (p.id, p.team_id)),
    );
    let mut blockers = crate::game::services::standability::UnitBlockers::with_contact_locks(
        entities,
        anchor_contact_ids(entities, &teams, ability_runtime, tick),
    );
    let world_max_x = map.world_width_px() - 0.01;
    let world_max_y = map.world_height_px() - 0.01;
    for id in entities.ids() {
        if integrated.contains(&id) {
            continue;
        }
        let Some((kind, x, y, facing, delta)) = entities.get(id).and_then(|e| {
            if !e.is_unit() || e.kind == EntityKind::ScoutPlane {
                return None;
            }
            Some((
                e.kind,
                e.pos_x(),
                e.pos_y(),
                e.facing(),
                anchor_displacement(e, &teams, ability_runtime, tick)?,
            ))
        }) else {
            continue;
        };
        let next_x = (x + delta.0).clamp(0.0, world_max_x);
        let next_y = (y + delta.1).clamp(0.0, world_max_y);
        if !crate::game::services::standability::unit_static_standable_with_radius(
            map,
            occ,
            kind,
            next_x,
            next_y,
            facing,
            entities.get(id).map(|e| e.radius()),
        ) {
            continue;
        }
        if entities
            .get(id)
            .is_some_and(|e| !blockers.motion_clear(e, (next_x, next_y), facing, &teams))
        {
            continue;
        }
        if let Some(e) = entities.get_mut(id) {
            e.set_position(next_x, next_y);
            blockers.update(e);
            let (next_x, next_y) = (e.pos_x(), e.pos_y());
            let previous_delta = e.movement_delta();
            e.set_movement_delta(previous_delta.0 + next_x - x, previous_delta.1 + next_y - y);
            if kind == EntityKind::Tank && (next_x != x || next_y != y) {
                if let Some(combat) = e.combat.as_mut() {
                    combat.tank_stationary_range_ticks = 0;
                    combat.tank_stationary_range_reset_this_tick = true;
                }
            }
        }
    }
}
