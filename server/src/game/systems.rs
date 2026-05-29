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
//!   8. recompute supply cap

use std::collections::HashMap;

use crate::game::entity::EntityStore;
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services;
use crate::game::PlayerState;
use crate::protocol::{Command, Event};

/// Run all per-tick systems in order. `events` is the per-player event accumulator (already
/// keyed for every player). `tick` is the new tick number (post-increment).
pub(crate) fn run_tick(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    fog: &Fog,
    pending: Vec<(u32, Command)>,
    events: &mut HashMap<u32, Vec<Event>>,
    _tick: u32,
) {
    // Build occupancy once up front; commands that need pathing reuse it.
    let occ = services::occupancy::Occupancy::build(map, entities);

    services::commands::apply_commands(map, entities, players, &occ, pending, events);
    services::movement::movement_system(map, entities, &occ);
    services::combat::combat_system(map, entities, &occ, events);
    services::economy::gather_system(map, entities, players, &occ);
    services::production::production_system(map, entities, players, events);
    services::construction::construction_system(entities, events);
    services::death::death_system(entities, fog, events);
    services::supply::recompute_supply(players, entities);
}

// Re-exports for callers outside the services layer so the public surface of `systems` stays
// stable.
pub(crate) use services::occupancy::footprint_placeable;
pub(crate) use services::supply::recompute_supply;
