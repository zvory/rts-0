//! Unit movement service.
//!
//! This module owns per-tick waypoint advancement and collision cleanup for mobile entities.
//! Path requests are prepared by `move_coordinator`, but landing legality, vehicle steering,
//! weapon setup restrictions, and final overlap resolution live here so the tick pipeline has a
//! single movement phase.

use std::collections::HashMap;

use crate::config;
use crate::game::ability_runtime::AbilityRuntime;
use crate::game::entity::EntityStore;
use crate::game::entity::{EntityKind, Order, WeaponSetup};
use crate::game::map::Map;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::scout_plane;
use crate::game::services::spatial::SpatialIndex;
use crate::game::smoke::SmokeCloudStore;
use crate::game::upgrade::UpgradeKind;
use crate::game::PlayerState;
use crate::protocol::Event;

mod armor_reaction;
mod collision;
mod pivot_drive;
mod scout_car;
mod standability;
mod steering;
mod waypoints;

#[cfg(test)]
mod tests;

/// World pixels at which a unit is considered "arrived" at a waypoint / target point.
pub(super) const ARRIVE_EPS: f32 = 2.0;

/// Conservative fallback for broad-phase bounding-box queries if an entity body is unavailable.
pub(super) const MAX_UNIT_BOUNDING_RADIUS_PX: f32 = 32.0;

pub(super) const STEERING_MAX_NEIGHBORS: usize = 16;

const MAGIC_ANCHOR_STATIONARY_PULL_PER_TICK_SCALE: f32 =
    config::EKAT_MAGIC_ANCHOR_PULL_TOWARD_MULTIPLIER - 1.0;

pub(crate) use collision::resolve_collisions;
#[cfg(test)]
use pivot_drive::TANK_BODY_TURN_RATE_RAD_PER_TICK;
pub(crate) use pivot_drive::{angle_delta, rotate_toward};
pub(crate) use standability::is_collision_anchored;

/// Advance every moving unit along its waypoint path at its speed. Clamps the final landing
/// tile to passable terrain (soft overlap with other units is allowed, so we don't resolve
/// unit-unit collisions here). Arriving at the last waypoint of a plain Move clears the order.
#[cfg(test)]
pub(crate) fn movement_system(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
    spatial: &SpatialIndex,
    tick: u32,
) {
    let mut ignored_events = HashMap::new();
    let smokes = SmokeCloudStore::new();
    let ability_runtime = AbilityRuntime::new();
    movement_system_with_events(
        map,
        entities,
        players,
        occ,
        spatial,
        tick,
        &mut ignored_events,
        &smokes,
        &ability_runtime,
    );
}

/// Movement entry point for the real tick loop, with access to transient player events.
#[allow(clippy::too_many_arguments)]
pub(crate) fn movement_system_with_events(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
    spatial: &SpatialIndex,
    tick: u32,
    events: &mut HashMap<u32, Vec<Event>>,
    smokes: &SmokeCloudStore,
    ability_runtime: &AbilityRuntime,
) {
    for id in entities.ids() {
        if let Some(e) = entities.get_mut(id) {
            e.set_movement_delta(0.0, 0.0);
            // Attack orders are stationary intent. Clear any legacy/checkpoint path before the
            // movement pass so no previously authored pursuit state can translate the unit.
            if matches!(e.order(), Order::Attack(_)) {
                e.clear_path();
                e.set_path_goal(None);
            }
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
            .is_some_and(|e| e.is_unit() && smokes.point_inside(e.pos_x, e.pos_y));
        if in_smoke {
            if let Some(e) = entities.get_mut(id) {
                e.mark_in_smoke_for_breakthrough(config::BREAKTHROUGH_RECENT_SMOKE_TICKS);
            }
        }
    }
    scout_plane::advance_scout_planes(map, entities);
    armor_reaction::turn_stationary_tanks_toward_locked_ap_source(
        entities,
        tick,
        |owner, kind, x, y, facing| {
            let out_of_oil = players
                .iter()
                .find(|player| player.id == owner)
                .is_some_and(|player| player.oil == 0);
            !out_of_oil && standability::unit_static_standable(occ, map, kind, x, y, facing)
        },
    );
    waypoints::advance_moving_units(
        map,
        entities,
        players,
        occ,
        spatial,
        tick,
        events,
        ability_runtime,
    );
    apply_magic_anchor_stationary_pull(map, entities, occ, tick, ability_runtime);
    for id in entities.ids() {
        if let Some(e) = entities.get_mut(id) {
            e.tick_breakthrough_status();
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

fn apply_magic_anchor_stationary_pull(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    tick: u32,
    ability_runtime: &AbilityRuntime,
) {
    let world_max = map.world_size_px() - 0.01;
    for id in entities.ids() {
        let Some((kind, x, y, facing, speed, resistance)) = entities.get(id).and_then(|e| {
            if !e.is_unit() || e.kind == EntityKind::ScoutPlane || !e.path_is_empty() {
                return None;
            }
            let profile = standability::footing_profile(e);
            let resistance = standability::footing_resistance(profile);
            if resistance <= 0.0 {
                return None;
            }
            let speed = config::unit_stats(e.kind)?.speed;
            Some((e.kind, e.pos_x, e.pos_y, e.facing(), speed, resistance))
        }) else {
            continue;
        };
        let Some((dir, strength)) = ability_runtime.magic_anchor_stationary_pull(x, y, tick) else {
            continue;
        };
        let pull_px =
            speed * MAGIC_ANCHOR_STATIONARY_PULL_PER_TICK_SCALE * strength / resistance.sqrt();
        if pull_px <= 0.0 || !pull_px.is_finite() {
            continue;
        }
        let next_x = (x + dir.0 * pull_px).clamp(0.0, world_max);
        let next_y = (y + dir.1 * pull_px).clamp(0.0, world_max);
        if !standability::unit_static_standable(occ, map, kind, next_x, next_y, facing) {
            continue;
        }
        if let Some(e) = entities.get_mut(id) {
            e.set_position(next_x, next_y);
            e.set_movement_delta(next_x - x, next_y - y);
        }
    }
}
