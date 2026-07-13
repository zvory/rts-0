use super::*;

#[allow(clippy::too_many_arguments)]
fn apply_test_damage_with_weapon(
    entities: &mut EntityStore,
    teams: &TeamRelations,
    events: &mut HashMap<u32, Vec<Event>>,
    attacker: u32,
    victim: u32,
    weapon_profile: &combat_rules::WeaponProfile,
    dmg: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
    range_px: f32,
) {
    let map = Map::generate(2, 0x00C0_FFEE);
    let fog = Fog::new(map.size);
    let smokes = SmokeCloudStore::new();
    let mut rng = SmallRng::seed_from_u64(0);
    apply_damage(
        &map,
        entities,
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
fn tank_default_cannon_direct_hit_uses_ap_damage() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank attacker should spawn");
    let victim = entities
        .spawn_unit(2, EntityKind::Tank, 140.0, 100.0)
        .expect("victim tank should spawn");
    entities
        .get_mut(victim)
        .expect("victim tank should exist")
        .set_facing(std::f32::consts::PI);
    let before = entities.get(victim).expect("victim tank should exist").hp;
    let mut events: HashMap<u32, Vec<Event>> = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

    apply_test_damage(
        &mut entities,
        &mut events,
        attacker,
        victim,
        60,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert_eq!(
        before - entities.get(victim).expect("victim tank should exist").hp,
        60,
        "Tank default cannon damage should remain AP against armored targets"
    );
    let threat = entities
        .get(victim)
        .and_then(|victim| victim.combat.as_ref())
        .and_then(|combat| combat.incoming_direct_ap_threats.get(&attacker))
        .expect("successful enemy Tank cannon hit should record a direct AP threat");
    assert_eq!((threat.source_x, threat.source_y), (100.0, 100.0));
    assert_eq!(threat.damage_weight, 60);
}

#[test]
fn machine_gunner_default_hit_remains_small_arms_against_armor() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
        .expect("machine gunner should spawn");
    let victim = entities
        .spawn_unit(2, EntityKind::Tank, 140.0, 100.0)
        .expect("victim tank should spawn");
    entities
        .get_mut(victim)
        .expect("victim tank should exist")
        .set_facing(std::f32::consts::PI);
    let before = entities.get(victim).expect("victim tank should exist").hp;
    let mut events: HashMap<u32, Vec<Event>> = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

    apply_test_damage(
        &mut entities,
        &mut events,
        attacker,
        victim,
        40,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert_eq!(
        before - entities.get(victim).expect("victim tank should exist").hp,
        10,
        "Machine Gunner default damage should stay armor-reduced small arms"
    );
    assert!(
        entities
            .get(victim)
            .and_then(|victim| victim.combat.as_ref())
            .is_some_and(|combat| combat.incoming_direct_ap_threats.is_empty()),
        "non-AP direct fire must not trigger a tank hull response"
    );
}

#[test]
fn allied_tank_cannon_damage_does_not_record_armor_reaction_threat() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("attacker tank should spawn");
    let victim = entities
        .spawn_unit(2, EntityKind::Tank, 140.0, 100.0)
        .expect("victim tank should spawn");
    let teams = team_relations(&[(1, 7), (2, 7)]);
    let mut events: HashMap<u32, Vec<Event>> = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

    apply_test_damage_with_teams(
        &mut entities,
        &teams,
        &mut events,
        attacker,
        victim,
        60,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert!(
        entities
            .get(victim)
            .and_then(|victim| victim.combat.as_ref())
            .is_some_and(|combat| combat.incoming_direct_ap_threats.is_empty()),
        "allied AP damage must not steer the victim tank"
    );
}

#[test]
fn direct_damage_uses_weapon_profile_instead_of_attacker_kind() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank attacker should spawn");
    let victim = entities
        .spawn_unit(2, EntityKind::Tank, 140.0, 100.0)
        .expect("victim tank should spawn");
    entities
        .get_mut(victim)
        .expect("victim tank should exist")
        .set_facing(std::f32::consts::PI);
    let before = entities.get(victim).expect("victim tank should exist").hp;
    let weapon = combat_rules::weapon_profile(combat_rules::WeaponKind::MachineGunnerMg)
        .expect("machine-gun profile should exist");
    let mut events: HashMap<u32, Vec<Event>> = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

    apply_test_damage_with_weapon(
        &mut entities,
        &default_team_relations(),
        &mut events,
        attacker,
        victim,
        weapon,
        40,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert_eq!(
        before - entities.get(victim).expect("victim tank should exist").hp,
        10,
        "a Tank firing a small-arms weapon profile must not inherit Tank cannon AP"
    );
}

#[test]
fn overpenetration_uses_weapon_profile_for_secondary_damage() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank attacker should spawn");
    let primary = entities
        .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
        .expect("primary target should spawn");
    let secondary = entities
        .spawn_unit(2, EntityKind::Tank, 190.0, 100.0)
        .expect("secondary tank should spawn");
    entities
        .get_mut(secondary)
        .expect("secondary tank should exist")
        .set_facing(std::f32::consts::PI);
    let secondary_before = entities
        .get(secondary)
        .expect("secondary tank should exist")
        .hp;
    let weapon = combat_rules::weapon_profile(combat_rules::WeaponKind::MachineGunnerMg)
        .expect("machine-gun profile should exist");
    let mut events: HashMap<u32, Vec<Event>> = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

    apply_test_damage_with_weapon(
        &mut entities,
        &default_team_relations(),
        &mut events,
        attacker,
        primary,
        weapon,
        20,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert_eq!(
        secondary_before
            - entities
                .get(secondary)
                .expect("secondary tank should exist")
                .hp,
        2,
        "secondary overpenetration damage should stay small-arms when the weapon profile is small-arms"
    );
}

#[test]
fn overpenetration_depth_comes_from_weapon_profile() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman attacker should spawn");
    let primary = entities
        .spawn_unit(2, EntityKind::ScoutCar, 140.0, 100.0)
        .expect("primary scout car should spawn");
    let secondary = entities
        .spawn_unit(2, EntityKind::Worker, 195.0, 100.0)
        .expect("secondary worker should spawn");
    let secondary_before = entities
        .get(secondary)
        .expect("secondary worker should exist")
        .hp;
    let weapon = combat_rules::weapon_profile(combat_rules::WeaponKind::AntiTankGun)
        .expect("anti-tank profile should exist");
    let mut events: HashMap<u32, Vec<Event>> = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

    apply_test_damage_with_weapon(
        &mut entities,
        &default_team_relations(),
        &mut events,
        attacker,
        primary,
        weapon,
        20,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert!(
        entities
            .get(secondary)
            .expect("secondary worker should exist")
            .hp
            < secondary_before,
        "anti-tank weapon profile should keep its longer overpenetration depth independent of attacker kind"
    );
}
