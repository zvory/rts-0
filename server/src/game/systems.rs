//! Per-tick simulation systems orchestrator. See `DESIGN.md` §3.
//!
//! [`run_tick`] delegates to the internal services in the order mandated by the design:
//!   1. drain + apply queued commands
//!   2. movement
//!   3. combat
//!   4. gather progression
//!   5. production progression + spawning
//!   6. construction progression
//!   7. deaths
//!   8. unit-unit collision resolution (hard non-stacking; runs after spawning so newly
//!      produced units that land on the same spawn point are unstacked in the same tick)
//!   9. recompute supply cap

use std::collections::HashMap;

use crate::game::entity::EntityStore;
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services;
use crate::game::services::pathing::PathingService;
use crate::game::services::spatial::SpatialIndex;
use crate::game::PlayerState;
use crate::protocol::{Command, Event};

/// Run all per-tick systems in order. `events` is the per-player event accumulator (already
/// keyed for every player). `tick` is the new tick number (post-increment).
///
/// Returns the [`SpatialIndex`] built from the post-tick entity positions so the snapshot layer
/// can use it for interest filtering without rebuilding.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_tick(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    fog: &Fog,
    pathing: &mut PathingService,
    pending: Vec<(u32, Command)>,
    events: &mut HashMap<u32, Vec<Event>>,
    tick: u32,
) -> SpatialIndex {
    // Build occupancy once up front; commands that need pathing reuse it.
    let occ = services::occupancy::Occupancy::build(map, entities);
    // Pre-tick spatial index for commands (building placement checks).
    let spatial = services::spatial::SpatialIndex::build(entities, map.size);

    let mut coordinator =
        services::move_coordinator::MoveCoordinator::new(pathing, map, &occ, tick);

    services::commands::apply_commands(
        map,
        entities,
        players,
        &occ,
        &spatial,
        &mut coordinator,
        pending,
        events,
    );
    coordinator.process_awaiting_paths(entities);
    services::movement::movement_system(map, entities, &occ);

    // Rebuild after movement so combat, gather, and collision resolution see updated positions.
    let spatial = services::spatial::SpatialIndex::build(entities, map.size);

    services::combat::combat_system(map, entities, &occ, &spatial, &mut coordinator, events);
    services::economy::gather_system(map, entities, players, &occ, &spatial, &mut coordinator);
    services::production::production_system(map, entities, &coordinator, events);
    services::construction::construction_system(map, entities, events);
    services::death::death_system(entities, fog, events);

    // Collision resolution runs after production so newly-spawned units (which can land on
    // the same spawn point as their predecessors) are unstacked in the same tick they appear.
    // Rebuild the spatial index first so the resolver sees the post-production entity set.
    let spatial = services::spatial::SpatialIndex::build(entities, map.size);
    services::movement::resolve_collisions(entities, &spatial, map, &occ);

    services::supply::recompute_supply(players, entities);

    // Rebuild after all mutations so the returned index reflects the final positions.
    services::spatial::SpatialIndex::build(entities, map.size)
}

// Re-exports for callers outside the services layer so the public surface of `systems` stays
// stable.
pub(crate) use services::occupancy::footprint_placeable;
pub(crate) use services::supply::recompute_supply;
