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
use rand::rngs::SmallRng;

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
    rng: &mut SmallRng,
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
        &spatial,
        &mut coordinator,
        pending,
        events,
    );
    coordinator.process_awaiting_paths(entities);
    services::movement::movement_system(map, entities, &occ, &spatial, tick);

    // Rebuild after movement so combat, gather, and collision resolution see updated positions.
    let spatial = services::spatial::SpatialIndex::build(entities, map.size);

    services::combat::combat_system(
        map,
        entities,
        &occ,
        &spatial,
        &mut coordinator,
        fog,
        rng,
        events,
    );
    services::economy::gather_system(map, entities, players, &occ, &spatial, &mut coordinator);
    services::production::production_system(map, entities, players, &coordinator, events);
    services::construction::construction_system(map, entities, players, &spatial, events);
    services::death::death_system(entities, fog, players, events);

    // Collision resolution runs after production/construction/deaths so spawned units are
    // unstacked in the same tick and pushes respect the current building footprint set.
    let collision_occ = services::occupancy::Occupancy::build(map, entities);
    // Rebuild the spatial index first so the resolver sees the post-production entity set.
    let spatial = services::spatial::SpatialIndex::build(entities, map.size);
    services::movement::resolve_collisions(entities, &spatial, map, &collision_occ);

    services::supply::recompute_supply(players, entities);

    // Rebuild after all mutations so the returned index reflects the final positions.
    services::spatial::SpatialIndex::build(entities, map.size)
}

// Re-exports for callers outside the services layer so the public surface of `systems` stays
// stable.
pub(crate) use services::occupancy::footprint_placeable;
pub(crate) use services::supply::recompute_supply;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::game::entity::{EntityKind, Order};
    use crate::game::fog::Fog;
    use crate::game::map::Map;
    use crate::protocol::terrain;
    use rand::SeedableRng;

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![(4, 4)],
            expansion_sites: Vec::new(),
        }
    }

    fn player_state(id: u32) -> PlayerState {
        PlayerState {
            id,
            name: format!("Player {id}"),
            color: "#fff".to_string(),
            start_tile: (4, 4),
            steel: 1_000,
            oil: 1_000,
            supply_used: 0,
            supply_cap: 20,
            is_ai: false,
            score: crate::game::ScoreState::default(),
        }
    }

    #[test]
    fn construction_rejects_unit_body_before_collision_cleanup() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let mut players = vec![player_state(1)];
        let fog = Fog::new(map.size);
        let mut pathing = PathingService::new(1024, 16);
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();

        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 400.0, 390.0)
            .expect("worker kind should be valid");
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .set_order(Order::build(EntityKind::Depot, 10, 10));

        let blocker = entities
            .spawn_unit(1, EntityKind::Rifleman, 386.0, 336.0)
            .expect("rifleman kind should be valid");
        entities
            .spawn_unit(1, EntityKind::Rifleman, 387.0, 336.0)
            .expect("rifleman kind should be valid");
        assert_eq!(map.tile_of(386.0, 336.0), (12, 10));

        run_tick(
            &map,
            &mut entities,
            &mut players,
            &fog,
            &mut pathing,
            &mut SmallRng::seed_from_u64(0),
            Vec::new(),
            &mut events,
            1,
        );

        assert!(
            entities.iter().all(|e| e.kind != EntityKind::Depot),
            "construction should reject the scaffold before collision cleanup when a unit body intersects the footprint"
        );
        assert!(
            matches!(
                entities.get(worker).expect("worker should survive").order(),
                Order::Idle
            ),
            "blocked construction should clear the worker order"
        );
        assert!(
            entities.get(blocker).is_some(),
            "blocking unit should survive the rejected construction"
        );
        assert!(
            events
                .get(&1)
                .is_some_and(|events| matches!(events.as_slice(), [Event::Notice { msg }] if msg == "Cannot build there")),
            "rejected construction should notify the owner"
        );
    }
}
