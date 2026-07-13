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

    let (first_path, first) = service.request_tile_path_with_diagnostics(&map, &occ, req.clone());
    let (second_path, second) = service.request_tile_path_with_diagnostics(&map, &occ, req);

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
fn direct_if_clear_bypasses_astar_and_is_cached() {
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
        route_shape: RouteShape::DirectIfClear,
        budget: None,
    };

    let (first_path, first) = service.request_tile_path_with_diagnostics(&map, &occ, req.clone());
    let (second_path, second) = service.request_tile_path_with_diagnostics(&map, &occ, req);

    assert_eq!(first_path, vec![(25, 19)]);
    assert_eq!(first.cache_status, PathCacheStatus::Miss);
    assert_eq!(first.expanded_nodes, 0);
    assert!(!first.budget_exhausted);
    assert_eq!(second_path, first_path);
    assert_eq!(second.cache_status, PathCacheStatus::Hit);
}

#[test]
fn direct_if_clear_falls_back_to_full_astar_around_blockers() {
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
        route_shape: RouteShape::DirectIfClear,
        budget: None,
    };

    let (path, diagnostics) = service.request_tile_path_with_diagnostics(&map, &occ, req);

    assert_eq!(path.last(), Some(&(24, 10)));
    assert!(path.len() > 1);
    assert!(diagnostics.expanded_nodes > 0);
    assert!(!diagnostics.budget_exhausted);
    assert!(path.iter().all(|&(tx, ty)| map.is_passable(tx, ty)));
}
