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
    assert_eq!(coax.range_tiles, 6.0);
    assert_eq!(coax.dmg, 4);
    assert_eq!(coax.cooldown, 12);
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
        7,
        "three seeded burst rays should hit this infantry target"
    );
    let tank_entity = entities.get(tank).expect("tank should exist");
    assert_eq!(
        tank_entity.weapon_cooldown(combat_rules::WeaponKind::TankCoax),
        12
    );
    assert_eq!(
        tank_entity.weapon_cooldown(combat_rules::WeaponKind::TankCannon),
        98,
        "coax fire must not reset the cannon cooldown"
    );
    assert_eq!(
        attack_weapon_kinds(&events, 1, tank),
        vec!["tank_coax".to_string(); 3]
    );
}

#[test]
fn clear_obstacle_area_coax_bullets_can_be_intercepted_by_infantry() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let worker = entities
        .spawn_unit(2, EntityKind::Worker, 180.0, 100.0)
        .expect("worker should spawn");
    let trap = entities
        .spawn_building(2, EntityKind::TankTrap, 220.0, 100.0, true)
        .expect("Tank Trap should spawn");
    prepare_coax_tank(&mut entities, tank);
    if let Some(tank) = entities.get_mut(tank) {
        tank.set_order(Order::clear_obstacle_area_to(
            160.0, 100.0, trap, 220.0, 100.0,
        ));
    }
    let worker_hp = entities.get(worker).expect("worker should exist").hp;
    let trap_hp = entities.get(trap).expect("trap should exist").hp;

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities.get(worker).expect("worker should exist").hp,
        worker_hp - 7
    );
    assert_eq!(entities.get(trap).expect("trap should exist").hp, trap_hp);
}

#[test]
fn moving_breakthrough_halves_tank_coax_cooldown() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    entities
        .spawn_unit(2, EntityKind::Worker, COAX_TARGET_X, 100.0)
        .expect("worker should spawn");
    prepare_coax_tank(&mut entities, tank);
    if let Some(tank) = entities.get_mut(tank) {
        tank.set_movement_delta(2.0, 0.0);
        tank.start_breakthrough(config::BREAKTHROUGH_DURATION_TICKS);
    }

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities
            .get(tank)
            .expect("tank should survive")
            .weapon_cooldown(combat_rules::WeaponKind::TankCoax),
        combat_rules::weapon_profile(combat_rules::WeaponKind::TankCoax)
            .expect("coax profile should exist")
            .cooldown
            / 2,
        "Breakthrough should halve every weapon cooldown fired while moving"
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
        7
    );
    assert_eq!(
        attack_weapon_kinds(&events, 1, tank),
        vec!["tank_coax".to_string(); 3],
        "the attacker team should receive coax feedback"
    );
    assert_eq!(
        attack_weapon_kinds(&events, 2, tank),
        vec!["tank_coax".to_string(); 3],
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
fn tank_coax_pairwise_precedence_alternates_by_tick() {
    fn resolve_lethal_exchange(tick: u32) -> (u32, u32) {
        let map = open_map(16);
        let mut entities = EntityStore::new();
        let lower_id = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let higher_id = entities
            .spawn_unit(2, EntityKind::Tank, 180.0, 100.0)
            .expect("tank should spawn");
        prepare_coax_tank(&mut entities, lower_id);
        prepare_coax_tank(&mut entities, higher_id);
        {
            let lower = entities.get_mut(lower_id).expect("tank should exist");
            assert!(lower.apply_damage(lower.hp.saturating_sub(1), None));
        }
        {
            let higher = entities.get_mut(higher_id).expect("tank should exist");
            assert!(higher.apply_damage(higher.hp.saturating_sub(1), None));
            higher.set_facing(std::f32::consts::PI);
            higher.set_weapon_facing(std::f32::consts::PI);
            higher.combat.as_mut().expect("combat").commander_facing = std::f32::consts::PI;
        }
        let smokes = SmokeCloudStore::new();
        run_combat_tick_on_map_with_seed_smokes_and_tick(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
            0,
            &smokes,
            tick,
        );
        (
            entities.get(lower_id).expect("tank should exist").hp,
            entities.get(higher_id).expect("tank should exist").hp,
        )
    }

    assert_eq!(resolve_lethal_exchange(10), (1, 0));
    assert_eq!(resolve_lethal_exchange(11), (0, 1));
}

#[test]
fn tank_coax_burst_does_not_reach_a_body_beyond_weapon_range() {
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
        7
    );
    assert_eq!(
        secondary_before - entities.get(secondary).expect("secondary should exist").hp,
        0,
        "coax must not damage the rear unit beyond weapon range"
    );
    let player_events = events.get(&1).expect("attacker events should exist");
    assert!(
        player_events
            .iter()
            .all(|event| !matches!(event, Event::Attack { to, .. } if *to == secondary)),
        "secondary coax target should not emit a separate attack event"
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
    let infantry = entities
        .spawn_unit(2, EntityKind::Rifleman, COAX_TARGET_X, 100.0)
        .expect("infantry should spawn");
    prepare_coax_tank(&mut entities, tank);
    let tank_hp_before = entities.get(nearer_tank).expect("tank should exist").hp;
    let infantry_hp_before = entities.get(infantry).expect("infantry should exist").hp;

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        infantry_hp_before - entities.get(infantry).expect("infantry should exist").hp,
        7,
        "infantry-priority aim should send three seeded rays into the infantry"
    );
    assert_eq!(
        entities.get(nearer_tank).expect("tank should exist").hp,
        tank_hp_before,
        "seeded spread rays miss the nearer vehicle while aiming at infantry"
    );
}

#[test]
fn tank_commander_mg_rotates_and_fires_outside_cannon_arc() {
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

    // Give the independent mount time to traverse; keep the cannon reloading.
    let mut fired = false;
    for _ in 0..20 {
        let events = run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
        );
        fired |= !attack_weapon_kinds(&events, 1, tank).is_empty();
    }
    assert!(fired, "the commander MG must engage outside the cannon arc");
    assert!(entities.get(worker).expect("worker").hp < worker_hp_before);
    assert_eq!(entities.get(tank).expect("tank").weapon_facing(), Some(0.0));
    assert!(
        entities
            .get(tank)
            .expect("tank")
            .combat
            .as_ref()
            .expect("combat")
            .commander_facing
            > 0.2
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
        [
            vec!["tank_cannon".to_string()],
            vec!["tank_coax".to_string(); 3]
        ]
        .concat(),
        "Tank cannon attack feedback must be emitted before the same-tick coax feedback"
    );
    let tank_entity = entities.get(tank).expect("tank should exist");
    assert_eq!(
        tank_entity.weapon_cooldown(combat_rules::WeaponKind::TankCannon),
        144
    );
    assert_eq!(
        tank_entity.weapon_cooldown(combat_rules::WeaponKind::TankCoax),
        12
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
        7
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

#[test]
fn tank_commander_mg_engages_behind_turret_without_redirecting_cannon() {
    for (dx, dy) in [(-120.0, 0.0), (0.0, -120.0), (0.0, 120.0)] {
        let map = open_map(32);
        let mut entities = EntityStore::new();
        let tank = entities
            .spawn_unit(1, EntityKind::Tank, 400.0, 400.0)
            .expect("tank");
        let infantry = entities
            .spawn_unit(2, EntityKind::Rifleman, 400.0 + dx, 400.0 + dy)
            .expect("infantry");
        prepare_coax_tank(&mut entities, tank);
        let hp = entities.get(infantry).expect("infantry").hp;
        for _ in 0..100 {
            run_combat_tick_on_map(
                &mut entities,
                &[player_state(1, false), player_state(2, false)],
                &map,
            );
            if entities.get(infantry).expect("infantry").hp < hp {
                break;
            }
        }
        assert!(
            entities.get(infantry).expect("infantry").hp < hp,
            "MG must traverse to ({dx}, {dy})"
        );
        let tank = entities.get(tank).expect("tank");
        assert_eq!(tank.weapon_facing(), Some(0.0));
        assert_eq!(tank.target_id(), None);
    }
}
