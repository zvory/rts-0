use super::*;

#[test]
fn warrior_sword_two_shots_rifleman_without_overpenetration() {
    let mut entities = EntityStore::new();
    let warrior = entities
        .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
        .expect("Warrior should spawn");
    let rifleman = entities
        .spawn_unit(2, EntityKind::Rifleman, 125.0, 100.0)
        .expect("Rifleman should spawn");
    entities
        .get_mut(warrior)
        .expect("Warrior should exist")
        .set_order(Order::attack(rifleman));
    entities
        .get_mut(rifleman)
        .expect("Rifleman should exist")
        .set_order(Order::HoldPosition);

    let mut all_events = run_combat_tick(&mut entities);
    assert_eq!(entities.get(rifleman).map(|unit| unit.hp), Some(22));

    for _ in 0..32 {
        let tick_events = run_combat_tick(&mut entities);
        for (player, events) in tick_events {
            all_events.entry(player).or_default().extend(events);
        }
    }

    assert_eq!(entities.get(rifleman).map(|unit| unit.hp), Some(0));
    let owner_events = all_events
        .get(&1)
        .expect("Warrior owner should receive events");
    assert_eq!(
        owner_events
            .iter()
            .filter(|event| matches!(
                event,
                Event::Attack {
                    from,
                    to,
                    weapon_kind: Some(weapon_kind),
                    ..
                } if *from == warrior && *to == rifleman && weapon_kind == "warrior_sword"
            ))
            .count(),
        2
    );
    assert!(owner_events
        .iter()
        .all(|event| !matches!(event, Event::Overpenetration { .. })));
}
