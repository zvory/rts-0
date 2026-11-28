use super::*;
use crate::game::ability::AbilityKind;
use crate::game::ability_runtime::{
    AbilityObjectPayload, AbilityWorldObjectKind, AbilityWorldObjectSpec,
};
fn field(center: (f32, f32), direction: [f32; 2]) -> AbilityRuntime {
    let mut runtime = AbilityRuntime::new();
    add_field(&mut runtime, center, direction, 1, 100);
    runtime
}

fn add_field(
    runtime: &mut AbilityRuntime,
    center: (f32, f32),
    direction: [f32; 2],
    owner: u32,
    expires_tick: u32,
) {
    runtime
        .spawn_world_object(AbilityWorldObjectSpec {
            owner,
            caster_id: 999,
            ability: AbilityKind::VectorField,
            kind: AbilityWorldObjectKind::VectorField,
            x: center.0,
            y: center.1,
            created_tick: 0,
            expires_tick,
            payload: AbilityObjectPayload::VectorField {
                direction,
                size_tiles: 4.0,
            },
        })
        .unwrap();
}
#[test]
fn vector_field_rectangle_is_centered_rotated_and_expires() {
    let runtime = field((400.0, 400.0), [0.0, 1.0]);
    assert_eq!(runtime.vector_field_at(463.0, 463.0, 1), Some([0.0, 1.0]));
    assert_eq!(runtime.vector_field_at(465.0, 400.0, 1), None);
    assert_eq!(runtime.vector_field_at(400.0, 465.0, 1), None);
    assert_eq!(runtime.vector_field_at(400.0, 400.0, 100), None);
}
#[test]
fn vector_field_push_scales_with_each_body_and_also_moves_allies() {
    let map = flat_map(2);
    let center = map.tile_center(10, 10);
    let players = [player_with_oil(1, 0), player_with_oil(2, 0)];
    for owner in [1, 2] {
        for kind in [EntityKind::Rifleman, EntityKind::Tank, EntityKind::ScoutCar] {
            let mut entities = EntityStore::new();
            let id = entities
                .spawn_unit(owner, kind, center.0, center.1)
                .unwrap();
            let occ = Occupancy::build(&map, &entities);
            apply_magic_anchor_pull(
                &map,
                &mut entities,
                &players,
                &occ,
                1,
                &field(center, [0.0, 1.0]),
            );
            let unit = entities.get(id).unwrap();
            assert_eq!(unit.pos_x(), center.0);
            let expected = config::unit_stats(kind).unwrap().speed * 0.5;
            assert!(
                (unit.pos_y() - center.1 - expected).abs() < 0.02,
                "{kind:?} owner={owner}"
            );
            assert!(
                unit.facing().abs() < 0.001,
                "field displacement does not rotate the body"
            );
        }
    }
}

#[test]
fn vector_field_is_distinct_from_ekat_anchor_for_both_magic_ranks() {
    use crate::rules::faction::{ability_definition, catalog_for, CULTIVATORS_FACTION_ID};
    let catalog = catalog_for(CULTIVATORS_FACTION_ID).unwrap();
    assert!(catalog.allows_ability(AbilityKind::VectorField, EntityKind::MagicAdept));
    assert!(!catalog.allows_ability(AbilityKind::EkatMagicAnchor, EntityKind::MagicAdept));
    assert!(!ability_definition(AbilityKind::VectorField)
        .carriers
        .contains(&EntityKind::Ekat));
    assert!(ability_definition(AbilityKind::EkatMagicAnchor)
        .carriers
        .contains(&EntityKind::Ekat));
    for master in [false, true] {
        let mut entities = EntityStore::new();
        let id = entities
            .spawn_unit(1, EntityKind::MagicAdept, 320.0, 320.0)
            .unwrap();
        entities.get_mut(id).unwrap().dao_master = master;
        assert!(crate::game::ability::carried_by(
            AbilityKind::VectorField,
            entities.get(id).unwrap().kind
        ));
    }
}

#[test]
fn vector_field_master_has_five_tile_square_instead_of_four() {
    let mut runtime = field((400.0, 400.0), [1.0, 0.0]);
    assert_eq!(runtime.vector_field_at(479.0, 479.0, 1), None);
    runtime
        .spawn_world_object(AbilityWorldObjectSpec {
            owner: 1,
            caster_id: 1000,
            ability: AbilityKind::VectorField,
            kind: AbilityWorldObjectKind::VectorField,
            x: 400.0,
            y: 400.0,
            created_tick: 0,
            expires_tick: 100,
            payload: AbilityObjectPayload::VectorField {
                direction: [1.0, 0.0],
                size_tiles: 5.0,
            },
        })
        .unwrap();
    assert_eq!(runtime.vector_field_at(479.0, 479.0, 1), Some([1.0, 0.0]));
    assert_eq!(runtime.vector_field_at(481.0, 400.0, 1), None);
}

#[test]
fn vector_field_overlap_sums_directions_and_excludes_expired_or_outside_fields() {
    let center = (400.0, 400.0);
    for (direction, expected) in [
        ([1.0, 0.0], [2.0, 0.0]),
        ([-1.0, 0.0], [0.0, 0.0]),
        ([0.0, 1.0], [1.0, 1.0]),
    ] {
        let mut runtime = field(center, [1.0, 0.0]);
        // Enemy and allied fields contribute equally.
        add_field(&mut runtime, center, direction, 2, 100);
        add_field(&mut runtime, center, [1.0, 0.0], 1, 1);
        add_field(&mut runtime, (800.0, 800.0), [1.0, 0.0], 1, 100);
        assert_eq!(
            runtime.vector_field_at(center.0, center.1, 1),
            Some(expected)
        );
    }
}

#[test]
fn vector_field_halves_each_units_progress_and_two_fields_stop_it() {
    let map = flat_map(2);
    let center = map.tile_center(10, 10);
    for (kind, upgrade) in [
        (EntityKind::Tank, None),
        (EntityKind::Tank, Some(UpgradeKind::TankPlus)),
        (EntityKind::ScoutCar, None),
        (EntityKind::Rifleman, None),
        (EntityKind::Rifleman, Some(UpgradeKind::Methamphetamines)),
    ] {
        let mut players = [player_with_oil(1, 0), player_with_oil(2, 0)];
        if let Some(upgrade) = upgrade {
            players[0].upgrades.insert(upgrade);
        }
        let speed = config::unit_stats(kind).unwrap().speed
            * match upgrade {
                Some(UpgradeKind::Methamphetamines) => config::METHAMPHETAMINES_SPEED_MULTIPLIER,
                Some(UpgradeKind::TankPlus) => config::TANK_PLUS_SPEED_MULTIPLIER,
                _ => 1.0,
            };
        for count in 0..=3 {
            let mut runtime = AbilityRuntime::new();
            for _ in 0..count {
                add_field(&mut runtime, center, [-1.0, 0.0], 2, 100);
            }
            let mut entities = EntityStore::new();
            let id = entities.spawn_unit(1, kind, center.0, center.1).unwrap();
            entities.get_mut(id).unwrap().set_facing(0.0);
            mark_moving(&mut entities, id, (center.0 + 300.0, center.1));
            let occ = Occupancy::build(&map, &entities);
            let smokes = SmokeCloudStore::new();
            for tick in 1..=10 {
                let spatial = SpatialIndex::build(&entities, map.width, map.height);
                movement_system_with_context(
                    &map,
                    &mut entities,
                    &players,
                    &occ,
                    &spatial,
                    tick,
                    &smokes,
                    &runtime,
                );
                let unit = entities.get(id).unwrap();
                let expected = center.0 + tick as f32 * speed * (1.0 - count as f32 * 0.5);
                assert!(
                    (unit.pos_x() - expected).abs() < 0.02,
                    "{kind:?} {upgrade:?} fields={count} tick={tick}: got {}, expected {expected}",
                    unit.pos_x()
                );
                assert!((unit.pos_y() - center.1).abs() < 0.02);
            }
        }
    }
}

#[test]
fn vector_field_stack_cannot_push_through_blocked_terrain() {
    let mut map = flat_map(2);
    map.unwalkable_tiles.push((11, 10));
    let center = map.tile_center(10, 10);
    let mut runtime = AbilityRuntime::new();
    // Both endpoints are clear, but the displacement crosses a blocked tile.
    for _ in 0..80 {
        add_field(&mut runtime, center, [1.0, 0.0], 1, 100);
    }
    let players = [player_with_oil(1, 0), player_with_oil(2, 0)];
    let mut entities = EntityStore::new();
    let id = entities
        .spawn_unit(1, EntityKind::Rifleman, center.0, center.1)
        .unwrap();
    let occ = Occupancy::build(&map, &entities);
    assert!(standability::unit_static_standable(
        &map,
        &occ,
        EntityKind::Rifleman,
        center.0 + 80.0 * config::unit_stats(EntityKind::Rifleman).unwrap().speed * 0.5,
        center.1,
    ));
    apply_magic_anchor_pull(&map, &mut entities, &players, &occ, 1, &runtime);
    assert_eq!(pos(&entities, id), center);
}

#[test]
fn vector_field_spares_active_trench_occupants_until_they_leave() {
    let map = flat_map(2);
    let center = map.tile_center(10, 10);
    let players = [player_with_oil(1, 0), player_with_oil(2, 0)];
    let mut runtime = field(center, [1.0, 0.0]);
    add_field(&mut runtime, center, [1.0, 0.0], 2, 100);
    for owner in [1, 2] {
        for kind in [
            EntityKind::Rifleman,
            EntityKind::Panzerfaust,
            EntityKind::MachineGunner,
        ] {
            let mut entities = EntityStore::new();
            let id = entities
                .spawn_unit(owner, kind, center.0, center.1)
                .unwrap();
            entities
                .get_mut(id)
                .unwrap()
                .movement
                .as_mut()
                .unwrap()
                .occupied_trench_id = Some(1);
            let occ = Occupancy::build(&map, &entities);
            apply_magic_anchor_pull(&map, &mut entities, &players, &occ, 1, &runtime);
            assert_eq!(pos(&entities, id), center, "{kind:?} owner={owner}");
            assert_eq!(entities.get(id).unwrap().movement_delta(), (0.0, 0.0));
            assert_eq!(
                crate::game::entity::active_trench_occupation(entities.get(id).unwrap()),
                Some(1)
            );

            // Dig-in progress alone does not grant immunity after occupation ends.
            let movement = entities.get_mut(id).unwrap().movement.as_mut().unwrap();
            movement.occupied_trench_id = None;
            movement.entrenchment_dig_ticks = config::ENTRENCHMENT_DIG_IN_TICKS - 1;
            apply_magic_anchor_pull(&map, &mut entities, &players, &occ, 2, &runtime);
            let (x, y) = pos(&entities, id);
            assert!(
                (x - center.0 - config::unit_stats(kind).unwrap().speed).abs() < 0.001,
                "{kind:?} owner={owner}"
            );
            assert_eq!(y, center.1);
        }
    }
}
