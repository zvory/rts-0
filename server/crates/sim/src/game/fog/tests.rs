use super::*;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::services::occupancy::footprint_center;
use crate::game::teams::TeamRelations;
use crate::protocol::terrain;

fn open_map(size: u32) -> Map {
    Map {
        width: size,
        height: size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(1, 1)],
        ..Default::default()
    }
}

fn map_with_rock_at(tile: (u32, u32)) -> Map {
    let mut map = open_map(8);
    let size = map.width;
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
    let mut fog = Fog::new(map.width, map.height);

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
    let mut fog = Fog::new(map.width, map.height);

    fog.recompute(&[1], &entities, &map);
    let tile = (2 * map.width + 2) as usize;
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
    let mut fog = Fog::new(map.width, map.height);

    fog.recompute(&[1, 2], &entities, &map);
    fog.accumulate_explored_for_viewers(&[(1, vec![1, 2]), (2, vec![1, 2])]);

    let first_tile = (2 * map.width + 2) as usize;
    let second_tile = (9 * map.width + 9) as usize;
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
    let mut fog = Fog::new(map.width, map.height);

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
fn ordinary_unit_sight_gains_capped_radius_from_absolute_elevation() {
    let mut low_map = open_map(40);
    low_map.elevation = vec![0; (low_map.width * low_map.height) as usize];
    let origin_tile = (20, 20);
    let origin = low_map.tile_center(origin_tile.0, origin_tile.1);
    let mut entities = EntityStore::new();
    let worker_id = entities
        .spawn_unit(1, EntityKind::Worker, origin.0, origin.1)
        .expect("worker should spawn");
    let base_sight = entities
        .get(worker_id)
        .expect("worker should remain present")
        .sight_tiles();

    let mut low_fog = Fog::new(low_map.width, low_map.height);
    low_fog.recompute(&[1], &entities, &low_map);
    assert!(low_fog.is_visible(1, origin_tile.0 + base_sight, origin_tile.1));
    assert!(!low_fog.is_visible(1, origin_tile.0 + base_sight + 1, origin_tile.1));

    let mut high_map = low_map.clone();
    let origin_index = high_map.index(origin_tile.0, origin_tile.1);
    high_map.elevation[origin_index] = 9;
    let mut high_fog = Fog::new(high_map.width, high_map.height);
    high_fog.recompute(&[1], &entities, &high_map);

    assert!(high_fog.is_visible(1, origin_tile.0 + base_sight + 4, origin_tile.1));
    assert!(!high_fog.is_visible(1, origin_tile.0 + base_sight + 5, origin_tile.1));
}

#[test]
fn building_sight_gains_elevation_bonus_outward_from_its_footprint() {
    let mut map = open_map(32);
    map.elevation = vec![0; (map.width * map.height) as usize];
    let center_tile = (16, 16);
    let center = map.tile_center(center_tile.0, center_tile.1);
    let center_index = map.index(center_tile.0, center_tile.1);
    map.elevation[center_index] = 8;
    let mut entities = EntityStore::new();
    entities
        .spawn_building(1, EntityKind::Barracks, center.0, center.1, true)
        .expect("barracks should spawn");
    let mut fog = Fog::new(map.width, map.height);

    fog.recompute(&[1], &entities, &map);

    assert!(
        fog.is_visible(1, 10, 16),
        "level eight adds four perimeter tiles"
    );
    assert!(
        !fog.is_visible(1, 9, 16),
        "building bonus remains capped at four tiles"
    );
}

#[test]
fn elevation_does_not_turn_zero_sight_buildings_into_vision_sources() {
    let mut map = open_map(16);
    map.elevation = vec![9; (map.width * map.height) as usize];
    let center = map.tile_center(8, 8);
    let mut entities = EntityStore::new();
    entities
        .spawn_building(1, EntityKind::TankTrap, center.0, center.1, true)
        .expect("Tank Trap should spawn");
    let mut fog = Fog::new(map.width, map.height);

    fog.recompute(&[1], &entities, &map);

    assert!(!fog.is_visible(1, 8, 8));
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
    let mut fog = Fog::new(map.width, map.height);

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
    let mut fog = Fog::new(map.width, map.height);

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
    let mut fog = Fog::new(map.width, map.height);

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
    let mut fog = Fog::new(map.width, map.height);

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
    let mut fog = Fog::new(map.width, map.height);

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
    let mut fog = Fog::new(map.width, map.height);

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

#[test]
fn scout_plane_sight_does_not_gain_an_elevation_bonus() {
    let mut map = open_map(48);
    map.elevation = vec![0; (map.width * map.height) as usize];
    let origin_tile = (24, 24);
    let origin = map.tile_center(origin_tile.0, origin_tile.1);
    let origin_index = map.index(origin_tile.0, origin_tile.1);
    map.elevation[origin_index] = 9;
    let mut entities = EntityStore::new();
    let plane_id = entities
        .spawn_unit(1, EntityKind::ScoutPlane, origin.0, origin.1)
        .expect("Scout Plane should spawn");
    let base_sight = entities
        .get(plane_id)
        .expect("Scout Plane should remain present")
        .sight_tiles();
    let smokes = SmokeCloudStore::new();
    let teams = TeamRelations::from_player_teams([(1, 1)]);
    let mut fog = Fog::new(map.width, map.height);

    fog.recompute_with_smoke(&[1], &entities, &map, &smokes);
    fog.stamp_scout_plane_sources_for_teams_with_smoke(&map, &entities, &smokes, &teams);

    assert!(fog.is_visible(1, origin_tile.0 + base_sight, origin_tile.1));
    assert!(!fog.is_visible(1, origin_tile.0 + base_sight + 1, origin_tile.1));
}

/// Deliberately evaluates every eligible ray, including cells already visible. Keep this
/// independent reference when optimizing the additive sight-stamping path.
fn reference_stamp_sight(grid: &mut [bool], entity: &Entity, map: &Map, los: &LineOfSight<'_>) {
    let radius = sight_tiles_at_elevation(map, entity.pos_x, entity.pos_y, entity.sight_tiles());
    if radius == 0 {
        return;
    }
    let origins = if entity.is_building() {
        building_footprint(map, entity)
            .into_iter()
            .filter(|&(x, y)| x < map.width && y < map.height)
            .map(|(x, y)| map.tile_center(x, y))
            .collect::<Vec<_>>()
    } else {
        vec![(entity.pos_x, entity.pos_y)]
    };
    for origin in origins {
        let origin_tile = map.tile_of(origin.0, origin.1);
        for y in 0..map.height {
            for x in 0..map.width {
                let dx = i64::from(x) - i64::from(origin_tile.0);
                let dy = i64::from(y) - i64::from(origin_tile.1);
                let radius = i64::from(radius);
                let in_range = if entity.is_building() {
                    dx.abs() <= radius && dy.abs() <= radius
                } else {
                    dx * dx + dy * dy <= radius * radius
                };
                if in_range && los.tile_visible_from_world(origin, (x, y)) {
                    grid[(y * map.width + x) as usize] = true;
                }
            }
        }
    }
}

#[test]
fn overlapping_sight_matches_all_rays_reference_with_terrain_buildings_and_smoke() {
    use rand::{rngs::SmallRng, Rng, SeedableRng};

    for seed in 0..12 {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut map = open_map(32);
        map.elevation = (0..map.width * map.height)
            .map(|_| rng.gen_range(0..10))
            .collect();
        for y in 0..map.height {
            for x in 0..map.width {
                if rng.gen_ratio(1, 12) {
                    map.terrain[(y * map.width + x) as usize] = terrain::ROCK;
                }
                if rng.gen_ratio(1, 8) {
                    map.concealment_tiles.push((x, y));
                }
            }
        }
        map.concealment_tiles.sort_unstable();
        let mut entities = EntityStore::new();
        for i in 0..20 {
            // Cluster most sources to exercise repeated union writes, including within one
            // building footprint. Include edge sources and off-center unit positions too.
            let (x, y) = if i < 16 {
                (rng.gen_range(12..20), rng.gen_range(12..20))
            } else {
                (rng.gen_range(0..32), rng.gen_range(0..32))
            };
            let center = map.tile_center(x, y);
            if i % 3 == 0 {
                entities
                    .spawn_building(1, EntityKind::Barracks, center.0, center.1, true)
                    .expect("reference building should spawn");
            } else {
                entities
                    .spawn_unit(
                        1,
                        EntityKind::Worker,
                        center.0 + rng.gen_range(-7.0..7.0),
                        center.1 + rng.gen_range(-7.0..7.0),
                    )
                    .expect("reference unit should spawn");
            }
        }
        let mut smoke = SmokeCloudStore::new();
        if seed % 2 == 0 {
            let center = map.tile_center(19, 18);
            smoke.spawn(center.0, center.1, 2.5, 100, 0).unwrap();
        }
        let mask = BuildingLosMask::new(&entities, &map);
        let los = LineOfSight::with_smoke_and_building_blockers(&map, &smoke, &mask.blockers);
        let mut actual = vec![false; (map.width * map.height) as usize];
        let mut expected = actual.clone();
        for entity in entities.iter() {
            stamp_sight(&mut actual, map.width, map.height, entity, &map, &los);
            reference_stamp_sight(&mut expected, entity, &map, &los);
            assert_eq!(actual, expected, "seed={seed} after entity={}", entity.id);
        }
    }
}
