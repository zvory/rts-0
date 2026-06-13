//! Per-tick simulation systems orchestrator. See `docs/design/server-sim.md`.
//!
//! [`run_tick`] delegates to the internal services through explicit derived-state boundaries:
//!   1. rebuild pre-command occupancy/spatial indexes
//!   2. drain + apply queued commands
//!   3. movement
//!   4. rebuild post-movement occupancy/spatial indexes
//!   5. combat
//!   6. gather progression
//!   7. production progression + spawning
//!   8. construction progression
//!   9. deaths
//!   10. rebuild pre-collision occupancy/spatial indexes
//!   11. unit-unit collision resolution (hard non-stacking; runs after spawning so newly
//!       produced units that land on the same spawn point are unstacked in the same tick)
//!   12. recompute supply cap
//!   13. rebuild final spatial index for snapshot interest filtering

use std::collections::HashMap;

use crate::game::artillery::ArtilleryShellStore;
use crate::game::command::SimCommand;
use crate::game::entity::EntityStore;
use crate::game::fog::{Fog, LingeringSightSource};
use crate::game::map::Map;
use crate::game::mortar::MortarShellStore;
use crate::game::services;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::pathing::PathingService;
use crate::game::services::spatial::SpatialIndex;
use crate::game::smoke::SmokeCloudStore;
use crate::game::upgrade::UpgradeKind;
use crate::game::PlayerState;
use crate::protocol::Event;
use rand::rngs::SmallRng;

/// Derived state valid before commands mutate orders or units move.
///
/// This state is intentionally phase-specific: adding a system after movement should require
/// choosing a post-movement state instead of accidentally reusing this one.
struct PreCommandDerivedState<'a> {
    occupancy: Occupancy<'a>,
    spatial: SpatialIndex,
}

impl<'a> PreCommandDerivedState<'a> {
    fn rebuild(map: &'a Map, entities: &EntityStore) -> Self {
        PreCommandDerivedState {
            occupancy: Occupancy::build(map, entities),
            spatial: SpatialIndex::build(entities, map.size),
        }
    }
}

/// Derived state valid after movement has updated unit positions.
struct PostMovementDerivedState<'a> {
    occupancy: Occupancy<'a>,
    spatial: SpatialIndex,
}

impl<'a> PostMovementDerivedState<'a> {
    fn rebuild(map: &'a Map, entities: &EntityStore) -> Self {
        PostMovementDerivedState {
            occupancy: Occupancy::build(map, entities),
            spatial: SpatialIndex::build(entities, map.size),
        }
    }
}

/// Derived state valid after combat/economy/production/construction/death mutations and before
/// collision resolution changes unit positions.
struct PreCollisionDerivedState<'a> {
    occupancy: Occupancy<'a>,
    spatial: SpatialIndex,
}

impl<'a> PreCollisionDerivedState<'a> {
    fn rebuild(map: &'a Map, entities: &EntityStore) -> Self {
        PreCollisionDerivedState {
            occupancy: Occupancy::build(map, entities),
            spatial: SpatialIndex::build(entities, map.size),
        }
    }
}

/// Derived state valid after every tick mutation. This is the state handed back to `Game` for
/// fog-filtered snapshot interest queries.
struct FinalDerivedState {
    spatial: SpatialIndex,
}

impl FinalDerivedState {
    fn rebuild(map: &Map, entities: &EntityStore) -> Self {
        FinalDerivedState {
            spatial: SpatialIndex::build(entities, map.size),
        }
    }
}

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
    lingering_sight: &mut Vec<LingeringSightSource>,
    smokes: &mut SmokeCloudStore,
    mortar_shells: &mut MortarShellStore,
    artillery_shells: &mut ArtilleryShellStore,
    pending: Vec<(u32, SimCommand)>,
    events: &mut HashMap<u32, Vec<Event>>,
    tick: u32,
    mut perf: Option<&mut crate::perf::TickPerf>,
) -> SpatialIndex {
    let pre_command = crate::perf::timed(perf.as_deref_mut(), "pre_command_derived", || {
        PreCommandDerivedState::rebuild(map, entities)
    });
    let mut coordinator = crate::perf::timed(perf.as_deref_mut(), "move_coordinator_new", || {
        services::move_coordinator::MoveCoordinator::new(pathing, map, &pre_command.occupancy, tick)
    });

    crate::perf::timed(perf.as_deref_mut(), "apply_commands", || {
        services::commands::apply_commands(
            map,
            entities,
            players,
            &pre_command.spatial,
            &mut coordinator,
            fog,
            smokes,
            mortar_shells,
            artillery_shells,
            pending,
            events,
            tick,
        );
    });
    crate::perf::timed(
        perf.as_deref_mut(),
        "spawn_due_smokes_after_commands",
        || {
            smokes.spawn_due(tick);
        },
    );
    crate::perf::timed(perf.as_deref_mut(), "awaiting_paths", || {
        coordinator.process_awaiting_paths(entities);
    });
    crate::perf::timed(perf.as_deref_mut(), "movement", || {
        services::movement::movement_system_with_events(
            map,
            entities,
            players,
            &pre_command.occupancy,
            &pre_command.spatial,
            tick,
            events,
            smokes,
        );
    });
    crate::perf::timed(perf.as_deref_mut(), "promote_queued_orders", || {
        services::order_queue::promote_ready_orders(
            map,
            entities,
            players,
            fog,
            &mut coordinator,
            smokes,
            mortar_shells,
            events,
            tick,
        );
    });
    crate::perf::timed(
        perf.as_deref_mut(),
        "spawn_due_smokes_after_promotions",
        || {
            smokes.spawn_due(tick);
        },
    );
    crate::perf::timed(perf.as_deref_mut(), "promoted_awaiting_paths", || {
        coordinator.process_awaiting_paths(entities);
    });

    let post_movement = crate::perf::timed(perf.as_deref_mut(), "post_movement_derived", || {
        PostMovementDerivedState::rebuild(map, entities)
    });

    crate::perf::timed(perf.as_deref_mut(), "combat", || {
        let mortar_autocast_researched = |owner| {
            players
                .iter()
                .any(|p| p.id == owner && p.upgrades.contains(&UpgradeKind::MortarAutocast))
        };
        services::combat::combat_system(
            map,
            entities,
            &mortar_autocast_researched,
            &post_movement.occupancy,
            &post_movement.spatial,
            &mut coordinator,
            fog,
            smokes,
            mortar_shells,
            rng,
            events,
            tick,
        );
    });
    crate::perf::timed(perf.as_deref_mut(), "artillery_point_fire", || {
        services::commands::artillery_point_fire_system(
            map,
            entities,
            players,
            artillery_shells,
            events,
            tick,
        );
    });
    crate::perf::timed(perf.as_deref_mut(), "economy", || {
        services::economy::gather_system(
            map,
            entities,
            players,
            &post_movement.occupancy,
            &post_movement.spatial,
            &mut coordinator,
        );
    });
    crate::perf::timed(perf.as_deref_mut(), "production", || {
        services::production::production_system(map, entities, players, &mut coordinator, events);
    });
    crate::perf::timed(perf.as_deref_mut(), "construction", || {
        services::construction::construction_system(map, entities, players, events);
    });
    crate::perf::timed(perf.as_deref_mut(), "mortar_impacts", || {
        mortar_shells.resolve_due(map, entities, fog, events, tick);
    });
    crate::perf::timed(perf.as_deref_mut(), "artillery_impacts", || {
        artillery_shells.resolve_due(entities, events, tick);
    });
    crate::perf::timed(perf.as_deref_mut(), "death", || {
        services::death::death_system(
            entities,
            fog,
            smokes,
            players,
            lingering_sight,
            events,
            tick,
        );
    });

    let pre_collision = crate::perf::timed(perf.as_deref_mut(), "pre_collision_derived", || {
        PreCollisionDerivedState::rebuild(map, entities)
    });
    crate::perf::timed(perf.as_deref_mut(), "collision", || {
        services::movement::resolve_collisions(
            entities,
            &pre_collision.spatial,
            map,
            &pre_collision.occupancy,
        );
    });

    crate::perf::timed(perf.as_deref_mut(), "supply", || {
        services::supply::recompute_supply(players, entities);
    });

    crate::perf::timed(perf, "final_derived", || {
        FinalDerivedState::rebuild(map, entities).spatial
    })
}

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
            team_id: id,
            name: format!("Player {id}"),
            color: "#fff".to_string(),
            start_tile: (4, 4),
            steel: 1_000,
            oil: 1_000,
            supply_used: 0,
            supply_cap: 20,
            is_ai: false,
            score: crate::game::ScoreState::default(),
            upgrades: Default::default(),
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
        let mut lingering_sight = Vec::new();
        let mut smokes = SmokeCloudStore::new();
        let mut mortar_shells = MortarShellStore::default();
        let mut artillery_shells = ArtilleryShellStore::default();

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
            &mut lingering_sight,
            &mut smokes,
            &mut mortar_shells,
            &mut artillery_shells,
            Vec::new(),
            &mut events,
            1,
            None,
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
                .is_some_and(|events| matches!(events.as_slice(), [Event::Notice { msg, .. }] if msg == "Cannot build there")),
            "rejected construction should notify the owner"
        );
    }
}
