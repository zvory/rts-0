//! Unit movement service.
//!
//! This module owns per-tick waypoint advancement and collision cleanup for mobile entities.
//! Path requests are prepared by `move_coordinator`, but landing legality, vehicle steering,
//! weapon setup restrictions, and final overlap resolution live here so the tick pipeline has a
//! single movement phase.

use std::collections::HashMap;

use crate::game::entity::EntityStore;
use crate::game::map::Map;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::PlayerState;
use crate::protocol::Event;

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
    movement_system_with_events(
        map,
        entities,
        players,
        occ,
        spatial,
        tick,
        &mut ignored_events,
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
) {
    waypoints::advance_moving_units(map, entities, players, occ, spatial, tick, events);
    for id in entities.ids() {
        if let Some(e) = entities.get_mut(id) {
            e.tick_charge();
            e.tick_charge_cooldown();
        }
    }
}
