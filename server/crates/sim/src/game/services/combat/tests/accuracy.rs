use super::*;

#[allow(clippy::too_many_arguments)]
fn apply_test_damage_with_seed(
    entities: &mut EntityStore,
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
    let weapon_profile = entities
        .get(attacker)
        .and_then(|entity| combat_rules::default_weapon_profile(entity.kind))
        .expect("attacker should have a default weapon profile");
    let map = Map::generate(2, 0x00C0_FFEE);
    let fog = Fog::new(map.size);
    let smokes = SmokeCloudStore::new();
    let mut rng = SmallRng::seed_from_u64(rng_seed);
    let blockers = ShotBlockerIndex::build(&map, entities);
    apply_damage(
        &map,
        entities,
        &blockers,
        &default_team_relations(),
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
fn tank_cannon_seeded_shot_can_miss_infantry_without_missing_scout_cars() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("attacker should spawn");
    let infantry = entities
        .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
        .expect("infantry should spawn");
    let scout_car = entities
        .spawn_unit(2, EntityKind::ScoutCar, 140.0, 140.0)
        .expect("scout car should spawn");
    let infantry_hp = entities.get(infantry).expect("infantry should exist").hp;
    let scout_car_hp = entities.get(scout_car).expect("scout car should exist").hp;
    let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

    apply_test_damage_with_seed(
        &mut entities,
        &mut events,
        attacker,
        infantry,
        60,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        160.0,
        0,
    );
    apply_test_damage_with_seed(
        &mut entities,
        &mut events,
        attacker,
        scout_car,
        60,
        1,
        100.0,
        100.0,
        140.0,
        140.0,
        160.0,
        0,
    );

    assert_eq!(
        entities.get(infantry).expect("infantry should exist").hp,
        infantry_hp,
        "seeded tank shell should miss an infantry-sized target"
    );
    assert_eq!(
        entities.get(scout_car).expect("scout car should exist").hp,
        scout_car_hp.saturating_sub(60),
        "scout cars are vehicles and must not receive an infantry dodge roll"
    );
}

#[test]
fn tank_and_at_gun_primary_dodge_do_not_cancel_secondary_overpenetration_roll() {
    for attacker_kind in [EntityKind::Tank, EntityKind::AntiTankGun] {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, attacker_kind, 100.0, 100.0)
            .expect("attacker should spawn");
        let primary = entities
            .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
            .expect("primary should spawn");
        let secondary = entities
            .spawn_unit(2, EntityKind::Worker, 165.0, 100.0)
            .expect("secondary should spawn");
        let primary_hp = entities.get(primary).expect("primary should exist").hp;
        let secondary_hp = entities.get(secondary).expect("secondary should exist").hp;
        let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

        apply_test_damage_with_seed(
            &mut entities,
            &mut events,
            attacker,
            primary,
            100,
            1,
            100.0,
            100.0,
            140.0,
            100.0,
            160.0,
            3,
        );

        assert_eq!(
            entities.get(primary).expect("primary should exist").hp,
            primary_hp,
            "seed 3 should make the primary infantry target dodge {attacker_kind:?}"
        );
        assert!(
            entities.get(secondary).expect("secondary should exist").hp < secondary_hp,
            "the primary dodge must not cancel {attacker_kind:?}'s independent secondary roll"
        );
        assert!(
            events
                .get(&1)
                .expect("attacker events should exist")
                .iter()
                .any(|event| matches!(event, Event::Overpenetration { to } if *to == secondary)),
            "the independently hit secondary should emit overpenetration feedback for {attacker_kind:?}"
        );
    }
}
