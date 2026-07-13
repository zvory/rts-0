use super::*;
use crate::config;
use crate::game::entity::EntityStore;
use crate::protocol::terrain;

fn flat_test_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(1, 1)],
        base_sites: Vec::new(),
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
        relation: StaticPathingRelation::single_owner(1),
        kind: EntityKind::Worker,
        start: (1, 1),
        goal: (8, 8),
        radius_tiles: 0,
        route_shape: RouteShape::Normal,
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
    assert_eq!(second.tile_path_len, second_path.len());
    assert_eq!(first_path, second_path);
}

#[test]
fn exact_direct_segment_bypasses_astar() {
    let map = flat_test_map(32);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let req = PathRequest {
        relation: StaticPathingRelation::single_owner(1),
        kind: EntityKind::Rifleman,
        start: (3, 4),
        goal: (25, 19),
        radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
        route_shape: RouteShape::Normal,
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
fn blocked_direct_segment_falls_back_to_full_astar() {
    let map = map_with_rock_wall(32, 14, 2, 27);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let req = PathRequest {
        relation: StaticPathingRelation::single_owner(1),
        kind: EntityKind::Rifleman,
        start: (5, 10),
        goal: (24, 10),
        radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
        route_shape: RouteShape::Normal,
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
        relation: StaticPathingRelation::single_owner(1),
        kind: EntityKind::Rifleman,
        start: (5, 10),
        goal: (24, 10),
        radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
        route_shape: RouteShape::Normal,
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
fn search_permission_defers_misses_but_not_cache_hits() {
    let map = flat_test_map(32);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let mut service = PathingService::new(8_192, 256);
    let req = PathRequest {
        relation: StaticPathingRelation::single_owner(1),
        kind: EntityKind::Worker,
        start: (3, 4),
        goal: (25, 19),
        radius_tiles: 0,
        route_shape: RouteShape::Normal,
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
    let (cached_path, cached) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req, false));
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
        relation: StaticPathingRelation::single_owner(1),
        kind: EntityKind::Worker,
        start: (3, 4),
        goal: (25, 19),
        radius_tiles: 0,
        route_shape: RouteShape::Normal,
        budget: None,
    };

    let (path, first) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req.clone(), true));
    assert!(path.is_empty());
    assert!(first.expanded_nodes > 0);
    assert!(!first.budget_exhausted);

    let (cached_path, cached) =
        resolved(service.request_tile_path_with_diagnostics(&map, &occ, req, false));
    assert!(cached_path.is_empty());
    assert_eq!(cached.cache_status, PathCacheStatus::Hit);
    assert_eq!(cached.expanded_nodes, 0);
}
