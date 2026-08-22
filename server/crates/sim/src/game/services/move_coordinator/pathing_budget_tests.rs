use super::*;
use crate::game::entity::{MovePhase, Order};
use crate::protocol::terrain;

fn flat_test_map(size: u32) -> Map {
    Map {
        width: size,
        height: size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(1, 1)],
        ..Default::default()
    }
}

fn footprint_attempt(entities: &EntityStore, id: u32) -> Option<u32> {
    let movement = entities.get(id)?.movement.as_ref()?;
    match &movement.order {
        Order::Build(order) => Some(order.execution.routing.attempt),
        Order::Deconstruct(order) => Some(order.execution.routing.attempt),
        _ => None,
    }
}

#[test]
fn rebuildable_cache_does_not_change_tick_path_scheduling() {
    let map = flat_test_map(40);
    let mut entities = EntityStore::new();
    let mut units_and_goals = Vec::new();
    for index in 0..=MAX_REQUESTS_PER_TICK {
        let index = index as u32;
        let start_tile = (3, 3 + index * 2);
        let goal_tile = (32, 32 - index * 2);
        let start = map.tile_center(start_tile.0, start_tile.1);
        let goal = map.tile_center(goal_tile.0, goal_tile.1);
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
            .expect("rifleman should spawn");
        units_and_goals.push((unit, start_tile, goal_tile, goal));
    }
    let occ = Occupancy::build(&map, &entities);
    let mut warm_pathing = PathingService::new(8_192, 256);
    for &(_, start, goal, _) in &units_and_goals {
        let path = warm_pathing.request_tile_path(
            &map,
            &occ,
            PathRequest {
                kind: EntityKind::Rifleman,
                start: (start.0 as i32, start.1 as i32),
                goal: (goal.0 as i32, goal.1 as i32),
                radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
                route_shape: RouteShape::Normal,
                policy: RoutePolicy::LegacyShape,
                budget: None,
            },
        );
        assert!(!path.is_empty());
    }
    assert_eq!(warm_pathing.cache_len(), units_and_goals.len());

    for &(unit, _, _, goal) in &units_and_goals {
        let entity = entities.get_mut(unit).expect("rifleman should remain");
        entity.replace_active_order(Order::ability(
            AbilityKind::Smoke,
            goal.0,
            goal.1,
            goal.0,
            goal.1,
        ));
        entity.set_path_goal(Some(goal));
    }
    let mut warm_entities = entities.clone();
    let mut cold_entities = entities;
    let mut cold_pathing = PathingService::new(8_192, 256);
    warm_pathing.advance_tick(2);
    cold_pathing.advance_tick(2);

    MoveCoordinator::new(&mut warm_pathing, &map, &occ, 2)
        .process_awaiting_paths(&mut warm_entities);
    MoveCoordinator::new(&mut cold_pathing, &map, &occ, 2)
        .process_awaiting_paths(&mut cold_entities);

    let phases = |store: &EntityStore| {
        units_and_goals
            .iter()
            .map(|(unit, _, _, _)| store.get(*unit).and_then(|entity| entity.move_phase()))
            .collect::<Vec<_>>()
    };
    let warm_phases = phases(&warm_entities);
    assert_eq!(warm_phases, phases(&cold_entities));
    assert_eq!(
        warm_phases
            .iter()
            .filter(|phase| **phase == Some(MovePhase::Moving))
            .count(),
        MAX_REQUESTS_PER_TICK
    );
    assert_eq!(
        warm_phases
            .iter()
            .filter(|phase| **phase == Some(MovePhase::AwaitingPath))
            .count(),
        1
    );
}

#[test]
fn cached_heavy_route_consumes_the_remaining_allowance() {
    let map = flat_test_map(16);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(32_768, 16);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

    coordinator.consume_request_budget(Some(PathingRequestDiagnostics {
        cache_status: PathCacheStatus::Hit,
        expanded_nodes: 0,
        scheduling_expanded_nodes: HEAVY_PATH_EXPANSIONS,
        budget_exhausted: false,
        tile_path_len: 80,
    }));

    assert_eq!(coordinator.budget, 0);
}

#[test]
fn footprint_retry_progress_is_independent_of_the_rebuildable_cache() {
    let map = flat_test_map(40);
    let mut entities = EntityStore::new();
    let start = map.tile_center(10, 10);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, start.0, start.1)
        .expect("worker should spawn");
    for (tx, ty) in [(8, 10), (12, 10), (10, 8), (10, 12)] {
        let position = map.tile_center(tx, ty);
        entities
            .spawn_building(1, EntityKind::Depot, position.0, position.1, true)
            .expect("blocking depot should spawn");
    }
    let occ = Occupancy::build(&map, &entities);
    let mut warm_pathing = PathingService::new(8_192, 256);
    warm_pathing.advance_tick(1);
    {
        let mut coordinator = MoveCoordinator::new(&mut warm_pathing, &map, &occ, 1);
        assert!(
            coordinator.order_build(&mut entities, worker, EntityKind::Depot, 30, 30),
            "bounded routing should preserve the build intent for a later candidate"
        );
    }
    let initial_attempt =
        footprint_attempt(&entities, worker).expect("build order should retain routing progress");
    assert_eq!(initial_attempt, MAX_REQUESTS_PER_TICK as u32);

    let mut warm_entities = entities.clone();
    let mut cold_entities = entities;
    let mut cold_pathing = PathingService::new(8_192, 256);
    warm_pathing.advance_tick(2);
    cold_pathing.advance_tick(2);
    MoveCoordinator::new(&mut warm_pathing, &map, &occ, 2)
        .process_awaiting_paths(&mut warm_entities);
    MoveCoordinator::new(&mut cold_pathing, &map, &occ, 2)
        .process_awaiting_paths(&mut cold_entities);

    let warm_attempt = footprint_attempt(&warm_entities, worker);
    assert_eq!(warm_attempt, footprint_attempt(&cold_entities, worker));
    assert_eq!(
        warm_entities
            .get(worker)
            .and_then(|entity| entity.move_phase()),
        cold_entities
            .get(worker)
            .and_then(|entity| entity.move_phase())
    );
    if let Some(warm_attempt) = warm_attempt {
        assert!(warm_attempt > initial_attempt);
    }
}

#[test]
fn path_failed_is_set_on_unreachable_imported_goal() {
    let map = Map::generate(1, 0x1234_5678);
    let mut entities = EntityStore::new();
    let start = map.tile_center(10, 10);
    let unit_id = entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .expect("rifleman should spawn");
    for tile in [(8, 10), (12, 10), (10, 8), (10, 12)] {
        let center = map.tile_center(tile.0, tile.1);
        entities
            .spawn_building(1, EntityKind::Depot, center.0, center.1, true)
            .expect("blocking depot should spawn");
    }
    let goal = map.tile_center(30, 30);
    let unit = entities.get_mut(unit_id).expect("rifleman should remain");
    unit.replace_active_order(Order::move_to(goal.0, goal.1));
    unit.set_path_goal(Some(goal));
    unit.mark_move_phase(MovePhase::AwaitingPath);

    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(8_192, 256);
    pathing.advance_tick(1);
    MoveCoordinator::new(&mut pathing, &map, &occ, 1).process_awaiting_paths(&mut entities);

    let unit = entities.get(unit_id).expect("rifleman should remain");
    assert_eq!(unit.move_phase(), Some(MovePhase::PathFailed));
    assert!(unit.path_is_empty());
}

#[test]
fn forty_non_heavy_infantry_routes_resolve_within_five_ticks() {
    let map = flat_test_map(80);
    let mut entities = EntityStore::new();
    let mut ids = Vec::new();
    for index in 0..40 {
        let start = map.tile_center(4 + index % 8, 4 + index / 8);
        ids.push(
            entities
                .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
                .expect("rifleman should spawn"),
        );
    }
    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(32_768, 256);
    for tick in 1..=5 {
        pathing.advance_tick(tick);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, tick);
        if tick == 1 {
            coordinator.order_group_move(&mut entities, 1, &ids, map.tile_center(55, 55), false);
        }
        coordinator.process_awaiting_paths(&mut entities);
    }

    let failures = ids
        .iter()
        .filter_map(|&id| {
            let entity = entities.get(id)?;
            (entity.move_phase() == Some(MovePhase::AwaitingPath)
                || entity.path_policy() != RoutePolicy::FastestTerrainTime)
                .then_some((id, entity.move_phase(), entity.path_policy()))
        })
        .collect::<Vec<_>>();
    assert!(failures.is_empty(), "unresolved routes: {failures:?}");
}

#[test]
fn forty_non_heavy_vehicle_routes_resolve_within_five_ticks() {
    for kind in [EntityKind::ScoutCar, EntityKind::Tank] {
        let map = flat_test_map(80);
        let mut entities = EntityStore::new();
        let mut ids = Vec::new();
        for index in 0..40 {
            let start = map.tile_center(4 + index % 8 * 2, 4 + index / 8 * 2);
            ids.push(
                entities
                    .spawn_unit(1, kind, start.0, start.1)
                    .expect("vehicle should spawn"),
            );
        }
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(32_768, 256);
        for tick in 1..=5 {
            pathing.advance_tick(tick);
            let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, tick);
            if tick == 1 {
                coordinator.order_group_move(
                    &mut entities,
                    1,
                    &ids,
                    map.tile_center(45, 45),
                    false,
                );
            }
            coordinator.process_awaiting_paths(&mut entities);
        }

        let failures = ids
            .iter()
            .filter_map(|&id| {
                let entity = entities.get(id)?;
                (entity.move_phase() == Some(MovePhase::AwaitingPath)
                    || entity.path_policy() != RoutePolicy::FastestTerrainTime)
                    .then_some((id, entity.move_phase(), entity.path_is_empty()))
            })
            .collect::<Vec<_>>();
        assert!(
            failures.is_empty(),
            "{kind:?} unresolved routes: {failures:?}"
        );
    }
}
