use super::*;

fn mark_entrenched(entities: &mut EntityStore, id: u32) {
    entities
        .get_mut(id)
        .expect("entity should exist")
        .movement
        .as_mut()
        .expect("entity should have movement")
        .occupied_trench_id = Some(1);
}

#[allow(clippy::too_many_arguments)]
fn apply_test_damage_with_seed_and_teams(
    entities: &mut EntityStore,
    teams: &TeamRelations,
    events: &mut HashMap<u32, Vec<Event>>,
    attacker: u32,
    victim: u32,
    dmg: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
    range_px: f32,
    rng_seed: u64,
) {
    let map = Map::generate(2, 0x00C0_FFEE);
    let fog = Fog::new(map.size);
    let smokes = SmokeCloudStore::new();
    let mut rng = SmallRng::seed_from_u64(rng_seed);
    let weapon_profile = entities
        .get(attacker)
        .and_then(|entity| combat_rules::default_weapon_profile(entity.kind))
        .expect("test attacker should have a default weapon profile");
    let blockers = ShotBlockerIndex::build(&map, entities);
    apply_damage(
        &map,
        entities,
        &blockers,
        teams,
        events,
        &fog,
        &smokes,
        &mut rng,
        attacker,
        victim,
        weapon_profile,
        dmg,
        attacker_owner,
        ax,
        ay,
        vx,
        vy,
        range_px,
        0.0,
        10,
    );
}

#[test]
fn entrenched_eligible_infantry_gain_one_tile_of_weapon_range() {
    for kind in [EntityKind::Rifleman, EntityKind::MachineGunner] {
        let mut entities = EntityStore::new();
        let id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("eligible infantry should spawn");
        let base_range = combat_rules::attack_profile(kind).range_tiles as f32;

        assert_eq!(
            effective_attack_profile(entities.get(id).expect("unit should exist")).range_tiles,
            base_range,
            "{kind:?} should keep base range outside a trench"
        );

        mark_entrenched(&mut entities, id);
        assert_eq!(
            effective_attack_profile(entities.get(id).expect("unit should exist")).range_tiles,
            base_range + config::ENTRENCHMENT_RANGE_BONUS_TILES as f32,
            "{kind:?} should gain the entrenchment range bonus"
        );
    }
}

#[test]
fn excluded_units_do_not_gain_range_from_stale_trench_state() {
    for kind in [
        EntityKind::Worker,
        EntityKind::MortarTeam,
        EntityKind::Golem,
        EntityKind::Tank,
    ] {
        let mut entities = EntityStore::new();
        let id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("unit should spawn");
        let base_range =
            effective_attack_profile(entities.get(id).expect("unit should exist")).range_tiles;
        mark_entrenched(&mut entities, id);

        assert_eq!(
            effective_attack_profile(entities.get(id).expect("unit should exist")).range_tiles,
            base_range,
            "{kind:?} should not receive entrenchment combat benefits"
        );
    }
}

#[test]
fn entrenched_idle_rifleman_fires_at_bonus_range_without_chasing() {
    let map = open_map(20);
    let mut entities = EntityStore::new();
    let rifleman = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    mark_entrenched(&mut entities, rifleman);
    let enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 258.0, 100.0)
        .expect("enemy should spawn");
    let enemy_hp = entities.get(enemy).expect("enemy should exist").hp;

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let rifleman = entities.get(rifleman).expect("rifleman should exist");
    assert_eq!(rifleman.target_id(), Some(enemy));
    assert_eq!(rifleman.path_goal(), None);
    assert!(rifleman.path_is_empty());
    assert!(
        entities.get(enemy).expect("enemy should exist").hp < enemy_hp,
        "entrenched rifleman should fire at a target only inside bonus range"
    );
}

#[test]
fn entrenched_idle_rifleman_does_not_chase_enemies_outside_weapon_range() {
    let map = open_map(20);
    let mut entities = EntityStore::new();
    let rifleman = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    mark_entrenched(&mut entities, rifleman);
    let enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 320.0, 100.0)
        .expect("enemy should spawn");
    let enemy_hp = entities.get(enemy).expect("enemy should exist").hp;

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let rifleman = entities.get(rifleman).expect("rifleman should exist");
    assert_eq!(rifleman.target_id(), None);
    assert_eq!(rifleman.path_goal(), None);
    assert!(rifleman.path_is_empty());
    assert_eq!(
        entities.get(enemy).expect("enemy should exist").hp,
        enemy_hp,
        "entrenched idle acquisition should not attack or chase out-of-range targets"
    );
}

#[test]
fn entrenched_arrived_attack_move_units_do_not_leave_to_chase() {
    let map = open_map(20);
    for kind in [EntityKind::Rifleman, EntityKind::MachineGunner] {
        let mut entities = EntityStore::new();
        let unit_id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("eligible infantry should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 360.0, 100.0)
            .expect("enemy should spawn");
        let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;
        {
            let unit = entities.get_mut(unit_id).expect("unit should exist");
            unit.set_order(Order::attack_move_to(100.0, 100.0));
            unit.mark_move_phase(MovePhase::Arrived);
            if kind == EntityKind::MachineGunner {
                unit.set_weapon_setup(WeaponSetup::Deployed);
            }
        }
        mark_entrenched(&mut entities, unit_id);

        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
        );

        let unit = entities.get(unit_id).expect("unit should exist");
        assert!(matches!(unit.order(), Order::AttackMove(_)));
        assert_eq!(unit.move_phase(), Some(MovePhase::Arrived));
        assert_eq!(
            unit.target_id(),
            None,
            "{kind:?} should not acquire a chase target"
        );
        assert_eq!(
            unit.path_goal(),
            None,
            "{kind:?} should not request a chase path"
        );
        assert!(unit.path_is_empty(), "{kind:?} should remain in the trench");
        if kind == EntityKind::MachineGunner {
            assert_eq!(unit.weapon_setup(), WeaponSetup::Deployed);
        }
        assert_eq!(
            entities.get(enemy_id).expect("enemy should exist").hp,
            enemy_hp,
            "{kind:?} should not fire beyond entrenched weapon range"
        );
    }
}

#[test]
fn non_entrenched_idle_rifleman_also_ignores_out_of_range_enemy() {
    let map = open_map(20);
    let mut entities = EntityStore::new();
    let rifleman = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 300.0, 100.0)
        .expect("enemy should spawn");

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let rifleman = entities.get(rifleman).expect("rifleman should exist");
    assert_eq!(rifleman.target_id(), None);
    assert_eq!(rifleman.path_goal(), None);
    assert!(entities.get(enemy).is_some());
}

#[test]
fn entrenched_meth_rifleman_keeps_faster_attack_cooldown() {
    let mut entities = EntityStore::new();
    let rifleman_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    mark_entrenched(&mut entities, rifleman_id);
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 130.0, 100.0)
        .expect("enemy should spawn");
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;
    let mut meth_player = player_state(1, false);
    meth_player.upgrades.insert(UpgradeKind::Methamphetamines);

    run_combat_tick_with_players(&mut entities, &[meth_player, player_state(2, false)]);

    let rifleman = entities.get(rifleman_id).expect("rifleman should exist");
    assert_eq!(
        rifleman.attack_cd(),
        config::unit_stats(EntityKind::Rifleman)
            .expect("rifleman stats")
            .cooldown
            * config::METHAMPHETAMINES_ATTACK_COOLDOWN_NUMERATOR
            / config::METHAMPHETAMINES_ATTACK_COOLDOWN_DENOMINATOR,
        "Methamphetamines attack cadence should still apply while entrenched"
    );
    assert!(
        entities.get(enemy_id).expect("enemy should exist").hp < enemy_hp,
        "entrenched meth rifleman should still fire"
    );
}

#[test]
fn entrenched_direct_shot_halves_damage_without_emitting_a_miss() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let victim = entities
        .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
        .expect("victim should spawn");
    mark_entrenched(&mut entities, victim);
    let victim_hp = entities.get(victim).expect("victim should exist").hp;
    let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
    events.insert(1, Vec::new());
    events.insert(2, Vec::new());

    apply_test_damage(
        &mut entities,
        &mut events,
        attacker,
        victim,
        10,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert_eq!(
        entities.get(victim).expect("victim should exist").hp,
        victim_hp - 5,
        "actively entrenched infantry should take half direct damage"
    );
    assert!(
        events
            .get(&1)
            .expect("attacker owner events should exist")
            .iter()
            .any(|event| matches!(event, Event::Attack { from, to, .. } if *from == attacker && *to == victim)),
        "reduced-damage entrenched shots should still emit attack feedback"
    );
    assert!(
        events
            .get(&1)
            .expect("attacker owner events should exist")
            .iter()
            .all(|event| !matches!(event, Event::Miss { to } if *to == victim)),
        "entrenchment should not turn direct shots into miss feedback"
    );
}

#[test]
fn direct_shots_against_buildings_ignore_entrenchment_damage_reduction() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let depot = entities
        .spawn_building(2, EntityKind::Depot, 140.0, 100.0, true)
        .expect("depot should spawn");
    let depot_hp = entities.get(depot).expect("depot should exist").hp;
    let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
    events.insert(1, Vec::new());
    events.insert(2, Vec::new());

    apply_test_damage(
        &mut entities,
        &mut events,
        attacker,
        depot,
        10,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert!(
        entities.get(depot).expect("depot should exist").hp < depot_hp,
        "entrenchment damage reduction should not affect buildings"
    );
}

#[test]
fn entrenched_primary_victim_stops_overpenetration_after_a_hit() {
    for seed in 0..128 {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("attacker should spawn");
        let primary = entities
            .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
            .expect("primary target should spawn");
        mark_entrenched(&mut entities, primary);
        let secondary = entities
            .spawn_unit(2, EntityKind::Worker, 165.0, 100.0)
            .expect("secondary target should spawn");
        let primary_hp = entities.get(primary).expect("primary should exist").hp;
        let secondary_hp = entities.get(secondary).expect("secondary should exist").hp;
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        events.insert(1, Vec::new());
        events.insert(2, Vec::new());

        apply_test_damage_with_seed_and_teams(
            &mut entities,
            &default_team_relations(),
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
            seed,
        );

        if entities.get(primary).expect("primary should exist").hp == primary_hp {
            continue;
        }
        assert_eq!(
            entities.get(secondary).expect("secondary should exist").hp,
            secondary_hp,
            "entrenched primary victims should stop overpenetration"
        );
        assert!(
            events
                .get(&1)
                .expect("attacker owner events should exist")
                .iter()
                .all(|event| !matches!(event, Event::Overpenetration { to } if *to == secondary)),
            "entrenched primary victims should not emit secondary overpenetration feedback"
        );
        return;
    }
    panic!("expected an entrenched primary victim to take deterministic direct damage");
}

#[test]
fn entrenched_secondary_candidate_skips_overpenetration_damage() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let primary = entities
        .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
        .expect("primary target should spawn");
    let secondary = entities
        .spawn_unit(2, EntityKind::Rifleman, 165.0, 100.0)
        .expect("secondary target should spawn");
    mark_entrenched(&mut entities, secondary);
    let secondary_hp = entities.get(secondary).expect("secondary should exist").hp;
    let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
    events.insert(1, Vec::new());
    events.insert(2, Vec::new());

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
        entities.get(secondary).expect("secondary should exist").hp,
        secondary_hp,
        "entrenched secondary candidates should not take overpenetration damage"
    );
    assert!(
        events
            .get(&1)
            .expect("attacker owner events should exist")
            .iter()
            .all(|event| !matches!(event, Event::Overpenetration { to } if *to == secondary)),
        "skipped entrenched secondary candidates should not emit overpenetration feedback"
    );
}
