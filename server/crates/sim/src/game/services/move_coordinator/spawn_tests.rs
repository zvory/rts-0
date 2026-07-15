use super::*;
use crate::protocol::terrain;

fn flat_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![],
        base_sites: vec![],
    }
}

fn impassable_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::WATER; (size * size) as usize],
        starts: vec![],
        base_sites: vec![],
    }
}

fn set_passable(map: &mut Map, tx: u32, ty: u32) {
    map.terrain[(ty * map.size + tx) as usize] = terrain::GRASS;
}

#[test]
fn spawn_search_finds_point_outside_footprint() {
    let map = Map::generate(1, 0x1234_5678);
    let mut entities = EntityStore::new();
    // Place a barracks at tile (15, 15); footprint is 3x2.
    let (cx, cy) = map.tile_center(15, 15);
    let b_id = entities
        .spawn_building(1, EntityKind::Barracks, cx, cy, true)
        .unwrap();
    let occ = Occupancy::build(&map, &entities);

    let mut pathing = PathingService::new(8_192, 256);
    pathing.advance_tick(1);
    let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

    let (sx, sy) = coordinator
        .find_spawn_point(&entities, b_id, EntityKind::Tank, None)
        .expect("spawn point should exist");

    let (stx, sty) = map.tile_of(sx, sy);
    let footprint = building_footprint(&map, entities.get(b_id).unwrap());

    assert!(
        !footprint.contains(&(stx, sty)),
        "spawn tile ({stx},{sty}) is inside the barracks footprint {footprint:?}"
    );

    assert!(map.is_passable(stx as i32, sty as i32));
}

#[test]
fn tank_spawn_point_keeps_clear_of_top_map_edge() {
    let map = Map::generate(1, 0x1234_5678);
    let mut entities = EntityStore::new();
    let (bx, by) = map.tile_center(3, 0);
    let b_id = entities
        .spawn_building(1, EntityKind::Factory, bx, by, true)
        .unwrap();
    let occ = Occupancy::build(&map, &entities);

    let mut pathing = PathingService::new(8_192, 256);
    pathing.advance_tick(1);
    let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

    let (sx, sy) = coordinator
        .find_spawn_point(&entities, b_id, EntityKind::Tank, None)
        .expect("spawn point should exist");

    assert!(
        standability::unit_spawn_standable(&map, &occ, &entities, EntityKind::Tank, sx, sy,),
        "tank spawn point clips the top map edge"
    );
}

#[test]
fn tank_spawn_point_keeps_clear_of_adjacent_building() {
    let map = Map::generate(1, 0x1234_5678);
    let mut entities = EntityStore::new();
    let (fx, fy) = map.tile_center(16, 16);
    let factory_id = entities
        .spawn_building(1, EntityKind::Factory, fx, fy, true)
        .unwrap();
    let (nx, ny) = map.tile_center(20, 16);
    entities
        .spawn_building(1, EntityKind::Depot, nx, ny, true)
        .unwrap();
    let occ = Occupancy::build(&map, &entities);

    let mut pathing = PathingService::new(8_192, 256);
    pathing.advance_tick(1);
    let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

    let (sx, sy) = coordinator
        .find_spawn_point(&entities, factory_id, EntityKind::Tank, None)
        .expect("spawn point should exist");

    assert!(
        standability::unit_spawn_standable(&map, &occ, &entities, EntityKind::Tank, sx, sy,),
        "tank spawn point is too close to the adjacent building"
    );
}

#[test]
fn tank_spawn_point_prefers_gap_from_producer_when_available() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (fx, fy) = footprint_center(&map, EntityKind::Factory, 10, 10);
    let factory_id = entities
        .spawn_building(1, EntityKind::Factory, fx, fy, true)
        .expect("factory should spawn");
    let occ = Occupancy::build(&map, &entities);

    let mut pathing = PathingService::new(8_192, 256);
    pathing.advance_tick(1);
    let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

    let (sx, sy) = coordinator
        .find_spawn_point(&entities, factory_id, EntityKind::Tank, None)
        .expect("spawn point should exist");
    let factory = entities.get(factory_id).expect("factory");
    let rect = building_rect_for_entity(&map, factory).expect("factory rect");
    let gap = spawn_gap_from_building(EntityKind::Tank, sx, sy, rect).expect("tank body");
    let preferred = config::unit_stats(EntityKind::Tank)
        .expect("tank stats")
        .radius
        * SPAWN_PREFERRED_GAP_UNIT_FRACTION;

    assert!(
        gap >= preferred,
        "tank spawn should prefer at least {preferred:.2}px of building clearance, got {gap:.2}px"
    );
}

#[test]
fn tank_spawn_point_falls_back_to_tight_exit_when_no_gap_candidate_exists() {
    let mut map = impassable_map(12);
    let mut entities = EntityStore::new();
    let (fx, fy) = footprint_center(&map, EntityKind::Factory, 4, 4);
    let factory_id = entities
        .spawn_building(1, EntityKind::Factory, fx, fy, true)
        .expect("factory should spawn");
    for tx in 4..=6 {
        set_passable(&mut map, tx, 3);
    }
    let occ = Occupancy::build(&map, &entities);

    let mut pathing = PathingService::new(8_192, 256);
    pathing.advance_tick(1);
    let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

    let (sx, sy) = coordinator
        .find_spawn_point(&entities, factory_id, EntityKind::Tank, None)
        .expect("tight spawn point should still be allowed");
    let factory = entities.get(factory_id).expect("factory");
    let rect = building_rect_for_entity(&map, factory).expect("factory rect");
    let gap = spawn_gap_from_building(EntityKind::Tank, sx, sy, rect).expect("tank body");
    let preferred = config::unit_stats(EntityKind::Tank)
        .expect("tank stats")
        .radius
        * SPAWN_PREFERRED_GAP_UNIT_FRACTION;

    assert_eq!(
        map.tile_of(sx, sy),
        (5, 3),
        "only the tight tile-center exit should be legal"
    );
    assert!(
        gap < preferred,
        "test setup should force fallback to a sub-preferred gap, got {gap:.2}px"
    );
}

#[test]
fn rotation_clear_spawn_uses_nearby_units_exact_oriented_bodies() {
    let map = flat_map(12);
    let spawn = (160.0, 160.0);
    let nearby = (203.8, 203.8);
    let mut entities = EntityStore::new();
    let nearby_id = entities
        .spawn_unit(1, EntityKind::Tank, nearby.0, nearby.1)
        .expect("nearby tank should spawn");
    let occ = Occupancy::build(&map, &entities);

    let spawn_radius = unit_body(EntityKind::Tank, spawn.0, spawn.1)
        .expect("spawn body")
        .bounding_radius();
    let nearby_radius = unit_body_for_entity(entities.get(nearby_id).expect("nearby tank"))
        .expect("nearby body")
        .bounding_radius();
    let center_distance = ((nearby.0 - spawn.0).powi(2) + (nearby.1 - spawn.1).powi(2)).sqrt();
    assert!(
        center_distance < spawn_radius + nearby_radius,
        "test geometry must overlap the conservative bounding circles"
    );
    assert!(
        full_rotation_spawn_clear(&map, &occ, &entities, EntityKind::Tank, spawn.0, spawn.1,),
        "the swept spawn disk should clear the nearby tank's exact oriented body"
    );

    entities
        .spawn_unit(1, EntityKind::Tank, 200.0, 160.0)
        .expect("blocking tank should spawn");
    assert!(!full_rotation_spawn_clear(
        &map,
        &occ,
        &entities,
        EntityKind::Tank,
        spawn.0,
        spawn.1,
    ));
}
