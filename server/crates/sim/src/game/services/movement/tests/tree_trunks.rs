use super::*;
use crate::protocol::MapDoodad;

#[test]
fn rifleman_move_completes_around_preview_tree_trunk() {
    let size = 64;
    let mut map = Map {
        width: size,
        height: size,
        terrain: vec![crate::protocol::terrain::GRASS; (size * size) as usize],
        ..Default::default()
    };
    let trunk = (1557.0, 1329.0);
    map.doodads.push(MapDoodad {
        id: 25,
        type_id: "tree.oak".to_string(),
        x: trunk.0 as u32,
        y: trunk.1 as u32,
        color: None,
    });
    let start = (1542.0, 1329.0);
    let clicked_goal = (1572.0, 1329.0);
    let normalized_goal = (1584.0, 1328.0);
    let mut entities = EntityStore::new();
    let rifleman = entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .expect("rifleman");
    let mut pathing = PathingService::new(8_192, 256);

    for tick in 1..=180 {
        pathing.advance_tick(tick);
        let occupancy = Occupancy::build(&map, &entities);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occupancy, tick);
        if tick == 1 {
            coordinator.order_group_move(&mut entities, 1, &[rifleman], clicked_goal, false);
        }
        coordinator.process_awaiting_paths(&mut entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        movement_system(&map, &mut entities, &mut [], &occupancy, &spatial, tick);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        resolve_collisions(&mut entities, &spatial, &map, &occupancy);
        let unit = entities.get(rifleman).expect("rifleman survives");
        assert!(standability::unit_static_standable(
            &map,
            &occupancy,
            EntityKind::Rifleman,
            unit.pos_x,
            unit.pos_y,
        ));
        assert!(
            !matches!(unit.order(), Order::Idle) || unit.pos_x > trunk.0,
            "tick {tick}: move false-arrived at ({:.2},{:.2})",
            unit.pos_x,
            unit.pos_y,
        );
    }

    let unit = entities.get(rifleman).expect("rifleman survives");
    let goal_error = moved_distance((unit.pos_x, unit.pos_y), normalized_goal);
    assert!(goal_error <= 1.0, "goal error {goal_error:.2}px");
    assert!(unit.pos_x > trunk.0, "rifleman never crossed the trunk");
}

#[test]
fn rifleman_recovery_step_near_shoreline_trees_makes_progress() {
    // Replay 491, Rifleman 345: tree steering alternated between this northbound
    // recovery step and the old southbound route, returning to its start every two ticks.
    let size = 128;
    let mut map = Map {
        width: size,
        height: size,
        terrain: vec![crate::protocol::terrain::GRASS; (size * size) as usize],
        ..Default::default()
    };
    for y in 54..=75 {
        let shore_x = if y < 59 { 98 } else { 97 };
        for x in shore_x..=108 {
            let index = map.index(x, y);
            map.terrain[index] = crate::protocol::terrain::WATER;
        }
    }
    for (index, (x, y)) in [
        (3067, 1835),
        (3102, 1856),
        (3062, 1858),
        (3084, 1860),
        (3082, 1874),
        (3068, 1897),
    ]
    .into_iter()
    .enumerate()
    {
        map.doodads.push(MapDoodad {
            id: index as u32 + 1,
            type_id: "tree.oak".to_string(),
            x,
            y,
            color: None,
        });
    }
    let mut entities = EntityStore::new();
    let start = (3125.6404, 1846.9987);
    let recovery = (3122.4226, 1839.6171);
    let goal = (3396.1968, 2960.7869);
    let rifleman = entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .expect("rifleman");
    mark_moving(&mut entities, rifleman, goal);
    entities
        .get_mut(rifleman)
        .unwrap()
        .set_path(vec![goal, (3088.0, 2256.0), recovery]);
    for tick in 1..=5 {
        let occupancy = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        movement_system(&map, &mut entities, &[], &occupancy, &spatial, tick);
        let unit = entities.get(rifleman).unwrap();
        assert!(standability::unit_static_standable(
            &map, &occupancy, unit.kind, unit.pos_x, unit.pos_y,
        ));
    }
    let distance = moved_distance(pos(&entities, rifleman), recovery);
    assert!(
        distance < 1.0,
        "recovery must advance instead of oscillating: distance={distance}, pos={:?}",
        pos(&entities, rifleman),
    );
}
