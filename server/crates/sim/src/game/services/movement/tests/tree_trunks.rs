use super::*;
use crate::protocol::MapDoodad;

#[test]
fn rifleman_move_completes_around_preview_tree_trunk() {
    let size = 64;
    let mut map = Map {
        size,
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
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut [], &occupancy, &spatial, tick);
        let spatial = SpatialIndex::build(&entities, map.size);
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
