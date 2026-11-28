use super::*;

#[test]
fn bow_novice_fires_rifle_strength_arrows_at_range_and_earns_actual_damage_insight() {
    let map = open_map(24);
    let mut entities = EntityStore::new();
    let bow = entities
        .spawn_unit(1, EntityKind::BowNovice, 160.0, 160.0)
        .unwrap();
    let enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 288.0, 160.0)
        .unwrap();
    let players = [player_state(1, false), player_state(2, false)];
    let profile = effective_attack_profile(entities.get(bow).unwrap());
    assert_eq!(
        (profile.range_tiles, profile.dmg, profile.cooldown),
        (5.0, 5, 32)
    );
    assert_eq!(profile.weapon.unwrap().armor_penetration, 0.0);
    let events = run_combat_tick_on_map(&mut entities, &players, &map);
    assert_eq!(entities.get(enemy).unwrap().hp, 40);
    assert_eq!(entities.get(bow).unwrap().insight, 5);
    assert_eq!(entities.get(bow).unwrap().attack_cd(), 32);
    assert!(events.values().flatten().any(|event| matches!(event,
        Event::Attack { from, weapon_kind: Some(kind), .. }
        if *from == bow && kind == "disciple_arrow")));
    let before = entities.get(enemy).unwrap().hp;
    run_combat_tick_on_map(&mut entities, &players, &map);
    assert_eq!(
        entities.get(enemy).unwrap().hp,
        before,
        "cannot fire during reload"
    );
    let bow = entities.get_mut(bow).unwrap();
    bow.try_start_novice_charge((288.0, 160.0), (0.0, 0.0), true);
    assert_eq!(bow.movement_speed(), 2.0);
    assert!(!can_fire_while_moving(bow, true));
}
