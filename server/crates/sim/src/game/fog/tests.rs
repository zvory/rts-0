use super::*;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::services::occupancy::footprint_center;
use crate::game::teams::TeamRelations;
use crate::protocol::terrain;

fn open_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(1, 1)],
        base_sites: Vec::new(),
    }
}

fn map_with_rock_at(tile: (u32, u32)) -> Map {
    let mut map = open_map(8);
    let size = map.size;
    map.terrain[(tile.1 * size + tile.0) as usize] = terrain::ROCK;
    map
}

#[test]
fn stone_blocks_authoritative_fog_behind_it() {
    let map = map_with_rock_at((3, 2));
    let mut entities = EntityStore::new();
    let origin = map.tile_center(1, 2);
    entities
        .spawn_unit(1, EntityKind::Worker, origin.0, origin.1)
        .expect("worker should spawn");
    let mut fog = Fog::new(map.size);

    fog.recompute(&[1], &entities, &map);

    assert!(fog.is_visible(1, 3, 2));
    assert!(!fog.is_visible(1, 4, 2));
}

#[test]
fn explored_tiles_persist_after_current_sight_is_lost() {
    let map = open_map(8);
    let mut entities = EntityStore::new();
    let origin = map.tile_center(2, 2);
    entities
        .spawn_unit(1, EntityKind::Worker, origin.0, origin.1)
        .expect("worker should spawn");
    let mut fog = Fog::new(map.size);

    fog.recompute(&[1], &entities, &map);
    let tile = (2 * map.size + 2) as usize;
    assert_eq!(fog.explored_tiles_for(1)[tile], 1);

    fog.recompute(&[1], &EntityStore::new(), &map);

    assert_eq!(fog.visible_tiles_for(1)[tile], 0);
    assert_eq!(fog.explored_tiles_for(1)[tile], 1);
}

#[test]
fn team_visibility_is_accumulated_into_each_viewers_exploration() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let first = map.tile_center(2, 2);
    let second = map.tile_center(9, 9);
    entities
        .spawn_unit(1, EntityKind::Worker, first.0, first.1)
        .expect("player one worker should spawn");
    entities
        .spawn_unit(2, EntityKind::Worker, second.0, second.1)
        .expect("player two worker should spawn");
    let mut fog = Fog::new(map.size);

    fog.recompute(&[1, 2], &entities, &map);
    fog.accumulate_explored_for_viewers(&[(1, vec![1, 2]), (2, vec![1, 2])]);

    let first_tile = (2 * map.size + 2) as usize;
    let second_tile = (9 * map.size + 9) as usize;
    for player in [1, 2] {
        let explored = fog.explored_tiles_for(player);
        assert_eq!(explored[first_tile], 1);
        assert_eq!(explored[second_tile], 1);
    }
}

#[test]
fn building_sight_reveals_footprint_and_one_tile_perimeter() {
    let map = open_map(8);
    let mut entities = EntityStore::new();
    let center = map.tile_center(3, 3);
    entities
        .spawn_building(1, EntityKind::Barracks, center.0, center.1, true)
        .expect("barracks should spawn");
    let mut fog = Fog::new(map.size);

    fog.recompute(&[1], &entities, &map);

    for ty in 2..=3 {
        for tx in 2..=4 {
            assert!(fog.is_visible(1, tx, ty), "footprint tile ({tx},{ty})");
        }
    }
    for ty in 1..=4 {
        for tx in 1..=5 {
            assert!(
                fog.is_visible(1, tx, ty),
                "one-tile perimeter tile ({tx},{ty})"
            );
        }
    }
    assert!(!fog.is_visible(1, 0, 1));
    assert!(!fog.is_visible(1, 6, 4));
}

#[test]
fn smoke_blocks_authoritative_fog_behind_it_but_reveals_cloud_edge() {
    let map = map_with_rock_at((7, 7));
    let mut entities = EntityStore::new();
    let origin = map.tile_center(1, 2);
    entities
        .spawn_unit(1, EntityKind::Worker, origin.0, origin.1)
        .expect("worker should spawn");
    let mut smokes = SmokeCloudStore::new();
    let smoke = map.tile_center(3, 2);
    smokes
        .spawn(smoke.0, smoke.1, 1.0, 100, 0)
        .expect("smoke should spawn");
    let mut fog = Fog::new(map.size);

    fog.recompute_with_smoke(&[1], &entities, &map, &smokes);

    assert!(fog.is_visible(1, 3, 2));
    assert!(!fog.is_visible(1, 5, 2));
}

#[test]
fn owned_building_blocks_authoritative_fog_beyond_edge_sight() {
    let map = open_map(10);
    let mut entities = EntityStore::new();
    let origin = map.tile_center(1, 3);
    entities
        .spawn_unit(1, EntityKind::Worker, origin.0, origin.1)
        .expect("worker should spawn");
    let depot = footprint_center(&map, EntityKind::Depot, 3, 2);
    entities
        .spawn_building(1, EntityKind::Depot, depot.0, depot.1, true)
        .expect("depot should spawn");
    let mut fog = Fog::new(map.size);

    fog.recompute(&[1], &entities, &map);

    assert!(fog.is_visible(1, 3, 3));
    assert!(
        fog.is_visible_world(1, depot.0, depot.1),
        "the visible building face should reveal the building footprint"
    );
    assert!(
        fog.is_visible(1, 5, 3),
        "owned buildings should reveal one tile past their footprint edge"
    );
    assert!(
        !fog.is_visible(1, 6, 3),
        "owned buildings should still occlude tiles beyond their edge sight"
    );
}

#[test]
fn enemy_building_blocks_authoritative_fog_behind_it() {
    let map = open_map(10);
    let mut entities = EntityStore::new();
    let observer = map.tile_center(7, 3);
    entities
        .spawn_unit(2, EntityKind::Worker, observer.0, observer.1)
        .expect("observer should spawn");
    let hidden = map.tile_center(2, 3);
    entities
        .spawn_unit(1, EntityKind::Rifleman, hidden.0, hidden.1)
        .expect("hidden unit should spawn");
    let depot = footprint_center(&map, EntityKind::Depot, 3, 2);
    entities
        .spawn_building(1, EntityKind::Depot, depot.0, depot.1, true)
        .expect("depot should spawn");
    let mut fog = Fog::new(map.size);

    fog.recompute(&[2], &entities, &map);

    assert!(fog.is_visible(2, 4, 3));
    assert!(
        fog.is_visible_world(2, depot.0, depot.1),
        "enemy buildings should become visible when their near footprint edge is visible"
    );
    assert!(
        !fog.is_visible_world(2, hidden.0, hidden.1),
        "enemy buildings should occlude units on the far side"
    );
}

#[test]
fn tank_traps_do_not_block_authoritative_fog() {
    let map = open_map(10);
    let mut entities = EntityStore::new();
    let origin = map.tile_center(1, 3);
    entities
        .spawn_unit(1, EntityKind::Worker, origin.0, origin.1)
        .expect("worker should spawn");
    let trap = map.tile_center(3, 3);
    entities
        .spawn_building(2, EntityKind::TankTrap, trap.0, trap.1, true)
        .expect("tank trap should spawn");
    let mut fog = Fog::new(map.size);

    fog.recompute(&[1], &entities, &map);

    assert!(
        fog.is_visible(1, 5, 3),
        "Tank Traps remain low obstacles that do not occlude line of sight"
    );
}

#[test]
fn unit_inside_smoke_does_not_stamp_vision() {
    let map = map_with_rock_at((7, 7));
    let mut entities = EntityStore::new();
    let origin = map.tile_center(2, 2);
    entities
        .spawn_unit(1, EntityKind::Worker, origin.0, origin.1)
        .expect("worker should spawn");
    let mut smokes = SmokeCloudStore::new();
    smokes
        .spawn(origin.0, origin.1, 1.0, 100, 0)
        .expect("smoke should spawn");
    let mut fog = Fog::new(map.size);

    fog.recompute_with_smoke(&[1], &entities, &map, &smokes);

    assert!(!fog.is_visible(1, 2, 2));
    assert!(!fog.is_visible(1, 3, 2));
}

#[test]
fn every_scout_plane_stamps_independent_team_vision() {
    let map = open_map(48);
    let mut entities = EntityStore::new();
    let first = map.tile_center(8, 8);
    let second = map.tile_center(39, 39);
    entities
        .spawn_unit(1, EntityKind::ScoutPlane, first.0, first.1)
        .expect("first Scout Plane should spawn");
    entities
        .spawn_unit(1, EntityKind::ScoutPlane, second.0, second.1)
        .expect("second Scout Plane should spawn");
    let smokes = SmokeCloudStore::new();
    let teams = TeamRelations::from_player_teams([(1, 7), (2, 7), (3, 3)]);
    let mut fog = Fog::new(map.size);

    fog.recompute_with_smoke(&[1, 2, 3], &entities, &map, &smokes);
    fog.stamp_scout_plane_sources_for_teams_with_smoke(&map, &entities, &smokes, &teams);

    for viewer in [1, 2] {
        assert!(
            fog.is_visible_world(viewer, first.0, first.1),
            "owner and teammate should receive the first plane's sight"
        );
        assert!(
            fog.is_visible_world(viewer, second.0, second.1),
            "owner and teammate should receive the second plane's sight"
        );
    }
    assert!(
        !fog.is_visible_world(3, first.0, first.1) && !fog.is_visible_world(3, second.0, second.1),
        "enemy players must not receive Scout Plane vision"
    );
}
