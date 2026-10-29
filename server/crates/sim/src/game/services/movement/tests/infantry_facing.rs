use super::*;

#[test]
fn infantry_face_actual_avoidance_step_around_deployed_gunner() {
    for kind in [EntityKind::MachineGunner, EntityKind::Rifleman] {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        let start = map.tile_center(20, 20);
        let mover = entities.spawn_unit(1, kind, start.0, start.1).unwrap();
        let blocker = entities
            .spawn_unit(1, EntityKind::MachineGunner, start.0 + 40.0, start.1 + 20.0)
            .unwrap();
        entities
            .get_mut(blocker)
            .unwrap()
            .set_weapon_setup(WeaponSetup::Deployed);
        let goal = (start.0 + 160.0, start.1);
        mark_moving(&mut entities, mover, goal);
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);

        movement_system(&map, &mut entities, &mut [], &occ, &spatial, 1);

        let unit = entities.get(mover).unwrap();
        let dx = unit.pos_x - start.0;
        let dy = unit.pos_y - start.1;
        assert!(
            dx > 0.0 && dy < -0.01,
            "{kind:?} must still avoid the braced crew: delta=({dx},{dy})"
        );
        assert!(
            angle_delta(unit.facing(), dy.atan2(dx)).abs() < 0.0001,
            "{kind:?} faces the actual step rather than the eastward waypoint"
        );
    }
}

#[test]
fn infantry_arrival_snap_preserves_last_walking_heading() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let start = map.tile_center(20, 20);
    let mover = entities
        .spawn_unit(1, EntityKind::MachineGunner, start.0, start.1)
        .unwrap();
    let heading = -2.0;
    entities.get_mut(mover).unwrap().set_facing(heading);
    let goal = (start.0 + 0.5, start.1 + 0.5);
    mark_moving(&mut entities, mover, goal);
    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.width, map.height);

    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 1);

    let unit = entities.get(mover).unwrap();
    assert_eq!((unit.pos_x, unit.pos_y), goal);
    assert!(unit.path_is_empty());
    assert_eq!(
        unit.facing(),
        heading,
        "tiny arrival correction must not swing the crew around"
    );
}

#[test]
fn blocked_infantry_keep_last_walking_heading() {
    let mut map = flat_map(1);
    let mut entities = EntityStore::new();
    // An impassable next tile rejects both the direct landing and wall-slide fallback.
    let tile = (20, 20);
    let start = (
        (tile.0 + 1) as f32 * config::TILE_SIZE as f32 - 0.1,
        (tile.1 as f32 + 0.5) * config::TILE_SIZE as f32,
    );
    let blocked = tile.1 * map.width + tile.0 + 1;
    map.terrain[blocked as usize] = crate::protocol::terrain::ROCK;
    let mover = entities
        .spawn_unit(1, EntityKind::MachineGunner, start.0, start.1)
        .unwrap();
    let heading = -2.0;
    entities.get_mut(mover).unwrap().set_facing(heading);
    mark_moving(&mut entities, mover, (start.0 + 32.0, start.1));
    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.width, map.height);

    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 1);

    let unit = entities.get(mover).unwrap();
    assert_eq!((unit.pos_x, unit.pos_y), start);
    assert_eq!(
        unit.facing(),
        heading,
        "rejected steps must not turn a stationary crew"
    );
}
