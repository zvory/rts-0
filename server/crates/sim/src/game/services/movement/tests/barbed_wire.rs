use super::super::barbed_wire::{apply_entries, positions};
use super::*;

#[test]
fn barbed_wire_two_mixed_crossings_and_reentry() {
    for first in [EntityKind::Worker, EntityKind::MortarTeam] {
        for second in [
            EntityKind::Rifleman,
            EntityKind::Tank,
            EntityKind::MortarTeam,
        ] {
            let map = flat_map(2);
            let mut entities = EntityStore::new();
            let wire = entities
                .spawn_building(1, EntityKind::BarbedWire, 176.0, 176.0, true)
                .unwrap();
            let a = entities.spawn_unit(1, first, 144.0, 176.0).unwrap();
            let b = entities.spawn_unit(2, second, 144.0, 208.0).unwrap();
            let initial_a = entities.get(a).unwrap().hp;
            let initial_b = entities.get(b).unwrap().hp;
            let before = positions(&map, &entities);
            entities.get_mut(a).unwrap().set_position(176.0, 176.0);
            apply_entries(&map, &mut entities, &before);
            assert_eq!(entities.get(wire).unwrap().hp, 1);
            assert_eq!(
                entities.get(a).unwrap().hp,
                initial_a - if first == EntityKind::Tank { 0 } else { 10 }
            );
            let before = positions(&map, &entities);
            apply_entries(&map, &mut entities, &before);
            assert_eq!(
                entities.get(wire).unwrap().hp,
                1,
                "standing still must not consume wire"
            );
            entities.get_mut(b).unwrap().set_position(176.0, 176.0);
            assert!(apply_entries(&map, &mut entities, &before));
            assert_eq!(entities.get(wire).unwrap().hp, 0);
            crate::game::services::death::death_system(
                &map,
                &mut entities,
                &crate::game::fog::Fog::new(map.width, map.height),
                &SmokeCloudStore::new(),
                &crate::game::teams::TeamRelations::from_player_teams([]),
                &mut [],
                &mut Vec::new(),
                &mut crate::game::ground_decal::GroundDecalStore::new(),
                &mut std::collections::HashMap::new(),
                1,
            );
            assert!(
                entities.get(wire).is_none(),
                "untargetable exhausted wire must be removed"
            );
            assert_eq!(
                entities.get(b).unwrap().hp,
                initial_b - if second == EntityKind::Tank { 0 } else { 10 }
            );
        }
    }
    let map = flat_map(2);
    let mut entities = EntityStore::new();
    let wire = entities
        .spawn_building(1, EntityKind::BarbedWire, 176.0, 176.0, true)
        .unwrap();
    let unit = entities
        .spawn_unit(1, EntityKind::Worker, 144.0, 176.0)
        .unwrap();
    for x in [176.0, 144.0, 176.0] {
        let before = positions(&map, &entities);
        entities.get_mut(unit).unwrap().set_position(x, 176.0);
        apply_entries(&map, &mut entities, &before);
    }
    assert_eq!(entities.get(wire).unwrap().hp, 0);
}

#[test]
fn barbed_wire_is_untargetable_immune_and_scaffolds_are_inactive() {
    let map = flat_map(2);
    let mut entities = EntityStore::new();
    let wire = entities
        .spawn_building(1, EntityKind::BarbedWire, 176.0, 176.0, false)
        .unwrap();
    let unit = entities
        .spawn_unit(1, EntityKind::Worker, 144.0, 176.0)
        .unwrap();
    let hp = entities.get(wire).unwrap().hp;
    assert!(!entities.get(wire).unwrap().is_targetable());
    assert!(!entities.get_mut(wire).unwrap().apply_damage(999, None));
    let before = positions(&map, &entities);
    entities.get_mut(unit).unwrap().set_position(176.0, 176.0);
    apply_entries(&map, &mut entities, &before);
    assert_eq!(entities.get(wire).unwrap().hp, hp);
    while entities.get(wire).unwrap().under_construction() {
        entities.get_mut(wire).unwrap().advance_construction();
    }
    assert_eq!(entities.get(wire).unwrap().owner, 0);
    assert_eq!(entities.get(wire).unwrap().hp, 2);
    let before = positions(&map, &entities);
    apply_entries(&map, &mut entities, &before);
    assert_eq!(
        entities.get(wire).unwrap().hp,
        2,
        "completion beneath a unit is not entry"
    );
}

#[test]
fn barbed_wire_slows_only_infantry_and_mortars_and_is_walkable() {
    for kind in [
        EntityKind::Worker,
        EntityKind::Rifleman,
        EntityKind::MortarTeam,
        EntityKind::Tank,
        EntityKind::AntiTankGun,
    ] {
        let map = flat_map(2);
        let mut entities = EntityStore::new();
        let wire = entities
            .spawn_building(1, EntityKind::BarbedWire, 176.0, 176.0, true)
            .unwrap();
        let unit = entities.spawn_unit(1, kind, 176.0, 176.0).unwrap();
        mark_moving(&mut entities, unit, (240.0, 176.0));
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        movement_system(&map, &mut entities, &[], &occ, &spatial, 1);
        let moved = entities.get(unit).unwrap().pos_x() - 176.0;
        // Pivot-drive units can accelerate/steer; compare identical empty-tile movement.
        let mut bare = EntityStore::new();
        let other = bare.spawn_unit(1, kind, 176.0, 176.0).unwrap();
        mark_moving(&mut bare, other, (240.0, 176.0));
        let occ = Occupancy::build(&map, &bare);
        let spatial = SpatialIndex::build(&bare, map.width, map.height);
        movement_system(&map, &mut bare, &[], &occ, &spatial, 1);
        let expected = (bare.get(other).unwrap().pos_x() - 176.0)
            * if rts_rules::barbed_wire_affects(kind) {
                0.7
            } else {
                1.0
            };
        assert!(moved > 0.0);
        assert!(
            (moved - expected).abs() < 0.001,
            "{kind}: {moved} vs {expected}"
        );
        assert_eq!(entities.get(wire).unwrap().hp, 2);
    }
}

#[test]
fn tank_crushes_fresh_or_used_wire_on_first_entry_without_damage() {
    for owner in [1, 2] {
        for used in [false, true] {
            let map = flat_map(2);
            let mut entities = EntityStore::new();
            let wire = entities
                .spawn_building(1, EntityKind::BarbedWire, 176.0, 176.0, true)
                .unwrap();
            if used {
                entities
                    .get_mut(wire)
                    .unwrap()
                    .consume_wire_crossing(EntityKind::Worker);
            }
            let tank = entities
                .spawn_unit(owner, EntityKind::Tank, 144.0, 176.0)
                .unwrap();
            let hp = entities.get(tank).unwrap().hp;
            let before = positions(&map, &entities);
            entities.get_mut(tank).unwrap().set_position(176.0, 176.0);
            assert!(
                apply_entries(&map, &mut entities, &before),
                "crushing requests immediate death cleanup"
            );
            assert_eq!(
                entities.get(wire).unwrap().hp,
                0,
                "a tank exhausts the entire segment on entry"
            );
            assert_eq!(
                entities.get(tank).unwrap().hp,
                hp,
                "wire cannot hurt the tank"
            );

            let worker = entities
                .spawn_unit(owner, EntityKind::Worker, 144.0, 176.0)
                .unwrap();
            let worker_hp = entities.get(worker).unwrap().hp;
            let before = positions(&map, &entities);
            entities.get_mut(worker).unwrap().set_position(176.0, 176.0);
            apply_entries(&map, &mut entities, &before);
            assert_eq!(
                entities.get(worker).unwrap().hp,
                worker_hp,
                "crushed wire is inactive before cleanup"
            );
        }
    }
}
