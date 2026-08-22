use super::*;
use crate::config;
use crate::game::entity::EntityStore;
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

fn map_with_rock_wall(size: u32, wall_x: u32, min_y: u32, max_y: u32) -> Map {
    let mut map = flat_test_map(size);
    for ty in min_y..=max_y {
        let index = map.index(wall_x, ty);
        map.terrain[index] = terrain::ROCK;
    }
    map
}

fn mark_slow_rect(map: &mut Map, min_x: u32, max_x: u32, min_y: u32, max_y: u32) {
    map.slow_movement_tiles = (min_y..=max_y)
        .flat_map(|ty| (min_x..=max_x).map(move |tx| (tx, ty)))
        .collect();
    map.slow_movement_tiles.sort_unstable();
}

fn resolved<T>(outcome: PathingRequestOutcome<T>) -> (T, PathingRequestDiagnostics) {
    match outcome {
        PathingRequestOutcome::Resolved { path, diagnostics } => (path, diagnostics),
        PathingRequestOutcome::Deferred => panic!("search should have been permitted"),
    }
}

#[test]
fn request_tile_path_reports_cache_and_complexity_diagnostics() {
    let map = Map::generate(1, 0x1234_5678);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    service.advance_tick(1);
    let req = PathRequest {
        kind: EntityKind::Worker,
        start: (1, 1),
        goal: (8, 8),
        radius_tiles: 0,
        route_shape: RouteShape::Normal,
        policy: RoutePolicy::LegacyShape,
        budget: None,
    };

    let (first_path, first) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req.clone(), true));
    let (second_path, second) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req, true));

    assert_eq!(first.cache_status, PathCacheStatus::Miss);
    assert!(first.expanded_nodes > 0);
    assert!(!first.budget_exhausted);
    assert_eq!(first.tile_path_len, first_path.len());
    assert_eq!(second.cache_status, PathCacheStatus::Hit);
    assert_eq!(second.expanded_nodes, 0);
    assert_eq!(second.scheduling_expanded_nodes, first.expanded_nodes);
    assert_eq!(second.tile_path_len, second_path.len());
    assert_eq!(first_path, second_path);
}

#[test]
fn cloning_pathing_service_does_not_copy_ephemeral_search_capacity() {
    let map = flat_test_map(64);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let req = PathRequest {
        kind: EntityKind::Worker,
        start: (2, 2),
        goal: (55, 55),
        radius_tiles: 0,
        route_shape: RouteShape::Normal,
        policy: RoutePolicy::LegacyShape,
        budget: None,
    };

    assert!(!service.request_tile_path(&map, &occ, req).is_empty());
    assert!(service.search_scratch.retained_capacity() > 0);

    let cloned = service.clone();
    assert_eq!(cloned.search_scratch.retained_capacity(), 0);
    assert_eq!(cloned.cache_len(), service.cache_len());

    service.clear_rebuildable_state();
    assert_eq!(service.search_scratch.retained_capacity(), 0);
    assert_eq!(service.cache_len(), 0);
}

#[test]
fn budget_exhausted_partial_path_is_cached_only_for_the_same_budget() {
    let map = flat_test_map(64);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let req = PathRequest {
        kind: EntityKind::Worker,
        start: (2, 2),
        goal: (55, 55),
        radius_tiles: 0,
        route_shape: RouteShape::Normal,
        policy: RoutePolicy::LegacyShape,
        budget: Some(1),
    };

    let (partial, diagnostics) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req.clone(), true));
    assert!(!partial.is_empty());
    assert!(diagnostics.budget_exhausted);
    assert_eq!(service.cache_len(), 1);

    let (cached_partial, cached) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req.clone(), true));
    assert_eq!(cached_partial, partial);
    assert_eq!(cached.cache_status, PathCacheStatus::Hit);
    assert!(!cached.budget_exhausted);

    let mut full_budget_req = req;
    full_budget_req.budget = None;
    let (complete, full) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, full_budget_req, true));
    assert_eq!(complete.last(), Some(&(55, 55)));
    assert_eq!(full.cache_status, PathCacheStatus::Miss);
    assert!(!full.budget_exhausted);
}

#[test]
fn exact_direct_segment_bypasses_astar() {
    let map = flat_test_map(32);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let req = PathRequest {
        kind: EntityKind::Rifleman,
        start: (3, 4),
        goal: (25, 19),
        radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
        route_shape: RouteShape::Normal,
        policy: RoutePolicy::LegacyShape,
        budget: None,
    };
    let start = map.tile_center(3, 4);
    let goal = map.tile_center(25, 19);

    let (path, diagnostics) =
        resolved(service.request_with_diagnostics(&map, &occ, req, Some((start, goal)), false));

    assert_eq!(path, vec![goal]);
    assert_eq!(diagnostics.cache_status, PathCacheStatus::Bypassed);
    assert_eq!(diagnostics.expanded_nodes, 0);
    assert!(!diagnostics.budget_exhausted);
}

#[test]
fn infantry_direct_segment_crossing_slow_tiles_uses_astar_detour() {
    let mut map = flat_test_map(16);
    mark_slow_rect(&mut map, 4, 8, 8, 8);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let req = PathRequest {
        kind: EntityKind::Rifleman,
        start: (2, 8),
        goal: (11, 8),
        radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
        route_shape: RouteShape::Normal,
        policy: RoutePolicy::LegacyShape,
        budget: None,
    };
    let start = map.tile_center(2, 8);
    let goal = map.tile_center(11, 8);

    let (path, diagnostics) =
        resolved(service.request_with_diagnostics(&map, &occ, req, Some((start, goal)), true));

    assert_eq!(path.first(), Some(&goal));
    assert!(diagnostics.expanded_nodes > 0);
    assert!(path.len() > 1);
    assert!(path.iter().all(|&(x, y)| {
        let tile = map.tile_of(x, y);
        !map.is_slow_movement_tile(tile.0, tile.1)
    }));
}

#[test]
fn infantry_uses_slow_tiles_when_open_detour_is_longer_and_caches_route() {
    let mut map = flat_test_map(16);
    mark_slow_rect(&mut map, 4, 8, 2, 14);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let req = PathRequest {
        kind: EntityKind::Rifleman,
        start: (2, 8),
        goal: (11, 8),
        radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
        route_shape: RouteShape::Normal,
        policy: RoutePolicy::LegacyShape,
        budget: None,
    };

    let (first_path, first) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req.clone(), true));
    let (cached_path, cached) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req, true));

    assert_eq!(first.cache_status, PathCacheStatus::Miss);
    assert_eq!(cached.cache_status, PathCacheStatus::Hit);
    assert_eq!(cached_path, first_path);
    assert!(first_path
        .iter()
        .any(|&(tx, ty)| map.is_slow_movement_tile(tx as u32, ty as u32)));
}

#[test]
fn blocked_direct_segment_falls_back_to_full_astar() {
    let map = map_with_rock_wall(32, 14, 2, 27);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let req = PathRequest {
        kind: EntityKind::Rifleman,
        start: (5, 10),
        goal: (24, 10),
        radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
        route_shape: RouteShape::Normal,
        policy: RoutePolicy::LegacyShape,
        budget: None,
    };
    let start = map.tile_center(5, 10);
    let goal = map.tile_center(24, 10);

    let (path, diagnostics) =
        resolved(service.request_with_diagnostics(&map, &occ, req, Some((start, goal)), true));

    assert_eq!(path.first(), Some(&goal));
    assert!(path.len() > 1);
    assert!(diagnostics.expanded_nodes > 0);
    assert!(!diagnostics.budget_exhausted);
}

#[test]
fn direct_segment_result_is_not_reused_for_unsafe_offsets_in_the_same_tiles() {
    let mut map = flat_test_map(32);
    let rock_index = map.index(14, 9);
    map.terrain[rock_index] = terrain::ROCK;
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let req = PathRequest {
        kind: EntityKind::Rifleman,
        start: (5, 10),
        goal: (24, 10),
        radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
        route_shape: RouteShape::Normal,
        policy: RoutePolicy::LegacyShape,
        budget: None,
    };
    let safe_segment = (map.tile_center(5, 10), map.tile_center(24, 10));
    let unsafe_y = 10.0 * config::TILE_SIZE as f32 + 3.0;
    let (safe_start, safe_goal) = safe_segment;
    let unsafe_segment = ((safe_start.0, unsafe_y), (safe_goal.0, unsafe_y));

    let (safe_path, safe) = resolved(service.request_with_diagnostics(
        &map,
        &occ,
        req.clone(),
        Some(safe_segment),
        true,
    ));
    let (offset_path, offset) =
        resolved(service.request_with_diagnostics(&map, &occ, req, Some(unsafe_segment), true));

    assert_eq!(safe_path, vec![safe_segment.1]);
    assert_eq!(safe.expanded_nodes, 0);
    assert!(offset.expanded_nodes > 0);
    assert_ne!(offset_path, vec![unsafe_segment.1]);
}

#[test]
fn pathing_permission_defers_cache_hits_and_misses() {
    let map = flat_test_map(32);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let req = PathRequest {
        kind: EntityKind::Worker,
        start: (3, 4),
        goal: (25, 19),
        radius_tiles: 0,
        route_shape: RouteShape::Normal,
        policy: RoutePolicy::LegacyShape,
        budget: None,
    };

    assert!(matches!(
        service.request_tile_path_with_diagnostics(&map, &occ, req.clone(), false),
        PathingRequestOutcome::Deferred
    ));
    let (path, diagnostics) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req.clone(), true));
    assert!(!path.is_empty());
    assert!(diagnostics.expanded_nodes > 0);
    assert!(matches!(
        service.request_tile_path_with_diagnostics(&map, &occ, req.clone(), false),
        PathingRequestOutcome::Deferred
    ));
    let (cached_path, cached) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req, true));
    assert_eq!(cached_path, path);
    assert_eq!(cached.cache_status, PathCacheStatus::Hit);
}

#[test]
fn completed_no_route_result_is_reused_without_another_search() {
    let mut map = flat_test_map(32);
    for ty in 3..=5 {
        for tx in 2..=4 {
            if (tx, ty) != (3, 4) {
                let index = map.index(tx, ty);
                map.terrain[index] = terrain::ROCK;
            }
        }
    }
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let req = PathRequest {
        kind: EntityKind::Worker,
        start: (3, 4),
        goal: (25, 19),
        radius_tiles: 0,
        route_shape: RouteShape::Normal,
        policy: RoutePolicy::LegacyShape,
        budget: None,
    };

    let (path, first) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req.clone(), true));
    assert!(path.is_empty());
    assert!(first.expanded_nodes > 0);
    assert!(!first.budget_exhausted);

    let (cached_path, cached) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req, true));
    assert!(cached_path.is_empty());
    assert_eq!(cached.cache_status, PathCacheStatus::Hit);
    assert_eq!(cached.expanded_nodes, 0);
}

#[test]
fn fastest_terrain_policy_uses_offset_road_and_has_distinct_cache_identity() {
    let mut map = flat_test_map(20);
    for tx in 2..=16 {
        let index = map.index(tx, 6);
        map.terrain[index] = terrain::ROAD_HORIZONTAL;
    }
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let legacy = PathRequest {
        kind: EntityKind::Rifleman,
        start: (2, 9),
        goal: (16, 9),
        radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
        route_shape: RouteShape::Normal,
        policy: RoutePolicy::LegacyShape,
        budget: None,
    };
    let (legacy_path, legacy_diagnostics) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, legacy.clone(), true));
    let weighted = PathRequest {
        policy: RoutePolicy::FastestTerrainTime,
        ..legacy
    };
    let (weighted_path, weighted_diagnostics) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, weighted.clone(), true));
    let (cached_path, cached_diagnostics) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, weighted, true));

    assert_eq!(legacy_diagnostics.cache_status, PathCacheStatus::Miss);
    assert_eq!(weighted_diagnostics.cache_status, PathCacheStatus::Miss);
    assert_eq!(cached_diagnostics.cache_status, PathCacheStatus::Hit);
    assert_eq!(cached_path, weighted_path);
    assert!(legacy_path.iter().all(|&(_, ty)| ty == 9));
    assert!(weighted_path.iter().any(|&(_, ty)| ty == 6));
}

#[test]
fn fastest_terrain_policy_never_uses_legacy_direct_bypass() {
    let map = flat_test_map(20);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let start = map.tile_center(2, 9);
    let goal = map.tile_center(16, 9);
    let request = PathRequest {
        kind: EntityKind::Rifleman,
        start: (2, 9),
        goal: (16, 9),
        radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
        route_shape: RouteShape::Normal,
        policy: RoutePolicy::FastestTerrainTime,
        budget: None,
    };

    let (_, diagnostics) =
        resolved(service.request_with_diagnostics(&map, &occ, request, Some((start, goal)), true));
    assert_eq!(diagnostics.cache_status, PathCacheStatus::Miss);
    assert!(diagnostics.expanded_nodes > 0);
}

#[test]
fn fastest_terrain_finalized_cache_preserves_exact_goal_and_clears_with_derived_state() {
    let map = flat_test_map(24);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let start = map.tile_center(3, 4);
    let tile_goal = map.tile_center(18, 15);
    let request = PathRequest {
        kind: EntityKind::ScoutCar,
        start: (3, 4),
        goal: (18, 15),
        radius_tiles: config::unit_radius_tiles(EntityKind::ScoutCar),
        route_shape: RouteShape::VehicleClearance,
        policy: RoutePolicy::FastestTerrainTime,
        budget: None,
    };

    let (first, first_diagnostics) = resolved(service.request_finalized_with_diagnostics(
        &map,
        &occ,
        request.clone(),
        (start, tile_goal),
        None,
        true,
    ));
    let (second, second_diagnostics) = resolved(service.request_finalized_with_diagnostics(
        &map,
        &occ,
        request.clone(),
        (start, tile_goal),
        None,
        true,
    ));
    assert_eq!(first, second);
    assert_eq!(first_diagnostics.cache_status, PathCacheStatus::Miss);
    assert_eq!(second_diagnostics.cache_status, PathCacheStatus::Hit);
    assert_eq!(service.finalized_cache.len(), 1);

    let exact_goal = (tile_goal.0 + 4.0, tile_goal.1 - 3.0);
    let (offset, _) = resolved(service.request_finalized_with_diagnostics(
        &map,
        &occ,
        request,
        (start, exact_goal),
        None,
        true,
    ));
    assert_eq!(offset.first().copied(), Some(exact_goal));
    assert_eq!(service.finalized_cache.len(), 2);

    service.clear_cache_and_search();
    assert!(service.finalized_cache.is_empty());
}
