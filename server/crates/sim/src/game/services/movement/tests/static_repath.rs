use super::*;

/// A path that becomes invalid because a building appeared on it should not sidestep
/// forever against the old route. After a one-second static-block debounce, movement
/// queues the unit for the existing path coordinator to compute a fresh route.
#[test]
fn static_building_blockage_queues_repath_after_debounce() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (w0x, w0y) = map.tile_center(11, 10);
    let (gx, gy) = map.tile_center(20, 10);
    let unit = entities
        .spawn_unit(1, EntityKind::Rifleman, w0x - 16.5, w0y)
        .unwrap();
    set_path_direct(&mut entities, unit, vec![(w0x, w0y), (gx, gy)]);
    if let Some(e) = entities.get_mut(unit) {
        e.set_order(Order::move_to(gx, gy));
        e.mark_move_phase(MovePhase::Moving);
    }

    // Depot centered on tile (12,10) covers (11,9),(12,9),(11,10),(12,10),
    // so the next waypoint tile became blocked after the path was assigned.
    let (bx, by) = map.tile_center(12, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("building spawn");

    for tick in 0..config::STATIC_BLOCKED_REPATH_TICKS as u32 - 1 {
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        assert_eq!(
            entities.get(unit).and_then(|e| e.move_phase()),
            Some(MovePhase::Moving),
            "unit should debounce static blockage before repathing"
        );
    }

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(
        &map,
        &mut entities,
        &mut [],
        &occ,
        &spatial,
        config::STATIC_BLOCKED_REPATH_TICKS as u32,
    );

    let e = entities.get(unit).unwrap();
    assert_eq!(e.move_phase(), Some(MovePhase::AwaitingPath));
    assert!(e.path_is_empty(), "stale blocked path should be cleared");
    assert_eq!(e.path_goal(), Some((gx, gy)));
}

#[test]
fn shallow_wall_slide_still_queues_static_repath_after_debounce() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(10, 10);
    let (gx, gy) = map.tile_center(20, 10);
    let unit = entities
        .spawn_unit(1, EntityKind::Worker, sx, sy)
        .expect("worker spawn");
    let shallow_goal = (gx, gy + 4.0);
    set_path_direct(&mut entities, unit, vec![shallow_goal]);
    if let Some(entity) = entities.get_mut(unit) {
        entity.set_order(Order::move_to(shallow_goal.0, shallow_goal.1));
        entity.mark_move_phase(MovePhase::Moving);
        entity.reset_stuck(sx, sy);
    }

    let (bx, by) = map.tile_center(15, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("building spawn");

    let mut slid_sideways = false;
    let mut queued_repath = false;
    for tick in 0..300 {
        let before = pos(&entities, unit);
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        let after = pos(&entities, unit);
        slid_sideways |= (after.0 - before.0).abs() <= 1e-4 && (after.1 - before.1).abs() > 0.0;
        if entities.get(unit).and_then(|entity| entity.move_phase())
            == Some(MovePhase::AwaitingPath)
        {
            queued_repath = true;
            break;
        }
    }

    let entity = entities.get(unit).expect("worker should remain alive");
    assert!(
        slid_sideways,
        "fixture should exercise axis-only wall sliding"
    );
    assert!(
        queued_repath,
        "shallow wall slide should request a fresh path"
    );
    assert_eq!(entity.move_phase(), Some(MovePhase::AwaitingPath));
    assert!(
        entity.path_is_empty(),
        "stale shallow path should be cleared"
    );
    assert_eq!(entity.path_goal(), Some(shallow_goal));
}
