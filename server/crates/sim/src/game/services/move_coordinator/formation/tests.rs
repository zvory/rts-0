use super::super::MoveCoordinator;
use super::{
    formation_goal_facing, formation_goals, formation_goals_with_known_trenches, is_free_goal,
    polyline_slots, tile_chebyshev_distance, FormationAssignment, FormationUnit, KnownTrench,
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
fn long_polyline_spreads_one_rank_across_full_stroke() {
    let map = flat_map(64);
    let units = (0..4)
        .map(|index| formation_unit(index + 1, &map, (4, 4 + index)))
        .collect::<Vec<_>>();
    let y = map.tile_center(20, 20).1;
    let slots = polyline_slots(&units, &[(100.0, y), (700.0, y)]);
    let mut xs = slots.iter().map(|(_, point)| point.0).collect::<Vec<_>>();
    xs.sort_by(f32::total_cmp);
    assert_close(xs[0], 100.0);
    assert_close(xs[3], 700.0);
    assert!(slots.iter().all(|(_, point)| (point.1 - y).abs() <= 0.01));
}

#[test]
fn polyline_slots_keep_nearby_units_from_crossing() {
    let map = flat_map(64);
    let units = vec![
        formation_unit(1, &map, (20, 4)),
        formation_unit(2, &map, (4, 4)),
    ];
    let left = map.tile_center(4, 4).0;
    let right = map.tile_center(20, 4).0;
    let y = map.tile_center(4, 4).1;
    let slots = polyline_slots(&units, &[(left, y), (right, y)]);
    assert_close(slots.iter().find(|(id, _)| *id == 1).unwrap().1 .0, right);
    assert_close(slots.iter().find(|(id, _)| *id == 2).unwrap().1 .0, left);
}

#[test]
fn short_polyline_adds_parallel_ranks() {
    let map = flat_map(64);
    let units = (0..6)
        .map(|index| formation_unit(index + 1, &map, (4, 4 + index)))
        .collect::<Vec<_>>();
    let slots = polyline_slots(&units, &[(300.0, 300.0), (332.0, 300.0)]);
    let distinct_y = slots
        .iter()
        .map(|(_, point)| point.1.round() as i32)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        distinct_y.len() >= 3,
        "short strokes should grow multiple ranks"
    );
}

#[test]
fn point_move_uses_the_same_compact_shape_at_every_distance() {
    let map = Map::generate(1, 0x1234_5678);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let units = square_formation(&map);
    let near_click = map.tile_center(11, 65);
    let far_click = map.tile_center(30, 65);

    let near = formation_goals(&map, &occ, &units, near_click);
    let far = formation_goals(&map, &occ, &units, far_click);
    let near_offsets = near
        .iter()
        .map(|point| (point.0 - near_click.0, point.1 - near_click.1))
        .collect::<Vec<_>>();
    let far_offsets = far
        .iter()
        .map(|point| (point.0 - far_click.0, point.1 - far_click.1))
        .collect::<Vec<_>>();

    assert_eq!(near_offsets, far_offsets);
    assert_eq!(
        near_offsets,
        vec![
            (-(config::TILE_SIZE as f32), -(config::TILE_SIZE as f32)),
            (0.0, -(config::TILE_SIZE as f32)),
            (-(config::TILE_SIZE as f32), 0.0),
            (0.0, 0.0),
        ]
    );
}

#[test]
fn scattered_infantry_compacts_without_reversing_left_to_right_order() {
    let map = flat_map(80);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let units = vec![
        formation_unit(1, &map, (45, 20)),
        formation_unit(2, &map, (5, 20)),
    ];
    let click = map.tile_center(60, 50);

    let goals = formation_goals(&map, &occ, &units, click);
    let right_goal = map.tile_of(goals[0].0, goals[0].1);
    let left_goal = map.tile_of(goals[1].0, goals[1].1);

    assert_eq!(right_goal.0, left_goal.0 + 1);
    assert_eq!(right_goal.1, left_goal.1);
}

#[test]
fn vehicle_column_keeps_one_open_tile_and_original_vertical_order() {
    let map = flat_map(80);
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let units = vec![
        formation_unit_kind(1, EntityKind::Tank, &map, (20, 30)),
        formation_unit_kind(2, EntityKind::Tank, &map, (20, 10)),
        formation_unit_kind(3, EntityKind::Tank, &map, (20, 20)),
    ];
    let click = map.tile_center(60, 40);

    let goals = formation_goals(&map, &occ, &units, click);
    let bottom = map.tile_of(goals[0].0, goals[0].1);
    let top = map.tile_of(goals[1].0, goals[1].1);
    let middle = map.tile_of(goals[2].0, goals[2].1);

    assert_eq!(top.0, middle.0);
    assert_eq!(middle.0, bottom.0);
    assert_eq!(middle.1, top.1 + 2);
    assert_eq!(bottom.1, middle.1 + 2);
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
    let blocked_tile = (29, 64);
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
    assert!(tile_chebyshev_distance(first_tile, blocked_tile) <= 6);
    assert!(map.is_passable(first_tile.0 as i32, first_tile.1 as i32));
    assert!(occ.passable(first_tile.0 as i32, first_tile.1 as i32));
}

#[test]
fn unreachable_compact_slots_use_nearby_reachable_tiles() {
    let mut map = flat_map(80);
    let isolated_tile = (59, 50);
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
    assert!(tile_chebyshev_distance(left_tile, isolated_tile) <= 6);
    assert!(map.is_passable(left_tile.0 as i32, left_tile.1 as i32));
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
