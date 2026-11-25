use super::*;
use crate::game::ability::AbilityKind;
use crate::game::ability_runtime::{
    AbilityObjectPayload, AbilityWorldObjectKind, AbilityWorldObjectSpec,
};
use std::collections::BTreeSet;

fn anchor(center: (f32, f32), expires_tick: u32) -> AbilityRuntime {
    let mut runtime = AbilityRuntime::new();
    runtime
        .spawn_world_object(AbilityWorldObjectSpec {
            owner: 1,
            caster_id: 999,
            ability: AbilityKind::EkatMagicAnchor,
            kind: AbilityWorldObjectKind::MagicAnchor,
            x: center.0,
            y: center.1,
            created_tick: 0,
            expires_tick,
            payload: AbilityObjectPayload::MagicAnchor {
                radius: config::EKAT_MAGIC_ANCHOR_RADIUS_TILES * config::TILE_SIZE as f32,
            },
        })
        .unwrap();
    runtime
}

#[test]
fn anchor_pull_clamps_at_center_and_ignores_expired_or_outside_fields() {
    let map = flat_map(2);
    let players = [player_with_oil(1, 0), player_with_oil(2, 0)];
    let center = map.tile_center(12, 12);
    for (offset, expires_tick, expected_offset) in [
        (-0.25, 100, 0.0),
        (0.0, 100, 0.0),
        (-0.25, 1, -0.25),
        (-96.0, 100, -96.0),
        (-100.0, 100, -100.0),
    ] {
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(2, EntityKind::Rifleman, center.0 + offset, center.1)
            .unwrap();
        let occ = Occupancy::build(&map, &entities);
        super::super::anchor::apply_pull(
            &map,
            &mut entities,
            &crate::game::teams::TeamRelations::from_player_teams(
                players.iter().map(|p| (p.id, p.team_id)),
            ),
            &occ,
            1,
            &anchor(center, expires_tick),
            &BTreeSet::new(),
        );
        assert!((pos(&entities, unit).0 - center.0 - expected_offset).abs() < 0.01);
    }
}

#[test]
fn anchor_vector_respects_enemy_body_blockers_and_preserves_path() {
    let map = flat_map(2);
    let players = [player_with_oil(1, 0), player_with_oil(2, 0)];
    let start = map.tile_center(10, 10);
    let mut entities = EntityStore::new();
    let unit = entities
        .spawn_unit(2, EntityKind::Rifleman, start.0, start.1)
        .unwrap();
    let radius = entities.get(unit).unwrap().radius();
    entities
        .spawn_unit(
            1,
            EntityKind::Rifleman,
            start.0 + 2.0 * radius + 0.1,
            start.1,
        )
        .unwrap();
    let destination = (start.0, start.1 + 200.0);
    entities.get_mut(unit).unwrap().set_path(vec![destination]);
    let occ = Occupancy::build(&map, &entities);
    let runtime = anchor((start.0 + 32.0, start.1), 100);
    super::super::anchor::apply_pull(
        &map,
        &mut entities,
        &crate::game::teams::TeamRelations::from_player_teams(
            players.iter().map(|p| (p.id, p.team_id)),
        ),
        &occ,
        1,
        &runtime,
        &BTreeSet::new(),
    );
    assert_eq!(
        pos(&entities, unit),
        start,
        "pull must not cross an enemy body"
    );
    assert!(
        !entities.get(unit).unwrap().path_is_empty(),
        "pull must preserve movement intent"
    );
}

#[test]
fn anchor_pull_affects_workers_with_pass_through_work_orders() {
    let map = flat_map(2);
    let players = [player_with_oil(1, 0), player_with_oil(2, 0)];
    let start = map.tile_center(10, 10);
    let runtime = anchor((start.0 + 32.0, start.1), 100);
    let mut idle_position = None;
    for order in [
        Order::Idle,
        Order::gather(u32::MAX),
        Order::build(EntityKind::Barracks, 15, 15),
        Order::deconstruct(u32::MAX),
    ] {
        let mut entities = EntityStore::new();
        let worker = entities
            .spawn_unit(2, EntityKind::Worker, start.0, start.1)
            .unwrap();
        entities.get_mut(worker).unwrap().set_order(order.clone());
        let occ = Occupancy::build(&map, &entities);
        super::super::anchor::apply_pull(
            &map,
            &mut entities,
            &crate::game::teams::TeamRelations::from_player_teams(
                players.iter().map(|p| (p.id, p.team_id)),
            ),
            &occ,
            1,
            &runtime,
            &BTreeSet::new(),
        );
        let pulled = pos(&entities, worker);
        assert!(
            pulled.0 > start.0,
            "work order must not grant immunity: {order:?}"
        );
        assert_eq!(pulled, *idle_position.get_or_insert(pulled));
        assert_eq!(entities.get(worker).unwrap().order(), order);
    }
}

#[test]
fn anchor_contact_prevents_rear_pressure_from_advancing_the_front() {
    let map = flat_map(2);
    let players = [player_with_oil(1, 0), player_with_oil(2, 0)];
    let teams = crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let center = map.tile_center(14, 14);
    for reverse_spawn in [false, true] {
        let mut entities = EntityStore::new();
        let mut ids = Vec::new();
        let indices: Vec<usize> = if reverse_spawn {
            (0..10).rev().collect()
        } else {
            (0..10).collect()
        };
        let mut front = 0;
        for index in indices {
            let unit = entities
                .spawn_unit(
                    2,
                    EntityKind::Rifleman,
                    center.0 + 40.0 - index as f32 * 18.1,
                    center.1,
                )
                .unwrap();
            mark_moving(&mut entities, unit, (center.0 + 240.0, center.1));
            if index == 0 {
                front = unit;
            }
            ids.push(unit);
        }
        let start_x = pos(&entities, front).0;
        let runtime = anchor(center, 100);
        for tick in 1..=30 {
            let occ = Occupancy::build(&map, &entities);
            let spatial = SpatialIndex::build(&entities, map.width, map.height);
            movement_system_with_context(
                &map,
                &mut entities,
                &players,
                &occ,
                &spatial,
                tick,
                &SmokeCloudStore::new(),
                &runtime,
            );
            let spatial = SpatialIndex::build(&entities, map.width, map.height);
            resolve_collisions_with_teams(
                &mut entities,
                &spatial,
                &map,
                &occ,
                &teams,
                Some((&runtime, tick)),
            );
            assert!(pos(&entities, front).0 <= start_x + 0.01,
                "rear traffic must not override net inward motion: reverse={reverse_spawn}, tick={tick}, front={:?}", pos(&entities, front));
        }
        // With the same queued movement, expiry restores ordinary friendly traffic.
        let before = pos(&entities, front).0;
        for tick in 100..110 {
            let occ = Occupancy::build(&map, &entities);
            let spatial = SpatialIndex::build(&entities, map.width, map.height);
            movement_system_with_context(
                &map,
                &mut entities,
                &players,
                &occ,
                &spatial,
                tick,
                &SmokeCloudStore::new(),
                &runtime,
            );
            let spatial = SpatialIndex::build(&entities, map.width, map.height);
            resolve_collisions_with_teams(
                &mut entities,
                &spatial,
                &map,
                &occ,
                &teams,
                Some((&runtime, tick)),
            );
        }
        assert!(
            pos(&entities, front).0 > before + 1.0,
            "movement must resume on expiry"
        );
    }
}

#[test]
fn anchor_combines_outward_walking_with_pull_before_admitting_contact() {
    let map = flat_map(2);
    let players = [player_with_oil(1, 0), player_with_oil(2, 0)];
    let center = map.tile_center(14, 14);
    let mut entities = EntityStore::new();
    let front = entities
        .spawn_unit(2, EntityKind::Rifleman, center.0 + 40.0, center.1)
        .unwrap();
    entities
        .spawn_unit(2, EntityKind::Rifleman, center.0 + 22.0, center.1)
        .unwrap();
    mark_moving(&mut entities, front, (center.0 + 240.0, center.1));
    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.width, map.height);
    movement_system_with_context(
        &map,
        &mut entities,
        &players,
        &occ,
        &spatial,
        1,
        &SmokeCloudStore::new(),
        &anchor(center, 100),
    );
    assert!(
        pos(&entities, front).0 <= center.0 + 40.01,
        "a blocked inward vector must not leave a free outward path step"
    );
}

#[test]
fn anchor_collision_cleanup_cannot_add_outward_movement() {
    let map = flat_map(2);
    let teams = crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let center = map.tile_center(14, 14);
    for active in [false, true] {
        let mut entities = EntityStore::new();
        let front = entities
            .spawn_unit(2, EntityKind::Rifleman, center.0 + 40.0, center.1)
            .unwrap();
        entities
            .spawn_unit(2, EntityKind::Rifleman, center.0 + 30.0, center.1)
            .unwrap();
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        let runtime = anchor(center, 100);
        resolve_collisions_with_teams(
            &mut entities,
            &spatial,
            &map,
            &occ,
            &teams,
            active.then_some((&runtime, 1)),
        );
        if active {
            assert!(pos(&entities, front).0 <= center.0 + 40.01);
        } else {
            assert!(
                pos(&entities, front).0 > center.0 + 40.01,
                "control must reproduce ordinary pushing"
            );
        }
    }
}
