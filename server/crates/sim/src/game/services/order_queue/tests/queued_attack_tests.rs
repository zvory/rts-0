use super::*;

#[test]
fn queued_attack_skips_when_target_is_not_targetable() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let node = entities
        .spawn_node(EntityKind::Steel, 160.0, 100.0)
        .expect("node should spawn");
    {
        let unit = entities.get_mut(attacker).expect("attacker should exist");
        unit.append_queued_order(OrderIntent::attack(node));
        unit.append_queued_order(OrderIntent::attack_move_to(220.0, 100.0));
    }
    let players = vec![player_state(1), player_state(2)];

    promote_with_players(&map, &mut entities, &players);

    let unit = entities.get(attacker).expect("attacker should exist");
    assert!(
        matches!(unit.order(), Order::AttackMove(_)),
        "non-targetable attack target should be skipped and next attack-move promoted"
    );
    assert!(unit.queued_orders().is_empty());
}

#[test]
fn queued_attack_promotes_owned_targets() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let own_building = entities
        .spawn_building(1, EntityKind::Barracks, 160.0, 100.0, true)
        .expect("owned building should spawn");
    {
        let unit = entities.get_mut(attacker).expect("attacker should exist");
        unit.append_queued_order(OrderIntent::attack(own_building));
    }
    let players = vec![player_state(1), player_state(2)];

    promote_with_players(&map, &mut entities, &players);

    let unit = entities.get(attacker).expect("attacker should exist");
    assert_eq!(
        unit.order().attack_target(),
        Some(own_building),
        "queued explicit attacks should promote owned targets"
    );
    assert!(unit.queued_orders().is_empty());
}

#[test]
fn stationary_attack_that_cannot_fire_promotes_next_queued_order() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Rifleman, 700.0, 100.0)
        .expect("target should spawn");
    {
        let unit = entities.get_mut(attacker).expect("attacker should exist");
        unit.set_order(Order::attack(target));
        unit.set_target_id(Some(target));
        unit.append_queued_order(OrderIntent::move_to(220.0, 100.0));
    }
    let players = vec![player_state(1), player_state(2)];

    promote_with_players(&map, &mut entities, &players);

    let unit = entities.get(attacker).expect("attacker should exist");
    assert!(
        matches!(unit.order(), Order::Move(_)),
        "a stationary attack that cannot fire should not stall the queued move forever"
    );
    assert!(unit.queued_orders().is_empty());
}
