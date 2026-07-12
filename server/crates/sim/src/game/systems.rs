//! Per-tick simulation systems orchestrator. See `docs/design/server-sim.md`.
//!
//! [`run_tick`] delegates to the internal services through explicit derived-state boundaries:
//!   1. refresh pre-command occupancy/spatial indexes
//!   2. drain + apply queued commands
//!   3. start ready deferred production requests
//!   4. movement
//!   5. promote queued orders made ready by movement or previous-tick active-order cleanup
//!   6. refresh post-movement occupancy/spatial indexes
//!   7. combat
//!   8. gather progression
//!   9. production progression + spawning
//!   10. player-global ability cooldown progression
//!   11. construction progression
//!   12. deconstruction progression
//!   13. ability projectile/runtime progression
//!   14. deaths
//!   15. refresh pre-collision occupancy/spatial indexes
//!   16. unit-unit collision resolution (hard non-stacking; runs after spawning so newly
//!       produced units that land on the same spawn point are unstacked in the same tick)
//!   17. trench occupation, slotting, and dig-in progress
//!   18. recompute supply cap
//!   19. rebuild final spatial index for snapshot interest filtering
//!
//! The three occupancy boundaries compare the exact static-building topology and share immutable
//! clearance data when it is unchanged. Spatial indexes still rebuild at every named boundary.

use std::collections::{BTreeSet, HashMap};

mod occupancy_phase_cache;

use crate::game::ability_runtime::AbilityRuntime;
use crate::game::artillery::ArtilleryShellStore;
use crate::game::entity::EntityStore;
use crate::game::firing_reveal::FiringRevealSource;
use crate::game::fog::{Fog, LingeringSightSource};
use crate::game::map::Map;
use crate::game::mortar::MortarShellStore;
use crate::game::panzerfaust_shot::PanzerfaustShotStore;
use crate::game::services;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::pathing::PathingService;
use crate::game::services::spatial::SpatialIndex;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::game::trench::TrenchStore;
use crate::game::upgrade::UpgradeKind;
use crate::game::PlayerState;
use crate::protocol::Event;
use rand::Rng;

use occupancy_phase_cache::OccupancyPhaseCache;

/// Derived state valid before commands mutate orders or units move.
///
/// This state is intentionally phase-specific: adding a system after movement should require
/// choosing a post-movement state instead of accidentally reusing this one.
struct PreCommandDerivedState<'a> {
    occupancy: Occupancy<'a>,
    spatial: SpatialIndex,
}

impl<'a> PreCommandDerivedState<'a> {
    fn rebuild(
        map: &'a Map,
        entities: &EntityStore,
        occupancy_cache: &mut OccupancyPhaseCache<'a>,
    ) -> Self {
        PreCommandDerivedState {
            occupancy: occupancy_cache.snapshot(entities),
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
    fn rebuild(
        map: &'a Map,
        entities: &EntityStore,
        occupancy_cache: &mut OccupancyPhaseCache<'a>,
    ) -> Self {
        PostMovementDerivedState {
            occupancy: occupancy_cache.snapshot(entities),
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
    fn rebuild(
        map: &'a Map,
        entities: &EntityStore,
        occupancy_cache: &mut OccupancyPhaseCache<'a>,
    ) -> Self {
        PreCollisionDerivedState {
            occupancy: occupancy_cache.snapshot(entities),
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
    active_vision_players: &BTreeSet<u32>,
    fog: &Fog,
    pathing: &mut PathingService,
    rng: &mut impl Rng,
    lingering_sight: &mut Vec<LingeringSightSource>,
    firing_reveals: &mut Vec<FiringRevealSource>,
    smokes: &mut SmokeCloudStore,
    trenches: &mut TrenchStore,
    ability_runtime: &mut AbilityRuntime,
    mortar_shells: &mut MortarShellStore,
    artillery_shells: &mut ArtilleryShellStore,
    panzerfaust_shots: &mut PanzerfaustShotStore,
    active_construction_sites: &mut BTreeSet<u32>,
    pending: Vec<crate::game::commands::PendingCommand>,
    events: &mut HashMap<u32, Vec<Event>>,
    tick: u32,
    mut perf: Option<&mut crate::perf::TickPerf>,
) -> SpatialIndex {
    let mut occupancy_cache = OccupancyPhaseCache::new(map);
    let pre_command = crate::perf::timed(perf.as_deref_mut(), "pre_command_derived", || {
        PreCommandDerivedState::rebuild(map, entities, &mut occupancy_cache)
    });
    let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
    let mut coordinator = crate::perf::timed(perf.as_deref_mut(), "move_coordinator_new", || {
        services::move_coordinator::MoveCoordinator::new_with_teams(
            pathing,
            map,
            &pre_command.occupancy,
            tick,
            teams.clone(),
        )
    });
    coordinator.enable_trench_formation_preference(
        entities,
        trenches,
        fog,
        smokes,
        players.iter().map(|player| player.id),
        active_vision_players,
    );
    if perf.is_some() {
        coordinator.enable_diagnostics();
    }

    crate::perf::timed(perf.as_deref_mut(), "apply_commands", || {
        services::commands::apply_commands(
            map,
            entities,
            players,
            &pre_command.spatial,
            &mut coordinator,
            fog,
            smokes,
            ability_runtime,
            mortar_shells,
            artillery_shells,
            firing_reveals,
            pending,
            events,
            tick,
        );
    });
    crate::perf::timed(perf.as_deref_mut(), "production_queue", || {
        services::production_queue::run_scheduler(entities, players);
    });
    crate::perf::timed(
        perf.as_deref_mut(),
        "spawn_due_smokes_after_commands",
        || {
            smokes.spawn_due(tick);
        },
    );
    coordinator.begin_pathing_diagnostics("awaiting_paths", entities);
    crate::perf::timed(perf.as_deref_mut(), "awaiting_paths", || {
        coordinator.process_awaiting_paths(entities);
    });
    if let Some(record) = coordinator.finish_pathing_diagnostics(entities) {
        if let Some(perf) = perf.as_deref_mut() {
            perf.record_pathing(record);
        }
    }
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
            ability_runtime,
        );
    });
    coordinator.begin_pathing_diagnostics("promote_queued_orders", entities);
    crate::perf::timed(perf.as_deref_mut(), "promote_queued_orders", || {
        services::order_queue::promote_ready_orders(
            map,
            entities,
            players,
            fog,
            &mut coordinator,
            smokes,
            ability_runtime,
            mortar_shells,
            events,
            tick,
        );
    });
    if let Some(record) = coordinator.finish_pathing_diagnostics(entities) {
        if let Some(perf) = perf.as_deref_mut() {
            perf.record_pathing(record);
        }
    }
    crate::perf::timed(
        perf.as_deref_mut(),
        "spawn_due_smokes_after_promotions",
        || {
            smokes.spawn_due(tick);
        },
    );
    coordinator.begin_pathing_diagnostics("promoted_awaiting_paths", entities);
    crate::perf::timed(perf.as_deref_mut(), "promoted_awaiting_paths", || {
        coordinator.process_awaiting_paths(entities);
    });
    if let Some(record) = coordinator.finish_pathing_diagnostics(entities) {
        if let Some(perf) = perf.as_deref_mut() {
            perf.record_pathing(record);
        }
    }

    let post_movement = crate::perf::timed(perf.as_deref_mut(), "post_movement_derived", || {
        PostMovementDerivedState::rebuild(map, entities, &mut occupancy_cache)
    });

    crate::perf::timed(perf.as_deref_mut(), "combat", || {
        let mortar_autocast_researched = |owner| {
            players
                .iter()
                .any(|p| p.id == owner && p.upgrades.contains(&UpgradeKind::MortarAutocast))
        };
        let methamphetamines_researched = |owner| {
            players
                .iter()
                .any(|p| p.id == owner && p.has_upgrade(UpgradeKind::Methamphetamines))
        };
        services::combat::combat_system(
            map,
            entities,
            &teams,
            &mortar_autocast_researched,
            &methamphetamines_researched,
            &post_movement.occupancy,
            &post_movement.spatial,
            &mut coordinator,
            fog,
            smokes,
            mortar_shells,
            panzerfaust_shots,
            rng,
            events,
            firing_reveals,
            tick,
        );
    });
    crate::perf::timed(perf.as_deref_mut(), "artillery_point_fire", || {
        services::commands::artillery_point_fire_system(
            map,
            entities,
            players,
            artillery_shells,
            firing_reveals,
            events,
            fog,
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
            tick,
        );
    });
    crate::perf::timed(perf.as_deref_mut(), "production", || {
        services::production::production_system(map, entities, players, &mut coordinator, events);
    });
    crate::perf::timed(perf.as_deref_mut(), "player_ability_cooldowns", || {
        for player in players.iter_mut() {
            player.ability_cooldowns.retain(|_, ticks| {
                *ticks = ticks.saturating_sub(1);
                *ticks > 0
            });
        }
    });
    crate::perf::timed(perf.as_deref_mut(), "construction", || {
        services::construction::construction_system(
            map,
            entities,
            players,
            events,
            fog,
            active_construction_sites,
        );
    });
    crate::perf::timed(perf.as_deref_mut(), "deconstruction", || {
        services::construction::deconstruction_system(entities, players);
    });
    crate::perf::timed(perf.as_deref_mut(), "mortar_impacts", || {
        let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
        mortar_shells.resolve_due(entities, &teams, fog, events, firing_reveals, tick);
    });
    crate::perf::timed(perf.as_deref_mut(), "artillery_impacts", || {
        let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
        artillery_shells.resolve_due(entities, &teams, fog, events, tick);
    });
    crate::perf::timed(perf.as_deref_mut(), "ability_runtime", || {
        let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
        ability_runtime.tick_projectiles(entities, &teams, &post_movement.spatial, tick);
        ability_runtime.tick(entities, tick);
    });
    crate::perf::timed(perf.as_deref_mut(), "death", || {
        let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
        services::death::death_system(
            entities,
            fog,
            smokes,
            &teams,
            players,
            lingering_sight,
            events,
            tick,
        );
    });

    let pre_collision = crate::perf::timed(perf.as_deref_mut(), "pre_collision_derived", || {
        PreCollisionDerivedState::rebuild(map, entities, &mut occupancy_cache)
    });
    let pre_collision_positions =
        crate::perf::timed(perf.as_deref_mut(), "pre_collision_positions", || {
            entities
                .iter()
                .filter(|entity| entity.is_unit())
                .map(|entity| (entity.id, (entity.pos_x, entity.pos_y)))
                .collect::<HashMap<_, _>>()
        });
    crate::perf::timed(perf.as_deref_mut(), "collision", || {
        services::movement::resolve_collisions(
            entities,
            &pre_collision.spatial,
            map,
            &pre_collision.occupancy,
        );
    });

    crate::perf::timed(perf.as_deref_mut(), "entrenchment", || {
        let entrenchment_researched = |owner| {
            players
                .iter()
                .any(|p| p.id == owner && p.has_upgrade(UpgradeKind::Entrenchment))
        };
        let pre_collision_position = |entity_id| pre_collision_positions.get(&entity_id).copied();
        services::entrenchment::entrenchment_system(
            map,
            entities,
            &entrenchment_researched,
            &pre_collision_position,
            &pre_collision.occupancy,
            trenches,
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
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![(4, 4)],
            base_sites: Vec::new(),
        }
    }

    fn player_state(id: u32) -> PlayerState {
        PlayerState {
            id,
            team_id: id,
            faction_id: "kriegsia".to_string(),
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
            ability_cooldowns: Default::default(),
            production_requests: Default::default(),
        }
    }

    #[test]
    fn construction_waits_on_unit_body_before_collision_cleanup() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let mut players = vec![player_state(1)];
        let fog = Fog::new(map.size);
        let mut pathing = PathingService::new(1024, 16);
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        let mut lingering_sight = Vec::new();
        let mut firing_reveals = Vec::new();
        let mut smokes = SmokeCloudStore::new();
        let mut trenches = TrenchStore::new();
        let mut ability_runtime = AbilityRuntime::new();
        let mut mortar_shells = MortarShellStore::default();
        let mut artillery_shells = ArtilleryShellStore::default();
        let mut panzerfaust_shots = PanzerfaustShotStore::default();
        let mut active_construction_sites = BTreeSet::new();
        let active_vision_players = BTreeSet::new();

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
            &active_vision_players,
            &fog,
            &mut pathing,
            &mut SmallRng::seed_from_u64(0),
            &mut lingering_sight,
            &mut firing_reveals,
            &mut smokes,
            &mut trenches,
            &mut ability_runtime,
            &mut mortar_shells,
            &mut artillery_shells,
            &mut panzerfaust_shots,
            &mut active_construction_sites,
            Vec::new(),
            &mut events,
            1,
            None,
        );

        assert!(
            entities.iter().all(|e| e.kind != EntityKind::Depot),
            "construction should not spawn the scaffold before collision cleanup while a unit body intersects the footprint"
        );
        assert!(
            matches!(
                entities
                    .get(worker)
                    .expect("worker should survive")
                    .build_phase(),
                Some(crate::game::entity::BuildPhase::WaitingAtSite)
            ),
            "blocked construction should keep the worker order during the grace window"
        );
        assert!(
            entities.get(blocker).is_some(),
            "blocking unit should survive the delayed construction"
        );
        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "unit-blocked construction should not notify the owner before timeout"
        );
    }
}
