use super::*;

#[test]
fn carry_through_requires_meaningful_ray_overlap() {
    for (side_offset, should_hit) in [(0.0, true), (4.0, true), (8.0, false)] {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("attacker should spawn");
        let primary = entities
            .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
            .expect("primary should spawn");
        let secondary = entities
            .spawn_unit(2, EntityKind::Worker, 165.0, 100.0 + side_offset)
            .expect("secondary should spawn");
        let secondary_hp = entities.get(secondary).expect("secondary should exist").hp;
        let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

        apply_test_damage(
            &mut entities,
            &mut events,
            attacker,
            primary,
            10,
            1,
            100.0,
            100.0,
            140.0,
            100.0,
            128.0,
        );

        assert_eq!(
            entities.get(secondary).expect("secondary should exist").hp < secondary_hp,
            should_hit,
            "unexpected carry-through result at {side_offset}px from the shot ray"
        );
        assert_eq!(
            events.get(&1).is_some_and(|events| events
                .iter()
                .any(|event| matches!(event, Event::Overpenetration { to } if *to == secondary))),
            should_hit,
            "secondary feedback must match damage at {side_offset}px"
        );
    }
}

#[test]
fn grazing_tank_does_not_absorb_carry_through() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let primary = entities
        .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
        .expect("primary should spawn");
    let grazing_tank = entities
        .spawn_unit(2, EntityKind::Tank, 180.0, 114.0)
        .expect("tank should spawn");
    let secondary = entities
        .spawn_unit(2, EntityKind::Worker, 220.0, 100.0)
        .expect("secondary should spawn");
    let tank_hp = entities.get(grazing_tank).expect("tank should exist").hp;
    let secondary_hp = entities.get(secondary).expect("secondary should exist").hp;
    let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

    apply_test_damage(
        &mut entities,
        &mut events,
        attacker,
        primary,
        20,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        400.0,
    );

    assert_eq!(
        entities.get(grazing_tank).expect("tank should exist").hp,
        tank_hp
    );
    assert!(entities.get(secondary).expect("secondary should exist").hp < secondary_hp);
}
