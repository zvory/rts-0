use super::*;
use crate::game::teams::TeamRelations;

fn teams() -> TeamRelations {
    TeamRelations::from_player_teams([(1, 1), (2, 2), (3, 1)])
}

#[test]
fn friendly_push_chain_stops_at_enemy_tank() {
    for reverse_spawn in [false, true] {
        let map = flat_map(2);
        let mut entities = EntityStore::new();
        let spawn_tank = |entities: &mut EntityStore| {
            entities
                .spawn_unit(2, EntityKind::Tank, 600.0, 600.0)
                .unwrap()
        };
        let first = reverse_spawn.then(|| spawn_tank(&mut entities));
        let mut friends = Vec::new();
        // The tank's broadphase hull boundary gives the infantry a legal starting gap.
        let hull = unit_body_with_facing(EntityKind::Tank, 600.0, 600.0, 0.0).unwrap();
        for i in 0..5 {
            let id = entities
                .spawn_unit(
                    1,
                    EntityKind::Rifleman,
                    hull.aabb().min_x - 10.0 - i as f32 * 12.0,
                    600.0,
                )
                .unwrap();
            mark_moving(&mut entities, id, (700.0, 600.0));
            friends.push(id);
        }
        let tank = first.unwrap_or_else(|| spawn_tank(&mut entities));
        let before = pos(&entities, tank);
        for _ in 0..120 {
            let occ = Occupancy::build(&map, &entities);
            let spatial = SpatialIndex::build(&entities, map.width, map.height);
            movement_system(&map, &mut entities, &[], &occ, &spatial, 0);
            resolve_collisions_with_teams(&mut entities, &spatial, &map, &occ, &teams());
            assert_eq!(
                pos(&entities, tank),
                before,
                "enemy tank must never absorb a friendly push chain"
            );
            for &friend in &friends {
                assert!(body_overlap_depth(&entities, friend, tank) <= 0.001);
            }
        }
    }
}

#[test]
fn opposing_moving_units_cannot_cross_or_push_each_other() {
    let map = flat_map(2);
    let mut entities = EntityStore::new();
    let a = entities
        .spawn_unit(1, EntityKind::Rifleman, 400.0, 600.0)
        .unwrap();
    let b = entities
        .spawn_unit(2, EntityKind::Rifleman, 450.0, 600.0)
        .unwrap();
    mark_moving(&mut entities, a, (500.0, 600.0));
    mark_moving(&mut entities, b, (350.0, 600.0));
    for tick in 0..60 {
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        movement_system(&map, &mut entities, &[], &occ, &spatial, tick);
        assert!(body_overlap_depth(&entities, a, b) <= 0.001);
        let before = (pos(&entities, a), pos(&entities, b));
        resolve_collisions_with_teams(&mut entities, &spatial, &map, &occ, &teams());
        assert_eq!((pos(&entities, a), pos(&entities, b)), before);
    }
}

#[test]
fn teammate_units_still_separate_but_opponents_do_not() {
    let map = flat_map(2);
    for owner in [2, 3] {
        let mut entities = EntityStore::new();
        let a = entities
            .spawn_unit(1, EntityKind::Rifleman, 400.0, 600.0)
            .unwrap();
        let b = entities
            .spawn_unit(owner, EntityKind::Rifleman, 410.0, 600.0)
            .unwrap();
        let before = (pos(&entities, a), pos(&entities, b));
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        resolve_collisions_with_teams(&mut entities, &spatial, &map, &occ, &teams());
        if owner == 2 {
            assert_eq!((pos(&entities, a), pos(&entities, b)), before);
        } else {
            assert!(body_overlap_depth(&entities, a, b) <= 0.001);
        }
    }
}

#[test]
fn swept_enemy_check_rejects_tunneling_and_hull_rotation() {
    let mut entities = EntityStore::new();
    let a = entities
        .spawn_unit(1, EntityKind::Rifleman, 400.0, 600.0)
        .unwrap();
    entities
        .spawn_unit(2, EntityKind::Rifleman, 450.0, 600.0)
        .unwrap();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 600.0, 600.0)
        .unwrap();
    let hull = unit_body_for_entity(entities.get(tank).unwrap()).unwrap();
    entities
        .spawn_unit(2, EntityKind::Rifleman, 600.0, hull.aabb().max_y + 9.1)
        .unwrap();
    let blockers = standability::UnitBlockers::new(&entities);
    assert!(!blockers.motion_clear(entities.get(a).unwrap(), (500.0, 600.0), 0.0, &teams()));
    assert!(!blockers.motion_clear(
        entities.get(tank).unwrap(),
        (600.0, 600.0),
        std::f32::consts::FRAC_PI_2,
        &teams()
    ));
}
