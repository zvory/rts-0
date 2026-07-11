use super::super::MoveCoordinator;
use super::{
    formation_goal_facing, formation_goals, formation_goals_with_known_trenches, is_free_goal,
    tile_chebyshev_distance, FormationAssignment, FormationUnit, KnownTrench,
    VEHICLE_BODY_FORMATION_GAP_TILES,
};
use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::pathfinding::Passability;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::pathing::PathingService;
use crate::game::services::standability;
use crate::protocol::terrain;

fn formation_unit(id: u32, map: &Map, tile: (u32, u32)) -> FormationUnit {
    formation_unit_kind(id, EntityKind::Rifleman, map, tile)
}

fn formation_unit_kind(id: u32, kind: EntityKind, map: &Map, tile: (u32, u32)) -> FormationUnit {
    FormationUnit {
        id,
        kind,
        pos: map.tile_center(tile.0, tile.1),
    }
}

fn square_formation(map: &Map) -> Vec<FormationUnit> {
    vec![
        formation_unit(1, map, (8, 63)),
        formation_unit(2, map, (12, 63)),
        formation_unit(3, map, (8, 67)),
        formation_unit(4, map, (12, 67)),
    ]
}

fn trench_at(id: u32, map: &Map, tile: (u32, u32)) -> KnownTrench {
    let (x, y) = map.tile_center(tile.0, tile.1);
    KnownTrench {
        id,
        x,
        y,
        radius_tiles: config::ENTRENCHMENT_TRENCH_RADIUS_TILES,
    }
}

fn offset_from(point: (f32, f32), origin: (f32, f32)) -> (f32, f32) {
    (point.0 - origin.0, point.1 - origin.1)
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.01,
        "expected {actual:.2} to be close to {expected:.2}"
    );
}

fn flat_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![],
        base_sites: vec![],
    }
}

fn set_passable(map: &mut Map, tx: u32, ty: u32) {
    map.terrain[(ty * map.size + tx) as usize] = terrain::GRASS;
}

#[test]
fn near_group_move_compacts_goals_near_click() {
    let map = Map::generate(1, 0x1234_5678);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let units = square_formation(&map);
    let click = map.tile_center(11, 65);

    let goals = formation_goals(&map, &occ, &units, click);

    assert_eq!(goals.len(), units.len());
    for goal in goals {
        let dx = goal.0 - click.0;
        let dy = goal.1 - click.1;
        let dist = (dx * dx + dy * dy).sqrt();
        assert!(
            dist <= config::TILE_SIZE as f32 * 1.5,
            "near goals should stay clustered around the click, got {goal:?}"
        );
    }
}

#[test]
fn far_group_move_preserves_world_offsets() {
    let map = Map::generate(1, 0x1234_5678);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let units = square_formation(&map);
    let click = map.tile_center(30, 65);

    let goals = formation_goals(&map, &occ, &units, click);

    let ts = config::TILE_SIZE as f32;
    let expected = [
        (-2.0 * ts, -2.0 * ts),
        (2.0 * ts, -2.0 * ts),
        (-2.0 * ts, 2.0 * ts),
        (2.0 * ts, 2.0 * ts),
    ];
    for (goal, expected_offset) in goals.iter().zip(expected) {
        let actual = offset_from(*goal, click);
        assert_close(actual.0, expected_offset.0);
        assert_close(actual.1, expected_offset.1);
    }
}

#[test]
fn far_scattered_group_move_caps_preserved_offsets() {
    let map = flat_map(80);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let units = vec![
        formation_unit(1, &map, (5, 20)),
        formation_unit(2, &map, (45, 20)),
    ];
    let click = map.tile_center(60, 50);

    let goals = formation_goals(&map, &occ, &units, click);

    let ts = config::TILE_SIZE as f32;
    let expected = [(-4.0 * ts, 0.0), (4.0 * ts, 0.0)];
    for (goal, expected_offset) in goals.iter().zip(expected) {
        let actual = offset_from(*goal, click);
        assert_close(actual.0, expected_offset.0);
        assert_close(actual.1, expected_offset.1);
    }
}

#[test]
fn medium_group_move_blends_offsets() {
    let map = Map::generate(1, 0x1234_5678);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let units = square_formation(&map);
    let click = map.tile_center(21, 65);

    let goals = formation_goals(&map, &occ, &units, click);

    let ts = config::TILE_SIZE as f32;
    let expected = [(-ts, -ts), (ts, -ts), (-ts, ts), (ts, ts)];
    for (goal, expected_offset) in goals.iter().zip(expected) {
        let actual = offset_from(*goal, click);
        assert_close(actual.0, expected_offset.0);
        assert_close(actual.1, expected_offset.1);
    }
}

#[test]
fn formation_goals_are_unique_when_tiles_are_free() {
    let map = Map::generate(1, 0x1234_5678);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let units = square_formation(&map);
    let click = map.tile_center(11, 65);

    let first = formation_goals(&map, &occ, &units, click);
    let second = formation_goals(&map, &occ, &units, click);

    assert_eq!(
        first, second,
        "formation assignment should be deterministic"
    );
    let mut seen = std::collections::HashSet::new();
    for goal in first {
        let tile = map.tile_of(goal.0, goal.1);
        assert!(
            seen.insert(tile),
            "duplicate goal tile {tile:?} for multi-unit formation"
        );
    }
}

#[test]
fn eligible_infantry_prefers_known_trench_near_formation_goal() {
    let map = flat_map(40);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let units = vec![formation_unit_kind(1, EntityKind::Rifleman, &map, (10, 20))];
    let click = map.tile_center(20, 20);
    let trench = trench_at(1, &map, (22, 20));

    let goals = formation_goals_with_known_trenches(
        &map,
        &occ,
        &units,
        click,
        &[trench],
        &std::collections::BTreeSet::new(),
    );

    assert_eq!(
        goals[0],
        (trench.x, trench.y),
        "eligible infantry should target the nearby known trench"
    );
}

#[test]
fn trench_preference_ignores_non_eligible_units_far_trenches_and_occupied_trenches() {
    let map = flat_map(40);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let click = map.tile_center(20, 20);
    let near_trench = trench_at(1, &map, (22, 20));
    let far_trench = trench_at(2, &map, (27, 20));

    let worker = vec![formation_unit_kind(1, EntityKind::Worker, &map, (10, 20))];
    let worker_goals = formation_goals_with_known_trenches(
        &map,
        &occ,
        &worker,
        click,
        &[near_trench],
        &std::collections::BTreeSet::new(),
    );
    assert_eq!(worker_goals[0], click);

    let rifle = vec![formation_unit_kind(2, EntityKind::Rifleman, &map, (10, 20))];
    let far_goals = formation_goals_with_known_trenches(
        &map,
        &occ,
        &rifle,
        click,
        &[far_trench],
        &std::collections::BTreeSet::new(),
    );
    assert_eq!(far_goals[0], click);

    let mut occupied = std::collections::BTreeSet::new();
    occupied.insert(near_trench.id);
    let occupied_goals =
        formation_goals_with_known_trenches(&map, &occ, &rifle, click, &[near_trench], &occupied);
    assert_eq!(occupied_goals[0], click);
}

#[test]
fn vehicle_body_group_move_prefers_one_tile_gap_between_final_goals() {
    let map = flat_map(40);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let units = vec![
        formation_unit_kind(1, EntityKind::AntiTankGun, &map, (10, 10)),
        formation_unit_kind(2, EntityKind::ScoutCar, &map, (11, 10)),
        formation_unit_kind(3, EntityKind::Tank, &map, (10, 11)),
    ];
    let click = map.tile_center(20, 20);

    let goals = formation_goals(&map, &occ, &units, click);

    for i in 0..goals.len() {
        for j in (i + 1)..goals.len() {
            let a = map.tile_of(goals[i].0, goals[i].1);
            let b = map.tile_of(goals[j].0, goals[j].1);
            assert!(
                tile_chebyshev_distance(a, b) > VEHICLE_BODY_FORMATION_GAP_TILES,
                "vehicle-body final goals should leave one open tile between them, got {a:?} and {b:?}"
            );
        }
    }
}

#[test]
fn vehicle_body_goal_spacing_applies_to_mixed_units_but_not_dense_infantry() {
    let map = flat_map(40);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let rifle = formation_unit_kind(2, EntityKind::Rifleman, &map, (10, 10));
    let tank = formation_unit_kind(3, EntityKind::Tank, &map, (10, 10));
    let infantry = [FormationAssignment {
        kind: EntityKind::Rifleman,
        tile: (20, 20),
        trench_id: None,
    }];
    let vehicle = [FormationAssignment {
        kind: EntityKind::AntiTankGun,
        tile: (20, 20),
        trench_id: None,
    }];
    let adjacent = (21, 20);

    assert!(is_free_goal(&map, &occ, &rifle, adjacent, &infantry, true));
    assert!(!is_free_goal(&map, &occ, &tank, adjacent, &infantry, true));
    assert!(!is_free_goal(&map, &occ, &rifle, adjacent, &vehicle, true));
    assert!(is_free_goal(&map, &occ, &tank, adjacent, &infantry, false));
}

#[test]
fn blocked_formation_slot_falls_back_to_nearby_passable_tile() {
    let map = Map::generate(1, 0x1234_5678);
    let mut entities = EntityStore::new();
    let blocked_tile = (28, 63);
    let blocked_center = map.tile_center(blocked_tile.0, blocked_tile.1);
    entities
        .spawn_building(
            1,
            EntityKind::Depot,
            blocked_center.0,
            blocked_center.1,
            true,
        )
        .unwrap();
    let occ = Occupancy::build(&map, &entities);
    let units = square_formation(&map);
    let click = map.tile_center(30, 65);

    let goals = formation_goals(&map, &occ, &units, click);
    let first_tile = map.tile_of(goals[0].0, goals[0].1);

    assert_ne!(
        first_tile, blocked_tile,
        "blocked desired formation slot should not be assigned"
    );
    assert_eq!(
        first_tile,
        (29, 62),
        "nearby fallback should use deterministic ring order"
    );
    assert!(map.is_passable(first_tile.0 as i32, first_tile.1 as i32));
    assert!(occ.passable(first_tile.0 as i32, first_tile.1 as i32));
}

#[test]
fn unreachable_formation_slot_collapses_toward_center() {
    let mut map = flat_map(80);
    let isolated_tile = (56, 50);
    for ty in (isolated_tile.1 - 1)..=(isolated_tile.1 + 1) {
        for tx in (isolated_tile.0 - 1)..=(isolated_tile.0 + 1) {
            let idx = map.index(tx, ty);
            map.terrain[idx] = terrain::WATER;
        }
    }
    set_passable(&mut map, isolated_tile.0, isolated_tile.1);

    let mut entities = EntityStore::new();
    let left_pos = map.tile_center(5, 20);
    let left = entities
        .spawn_unit(1, EntityKind::Rifleman, left_pos.0, left_pos.1)
        .expect("left rifleman should spawn");
    let right_pos = map.tile_center(45, 20);
    let right = entities
        .spawn_unit(1, EntityKind::Rifleman, right_pos.0, right_pos.1)
        .expect("right rifleman should spawn");
    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(8_192, 256);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    let click = map.tile_center(60, 50);

    coordinator.order_group_move(&mut entities, 1, &[left, right], click, false);

    let left_goal = entities
        .get(left)
        .and_then(|entity| entity.path_goal())
        .expect("left rifleman should receive a path goal");
    let left_tile = map.tile_of(left_goal.0, left_goal.1);

    assert_ne!(
        left_tile, isolated_tile,
        "formation assignment should reject an isolated but locally passable slot"
    );
    assert_eq!(
        left_tile,
        (58, 50),
        "unreachable offset should collapse inward toward the formation center"
    );
    let dist_sq = |a: (f32, f32), b: (f32, f32)| {
        let dx = a.0 - b.0;
        let dy = a.1 - b.1;
        dx * dx + dy * dy
    };
    assert!(
        dist_sq(left_goal, click)
            < dist_sq(map.tile_center(isolated_tile.0, isolated_tile.1), click),
        "fallback goal should be closer to the formation center"
    );
}

#[test]
fn formation_goal_accepts_side_on_tank_tile_center_near_building() {
    let mut map = Map::generate(1, 0x1234_5678);
    for tile in &mut map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    let mut entities = EntityStore::new();
    let (bx, by) =
        crate::game::services::occupancy::footprint_center(&map, EntityKind::Depot, 10, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("depot should spawn");
    let occ = Occupancy::build(&map, &entities);
    let units = vec![formation_unit_kind(1, EntityKind::Tank, &map, (12, 6))];
    let adjacent_tile_goal = map.tile_center(12, 10);

    let goals = formation_goals(&map, &occ, &units, adjacent_tile_goal);
    let goal = goals[0];

    assert_eq!(
        map.tile_of(goal.0, goal.1),
        (12, 10),
        "side-on tank hull should allow adjacent tile-center formation slots"
    );
    let facing = formation_goal_facing(&units[0], goal);
    assert!(
        standability::unit_static_standable_with_facing(
            &map,
            &occ,
            EntityKind::Tank,
            goal.0,
            goal.1,
            facing,
        ),
        "assigned formation goal must be body-standable"
    );
}
