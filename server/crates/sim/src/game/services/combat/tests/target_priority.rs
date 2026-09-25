use super::*;

#[test]
fn tank_prefers_nearby_unit_over_armored_command_center() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let resource_depot = entities
        .spawn_building(2, EntityKind::ResourceDepot, 160.0, 100.0, true)
        .expect("resource depot should spawn");
    let worker = entities
        .spawn_unit(2, EntityKind::Worker, 100.0, 180.0)
        .expect("worker should spawn");
    entities
        .get_mut(tank)
        .expect("tank should exist")
        .set_order(Order::attack_move_to(300.0, 100.0));

    let target = resolve_tank_test_target(&map, &entities, &default_team_relations(), tank);

    assert_eq!(target, Some(worker));
    assert_ne!(target, Some(resource_depot));
}

#[test]
fn machine_gunner_prefers_farther_rifleman_over_nearer_worker() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let machine_gunner = entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
        .expect("machine gunner should spawn");
    entities
        .spawn_unit(2, EntityKind::Worker, 120.0, 100.0)
        .expect("worker should spawn");
    let rifleman = entities
        .spawn_unit(2, EntityKind::Rifleman, 160.0, 100.0)
        .expect("rifleman should spawn");
    entities
        .get_mut(machine_gunner)
        .expect("machine gunner should exist")
        .set_order(Order::attack_move_to(300.0, 100.0));

    assert_eq!(
        resolve_test_target(
            &map,
            &entities,
            &default_team_relations(),
            machine_gunner,
            192.0,
        ),
        Some(rifleman)
    );
}

fn rifleman_attacking_building() -> (Map, EntityStore, u32, u32) {
    let map = open_map(24);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman");
    let building = entities
        .spawn_building(2, EntityKind::Depot, 180.0, 100.0, true)
        .expect("building");
    entities
        .get_mut(attacker)
        .expect("rifleman")
        .set_order(Order::attack_move_to(600.0, 100.0));
    tick_building_engagement(&map, &mut entities);
    assert_eq!(
        entities.get(attacker).expect("rifleman").target_id(),
        Some(building)
    );
    (map, entities, attacker, building)
}

fn tick_building_engagement(map: &Map, entities: &mut EntityStore) {
    run_combat_tick_on_map(
        entities,
        &[player_state(1, false), player_state(2, false)],
        map,
    );
}

#[test]
fn attack_move_retargets_building_to_new_defender_during_cooldown() {
    let (map, mut entities, attacker, building) = rifleman_attacking_building();
    let weapon = combat_rules::default_weapon_kind(EntityKind::Rifleman).expect("weapon");
    entities
        .get_mut(attacker)
        .expect("rifleman")
        .set_weapon_cooldown(weapon, 20);
    let defender = entities
        .spawn_unit(2, EntityKind::Rifleman, 100.0, 180.0)
        .expect("defender");
    tick_building_engagement(&map, &mut entities);
    let unit = entities.get(attacker).expect("rifleman");
    assert_eq!(unit.target_id(), Some(defender));
    assert_eq!(unit.move_intent(), Some((600.0, 100.0)));

    // A retreating defender must not drag the attack-move away from its objective.
    let enemy = entities.get_mut(defender).expect("defender");
    enemy.pos_y = 600.0;
    entities
        .get_mut(attacker)
        .expect("rifleman")
        .set_weapon_cooldown(weapon, 0);
    tick_building_engagement(&map, &mut entities);
    let unit = entities.get(attacker).expect("rifleman");
    assert_eq!(unit.target_id(), Some(building));
    assert_eq!(unit.move_intent(), Some((600.0, 100.0)));
    assert!(unit.path_is_empty(), "must not pursue the defender");
}

#[test]
fn attack_move_keeps_building_without_an_in_range_enemy_combat_unit() {
    for (owner, kind, y) in [
        (2, EntityKind::Rifleman, 600.0),
        (1, EntityKind::Rifleman, 180.0),
        (2, EntityKind::Worker, 180.0),
    ] {
        let (map, mut entities, attacker, building) = rifleman_attacking_building();
        entities
            .spawn_unit(owner, kind, 100.0, y)
            .expect("other unit");
        tick_building_engagement(&map, &mut entities);
        assert_eq!(
            entities.get(attacker).expect("rifleman").target_id(),
            Some(building)
        );
    }
}

#[test]
fn direct_attack_keeps_building_priority_over_new_defender() {
    for building_x in [180.0, 400.0] {
        let (map, mut entities, attacker, building) = rifleman_attacking_building();
        entities.get_mut(building).expect("building").pos_x = building_x;
        let unit = entities.get_mut(attacker).expect("rifleman");
        unit.set_order(Order::attack(building));
        let weapon = combat_rules::default_weapon_kind(EntityKind::Rifleman).expect("weapon");
        unit.set_weapon_cooldown(weapon, 0);
        let defender = entities
            .spawn_unit(2, EntityKind::Rifleman, 100.0, 180.0)
            .expect("defender");
        tick_building_engagement(&map, &mut entities);
        let unit = entities.get(attacker).expect("rifleman");
        assert_eq!(unit.order().attack_target(), Some(building));
        assert_ne!(unit.target_id(), Some(defender));
        if building_x == 180.0 {
            assert_eq!(unit.target_id(), Some(building));
        } else {
            assert!(
                !unit.path_is_empty(),
                "direct attack must still pursue its target"
            );
        }
    }
}

#[test]
fn attack_move_does_not_switch_building_target_to_smoke_hidden_defender() {
    let (map, mut entities, attacker, building) = rifleman_attacking_building();
    entities
        .spawn_unit(2, EntityKind::Rifleman, 100.0, 200.0)
        .expect("defender");
    let mut smokes = SmokeCloudStore::new();
    smokes.spawn(100.0, 165.0, 0.75, 100, 0).expect("smoke");
    run_combat_tick_on_map_with_seed_and_smokes(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
        0,
        &smokes,
    );
    assert_eq!(
        entities.get(attacker).expect("rifleman").target_id(),
        Some(building)
    );
}
