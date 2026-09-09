use super::*;

#[test]
fn moving_infantry_passes_stationary_blocker_with_small_side_push() {
    for mover_kind in [
        EntityKind::MachineGunner,
        EntityKind::Rifleman,
        EntityKind::Worker,
    ] {
        for blocker_kind in [
            EntityKind::Worker,
            EntityKind::Rifleman,
            EntityKind::MachineGunner,
        ] {
            let map = flat_map(1);
            let mut entities = EntityStore::new();
            let (x, y) = map.tile_center(20, 20);
            let mover = entities.spawn_unit(1, mover_kind, x, y).unwrap();
            let blocker = entities.spawn_unit(1, blocker_kind, x + 30.0, y).unwrap();
            if blocker_kind == EntityKind::MachineGunner {
                entities
                    .get_mut(blocker)
                    .unwrap()
                    .set_weapon_setup(WeaponSetup::Deployed);
            }
            mark_moving(&mut entities, mover, (x + 200.0, y));
            let occ = Occupancy::build(&map, &entities);
            for tick in 0..100 {
                let spatial = SpatialIndex::build(&entities, map.width, map.height);
                movement_system(&map, &mut entities, &[], &occ, &spatial, tick);
                let spatial = SpatialIndex::build(&entities, map.width, map.height);
                resolve_collisions(&mut entities, &spatial, &map, &occ);
            }
            let mover_pos = pos(&entities, mover);
            let blocker_pos = pos(&entities, blocker);
            assert!(
                mover_pos.0 > blocker_pos.0 + 20.0,
                "{mover_kind:?} failed to pass {blocker_kind:?}: {mover_pos:?}, {blocker_pos:?}"
            );
            assert!(
                (blocker_pos.1 - y).abs() > 1.0,
                "blocker should yield sideways"
            );
            assert!(
                moved_distance((x + 30.0, y), blocker_pos) < 32.0,
                "blocker should only move a small distance: {blocker_pos:?}"
            );
        }
    }
}

#[test]
fn moving_infantry_does_not_side_push_another_moving_unit() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (x, y) = map.tile_center(20, 20);
    let a = entities
        .spawn_unit(1, EntityKind::MachineGunner, x, y)
        .unwrap();
    let b = entities
        .spawn_unit(1, EntityKind::Worker, x + 18.0, y)
        .unwrap();
    mark_moving(&mut entities, a, (x + 200.0, y));
    mark_moving(&mut entities, b, (x + 200.0, y));
    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.width, map.height);
    resolve_collisions(&mut entities, &spatial, &map, &occ);
    assert_eq!(pos(&entities, a).1, y);
    assert_eq!(pos(&entities, b).1, y);
    assert!(body_overlap_depth(&entities, a, b) <= COLLISION_EPS_PX);
}

#[test]
fn infantry_side_push_respects_wall_clearance() {
    let mut map = flat_map(1);
    for tx in 0..map.width {
        map.terrain[(21 * map.width + tx) as usize] = crate::protocol::terrain::WATER;
    }
    let mut entities = EntityStore::new();
    let (x, _) = map.tile_center(20, 20);
    let y = 21.0 * config::TILE_SIZE as f32 - 9.0 - 0.1;
    let mover = entities
        .spawn_unit(1, EntityKind::MachineGunner, x, y - 1.0)
        .unwrap();
    let blocker = entities
        .spawn_unit(1, EntityKind::Worker, x + 18.0, y)
        .unwrap();
    mark_moving(&mut entities, mover, (x + 200.0, y - 1.0));
    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.width, map.height);
    resolve_collisions(&mut entities, &spatial, &map, &occ);
    for id in [mover, blocker] {
        let e = entities.get(id).unwrap();
        assert!(standability::unit_static_standable(
            &map, &occ, e.kind, e.pos_x, e.pos_y
        ));
    }
    assert!(body_overlap_depth(&entities, mover, blocker) <= COLLISION_EPS_PX);
}
