use super::*;

const COAX_TARGET_X: f32 = 288.0;

fn prepare_coax_tank(entities: &mut EntityStore, tank: u32) {
    let tank = entities.get_mut(tank).expect("tank should exist");
    tank.set_order(Order::HoldPosition);
    tank.set_facing(0.0);
    tank.set_weapon_facing(0.0);
    tank.set_weapon_cooldown(combat_rules::WeaponKind::TankCannon, 99);
}

fn attack_weapon_kinds(
    events: &HashMap<u32, Vec<Event>>,
    recipient: u32,
    attacker: u32,
) -> Vec<String> {
    events
        .get(&recipient)
        .into_iter()
        .flat_map(|events| events.iter())
        .filter_map(|event| match event {
            Event::Attack {
                from, weapon_kind, ..
            } if *from == attacker => weapon_kind.clone(),
            _ => None,
        })
        .collect()
}

#[test]
fn tank_coax_profile_is_live_without_replacing_tank_cannon() {
    let coax = combat_rules::weapon_profile(combat_rules::WeaponKind::TankCoax)
        .expect("Tank coax profile should be live");
    assert_eq!(coax.range_tiles, 6);
    assert_eq!(coax.dmg, 4);
    assert_eq!(coax.cooldown, 6);
    assert_eq!(
        coax.weapon_class,
        crate::rules::defs::WeaponClass::SmallArms
    );
    assert_eq!(
        combat_rules::default_weapon_profile(EntityKind::Tank)
            .expect("Tank default weapon should exist")
            .id,
        combat_rules::WeaponKind::TankCannon
    );
}

#[test]
fn tank_coax_fires_in_arc_with_small_arms_damage_and_weapon_event() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let worker = entities
        .spawn_unit(2, EntityKind::Worker, COAX_TARGET_X, 100.0)
        .expect("worker should spawn");
    prepare_coax_tank(&mut entities, tank);
    let worker_hp_before = entities.get(worker).expect("worker should exist").hp;

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        worker_hp_before - entities.get(worker).expect("worker should exist").hp,
        4,
        "coax should deal its small-arms base damage to infantry"
    );
    let tank_entity = entities.get(tank).expect("tank should exist");
    assert_eq!(
        tank_entity.weapon_cooldown(combat_rules::WeaponKind::TankCoax),
        6
    );
    assert_eq!(
        tank_entity.weapon_cooldown(combat_rules::WeaponKind::TankCannon),
        98,
        "coax fire must not reset the cannon cooldown"
    );
    assert_eq!(
        attack_weapon_kinds(&events, 1, tank),
        vec!["tank_coax".to_string()]
    );
}

#[test]
fn tank_coax_attack_events_remain_fog_projected() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let worker = entities
        .spawn_unit(2, EntityKind::Worker, COAX_TARGET_X, 100.0)
        .expect("worker should spawn");
    prepare_coax_tank(&mut entities, tank);
    let worker_hp_before = entities.get(worker).expect("worker should exist").hp;

    let events = run_combat_tick_on_map(
        &mut entities,
        &[
            player_state(1, false),
            player_state(2, false),
            player_state(3, false),
        ],
        &map,
    );

    assert_eq!(
        worker_hp_before - entities.get(worker).expect("worker should exist").hp,
        4
    );
    assert_eq!(
        attack_weapon_kinds(&events, 1, tank),
        vec!["tank_coax".to_string()],
        "the attacker team should receive coax feedback"
    );
    assert_eq!(
        attack_weapon_kinds(&events, 2, tank),
        vec!["tank_coax".to_string()],
        "the visible victim owner should receive coax feedback"
    );
    assert!(
        attack_weapon_kinds(&events, 3, tank).is_empty(),
        "hidden third-party viewers must not receive coax weapon hints"
    );
}

#[test]
fn tank_coax_fallback_vehicle_damage_stays_small_arms() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let enemy_tank = entities
        .spawn_unit(2, EntityKind::Tank, COAX_TARGET_X, 100.0)
        .expect("enemy tank should spawn");
    prepare_coax_tank(&mut entities, tank);
    if let Some(target) = entities.get_mut(enemy_tank) {
        target.set_facing(std::f32::consts::PI);
    }
    let hp_before = entities.get(enemy_tank).expect("target should exist").hp;

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        hp_before - entities.get(enemy_tank).expect("target should exist").hp,
        1,
        "coax fallback shots against armor should use small-arms reduction, not AP damage"
    );
}

#[test]
fn tank_coax_overpenetration_uses_small_arms_profile_without_extra_attack_event() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let primary = entities
        .spawn_unit(2, EntityKind::Worker, COAX_TARGET_X, 100.0)
        .expect("primary should spawn");
    let secondary = entities
        .spawn_unit(2, EntityKind::Worker, 320.0, 100.0)
        .expect("secondary should spawn");
    prepare_coax_tank(&mut entities, tank);
    let primary_before = entities.get(primary).expect("primary should exist").hp;
    let secondary_before = entities.get(secondary).expect("secondary should exist").hp;

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        primary_before - entities.get(primary).expect("primary should exist").hp,
        4
    );
    assert_eq!(
        secondary_before - entities.get(secondary).expect("secondary should exist").hp,
        2,
        "coax overpenetration should use the small-arms coax profile, not Tank cannon damage"
    );
    let player_events = events.get(&1).expect("attacker events should exist");
    assert!(
        player_events
            .iter()
            .any(|event| matches!(event, Event::Overpenetration { to } if *to == secondary)),
        "secondary coax hit should emit overpenetration feedback"
    );
    assert!(
        player_events
            .iter()
            .all(|event| !matches!(event, Event::Attack { to, .. } if *to == secondary)),
        "secondary coax overpenetration should not emit a separate attack event"
    );
}

#[test]
fn tank_coax_prioritizes_infantry_over_nearer_fallback_targets() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let nearer_tank = entities
        .spawn_unit(2, EntityKind::Tank, 250.0, 118.0)
        .expect("enemy tank should spawn");
    let worker = entities
        .spawn_unit(2, EntityKind::Worker, COAX_TARGET_X, 100.0)
        .expect("worker should spawn");
    prepare_coax_tank(&mut entities, tank);
    let tank_hp_before = entities.get(nearer_tank).expect("tank should exist").hp;
    let worker_hp_before = entities.get(worker).expect("worker should exist").hp;

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        worker_hp_before - entities.get(worker).expect("worker should exist").hp,
        4,
        "infantry-priority targets should beat nearer fallback vehicles"
    );
    assert_eq!(
        entities.get(nearer_tank).expect("tank should exist").hp,
        tank_hp_before,
        "nearer fallback target should not be hit while an infantry-priority target is legal"
    );
}

#[test]
fn tank_coax_rejects_targets_outside_turret_arc() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let worker = entities
        .spawn_unit(2, EntityKind::Worker, 278.0, 165.0)
        .expect("worker should spawn");
    prepare_coax_tank(&mut entities, tank);
    let worker_hp_before = entities.get(worker).expect("worker should exist").hp;

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities.get(worker).expect("worker should exist").hp,
        worker_hp_before
    );
    assert!(
        attack_weapon_kinds(&events, 1, tank).is_empty(),
        "coax must not fire outside its 10-degree half arc"
    );
}

#[test]
fn tank_coax_rejects_infantry_when_enemy_hard_blocker_would_take_the_shot() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let blocker = entities
        .spawn_unit(2, EntityKind::Tank, 180.0, 100.0)
        .expect("enemy blocker should spawn");
    let worker = entities
        .spawn_unit(2, EntityKind::Worker, COAX_TARGET_X, 100.0)
        .expect("worker should spawn");
    prepare_coax_tank(&mut entities, tank);
    if let Some(blocker) = entities.get_mut(blocker) {
        blocker.set_facing(std::f32::consts::PI);
    }
    let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
    let worker_hp_before = entities.get(worker).expect("worker should exist").hp;

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        blocker_hp_before - entities.get(blocker).expect("blocker should exist").hp,
        1,
        "the intervening hard blocker should be selected as the legal fallback target"
    );
    assert_eq!(
        entities.get(worker).expect("worker should exist").hp,
        worker_hp_before,
        "coax should not pick an intended infantry target hidden behind a hard blocker"
    );
}

#[test]
fn tank_cannon_and_coax_same_tick_emit_cannon_before_coax() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Tank, 150.0, 100.0)
        .expect("target should spawn");
    if let Some(tank_entity) = entities.get_mut(tank) {
        tank_entity.set_order(Order::attack(target));
        tank_entity.set_facing(0.0);
        tank_entity.set_weapon_facing(0.0);
    }
    if let Some(target_entity) = entities.get_mut(target) {
        target_entity.set_facing(std::f32::consts::PI);
    }
    let hp_before = entities.get(target).expect("target should exist").hp;

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        hp_before - entities.get(target).expect("target should exist").hp,
        61,
        "same-tick cannon and coax should both damage a surviving armored target"
    );
    assert_eq!(
        attack_weapon_kinds(&events, 1, tank),
        vec!["tank_cannon".to_string(), "tank_coax".to_string()],
        "Tank cannon attack feedback must be emitted before the same-tick coax feedback"
    );
    let tank_entity = entities.get(tank).expect("tank should exist");
    assert_eq!(
        tank_entity.weapon_cooldown(combat_rules::WeaponKind::TankCannon),
        72
    );
    assert_eq!(
        tank_entity.weapon_cooldown(combat_rules::WeaponKind::TankCoax),
        6
    );
}

#[test]
fn tank_coax_does_not_mutate_tank_target_or_path_state() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let worker = entities
        .spawn_unit(2, EntityKind::Worker, COAX_TARGET_X, 100.0)
        .expect("worker should spawn");
    if let Some(tank_entity) = entities.get_mut(tank) {
        tank_entity.set_order(Order::move_to(300.0, 100.0));
        tank_entity.set_path(vec![(300.0, 100.0)]);
        tank_entity.set_path_goal(Some((300.0, 100.0)));
        tank_entity.mark_move_phase(MovePhase::Moving);
        tank_entity.set_facing(0.0);
        tank_entity.set_weapon_facing(0.0);
        tank_entity.set_weapon_cooldown(combat_rules::WeaponKind::TankCannon, 99);
    }
    let worker_hp_before = entities.get(worker).expect("worker should exist").hp;

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        worker_hp_before - entities.get(worker).expect("worker should exist").hp,
        4
    );
    let tank_entity = entities.get(tank).expect("tank should exist");
    assert_eq!(
        tank_entity.target_id(),
        None,
        "coax opportunity fire should not claim the Tank cannon target slot"
    );
    assert_eq!(tank_entity.path_goal(), Some((300.0, 100.0)));
    assert!(
        !tank_entity.path_is_empty(),
        "coax opportunity fire should not clear or replace movement paths"
    );
}
