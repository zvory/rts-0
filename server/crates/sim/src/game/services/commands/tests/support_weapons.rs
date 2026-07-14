use super::*;

fn assert_error_tiles(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.001,
        "expected {expected:.3} tiles, got {actual:.3}"
    );
}

#[test]
fn artillery_error_scales_by_range_without_ballistic_tables() {
    let tile = config::TILE_SIZE as f32;
    let origin = (100.0, 100.0);
    let min_target = (
        origin.0 + config::ARTILLERY_MIN_RANGE_TILES as f32 * tile,
        origin.1,
    );
    let max_target = (
        origin.0 + config::ARTILLERY_MAX_RANGE_TILES as f32 * tile,
        origin.1,
    );

    assert_error_tiles(
        artillery_error_tiles(origin, min_target, 1, false),
        config::ARTILLERY_MIN_RANGE_ERROR_TILES,
    );
    assert_error_tiles(
        artillery_error_tiles(origin, max_target, 1, false),
        config::ARTILLERY_MAX_RANGE_ERROR_TILES,
    );
    assert_error_tiles(
        artillery_error_tiles(origin, max_target, 5, false),
        config::ARTILLERY_MAX_RANGE_ERROR_TILES,
    );
}

#[test]
fn ballistic_tables_tighten_range_scaled_artillery_error_to_three_tiles() {
    let tile = config::TILE_SIZE as f32;
    let origin = (100.0, 100.0);
    let max_target = (
        origin.0 + config::ARTILLERY_MAX_RANGE_TILES as f32 * tile,
        origin.1,
    );

    assert_error_tiles(
        artillery_error_tiles(origin, max_target, 1, true),
        config::ARTILLERY_MAX_RANGE_ERROR_TILES,
    );
    assert_error_tiles(artillery_error_tiles(origin, max_target, 3, true), 9.0);
    assert_error_tiles(
        artillery_error_tiles(origin, max_target, 5, true),
        config::ARTILLERY_MIN_ERROR_TILES,
    );
}

#[test]
fn unupgraded_artillery_fire_does_not_bank_ballistic_tables_accuracy() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let mut players = vec![player_state(1), player_state(2)];
    let pos = (320.0, 320.0);
    let target = (
        pos.0 + config::TILE_SIZE as f32 * config::ARTILLERY_MAX_RANGE_TILES as f32,
        pos.1,
    );
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    {
        let unit = entities.get_mut(artillery).expect("artillery should exist");
        unit.set_weapon_setup(WeaponSetup::Deployed);
        unit.set_emplacement_facing(Some(0.0));
        unit.set_weapon_facing(0.0);
    }

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::PointFire,
                units: vec![artillery],
                x: Some(target.0),
                y: Some(target.1),
                queued: false,
            },
        )],
    );

    assert_eq!(
        entities
            .get(artillery)
            .expect("artillery should exist")
            .artillery_shots_fired(),
        0,
        "unupgraded artillery shots should not pre-charge Artillery Fire Control accuracy"
    );
}

#[test]
fn setup_anti_tank_guns_filters_mixed_selection_and_records_facing() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let at = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("at gun should spawn");
    let rifle = entities
        .spawn_unit(1, EntityKind::Rifleman, 120.0, 100.0)
        .expect("rifleman should spawn");
    let enemy_at = entities
        .spawn_unit(2, EntityKind::AntiTankGun, 140.0, 100.0)
        .expect("enemy at gun should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::SetupAntiTankGuns {
                units: vec![at, rifle, enemy_at, at],
                x: 100.0,
                y: 140.0,
                queued: false,
            },
        )],
    );

    let at = entities.get(at).expect("at gun should exist");
    assert_eq!(at.weapon_setup(), WeaponSetup::Packed);
    assert!(
        (at.emplacement_facing().unwrap_or_default() - std::f32::consts::FRAC_PI_2).abs() < 0.001,
        "setup command should store a finite facing toward the target point"
    );
    assert!(
        at.facing().abs() < 0.001,
        "setup command should not snap the anti-tank gun body to the target facing"
    );
    assert_eq!(
        entities
            .get(rifle)
            .expect("rifleman should exist")
            .weapon_setup(),
        WeaponSetup::Packed,
        "non-setup-capable units in the selected list are ignored"
    );
    assert_eq!(
        entities
            .get(enemy_at)
            .expect("enemy at gun should exist")
            .weapon_setup(),
        WeaponSetup::Packed,
        "enemy anti-tank guns are ignored"
    );
}

#[test]
fn queued_setup_anti_tank_guns_filters_to_anti_tank_guns_and_preserves_later_attack_move() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let at = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("at gun should spawn");
    let rifle = entities
        .spawn_unit(1, EntityKind::Rifleman, 120.0, 100.0)
        .expect("rifleman should spawn");

    apply(
        &map,
        &mut entities,
        vec![
            (
                1,
                SimCommand::SetupAntiTankGuns {
                    units: vec![at, rifle],
                    x: 100.0,
                    y: 140.0,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::AttackMove {
                    units: vec![at, rifle],
                    x: 220.0,
                    y: 100.0,
                    queued: true,
                },
            ),
        ],
    );

    assert!(matches!(
        entities.get(at).unwrap().queued_orders()[0],
        OrderIntent::SetupAntiTankGuns(_)
    ));
    assert_eq!(entities.get(at).unwrap().queued_orders().len(), 2);
    assert_eq!(
        entities.get(rifle).unwrap().queued_orders().len(),
        1,
        "non-setup-capable units skip setup but keep later compatible stages"
    );
}

#[test]
fn artillery_point_fire_inside_arc_keeps_setup_facing_fixed() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let mut players = vec![player_state(1), player_state(2)];
    let pos = (320.0, 320.0);
    let angle = config::ARTILLERY_FIELD_OF_FIRE_RAD * 0.45;
    let distance = config::TILE_SIZE as f32 * 30.0;
    let target = (
        pos.0 + angle.cos() * distance,
        pos.1 + angle.sin() * distance,
    );
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    {
        let unit = entities.get_mut(artillery).expect("artillery should exist");
        unit.set_weapon_setup(WeaponSetup::Deployed);
        unit.set_emplacement_facing(Some(0.0));
        unit.set_weapon_facing(0.0);
    }

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::PointFire,
                units: vec![artillery],
                x: Some(target.0),
                y: Some(target.1),
                queued: false,
            },
        )],
    );

    let unit = entities.get(artillery).expect("artillery should exist");
    assert!(matches!(unit.weapon_setup(), WeaponSetup::Deployed));
    assert!(matches!(unit.order(), Order::ArtilleryPointFire(_)));
    assert!(
        unit.emplacement_facing().unwrap_or_default().abs() < 0.001,
        "in-arc point fire must not recenter the deployed field of fire"
    );
    assert_eq!(players[0].steel, 1_000 - config::ARTILLERY_AMMO_COST_STEEL);
    assert!(events.get(&1).is_some_and(|events| events
        .iter()
        .any(|event| matches!(event, Event::ArtilleryTarget { from, .. } if *from == artillery))));
    assert!(events.get(&1).is_some_and(|events| events.iter().any(
        |event| matches!(event, Event::ArtilleryFiring { owner: 1, x, y, .. }
            if (*x - pos.0).abs() < 0.001 && (*y - pos.1).abs() < 0.001)
    )));
    assert!(
        events.get(&2).is_some_and(|events| events.iter().any(
            |event| matches!(event, Event::ArtilleryFiring { owner: 1, x, y, .. }
                if (*x - pos.0).abs() < 0.001 && (*y - pos.1).abs() < 0.001)
        )),
        "all players receive the firing-position minimap marker"
    );
    assert!(
        events.get(&2).map_or(true, |events| events
            .iter()
            .all(|event| !matches!(event, Event::ArtilleryTarget { .. }))),
        "enemy players still do not receive artillery target data"
    );
}

#[test]
fn artillery_point_fire_system_rechecks_ammo_affordability() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let mut players = vec![player_state(1), player_state(2)];
    assert!(players[0].spend_cost(rules::economy::ResourceCost::new(1_000, 0)));
    let pos = (640.0, 640.0);
    let target = (pos.0 + config::TILE_SIZE as f32 * 30.0, pos.1);
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    {
        let unit = entities.get_mut(artillery).expect("artillery should exist");
        unit.set_weapon_setup(WeaponSetup::Deployed);
        unit.set_emplacement_facing(Some(0.0));
        unit.set_weapon_facing(0.0);
        unit.set_order(Order::artillery_point_fire(target.0, target.1));
    }
    let mut artillery_shells = ArtilleryShellStore::default();
    let mut firing_reveals = Vec::new();
    let mut events: HashMap<u32, Vec<Event>> = HashMap::from([(1, Vec::new()), (2, Vec::new())]);
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2], &entities, &map);

    artillery_point_fire_system(
        &map,
        &mut entities,
        &mut players,
        &mut artillery_shells,
        &mut firing_reveals,
        &mut events,
        &fog,
        7,
    );

    assert_eq!(
        players[0].steel, 0,
        "failed artillery fire should not spend unavailable ammo"
    );
    assert_eq!(
        entities
            .get(artillery)
            .expect("artillery should exist")
            .attack_cd(),
        config::ARTILLERY_RELOAD_TICKS,
        "promotion-time ammo failure still applies the current reload penalty"
    );
    assert_notice(&events, 1, "Not enough steel");
    assert!(
        events
            .values()
            .flat_map(|events| events.iter())
            .all(|event| !matches!(event, Event::ArtilleryTarget { .. })),
        "unaffordable point fire should not schedule a visible target marker"
    );
}

#[test]
fn artillery_blanket_fire_system_rechecks_ammo_affordability() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let mut players = vec![player_state(1), player_state(2)];
    assert!(players[0].spend_cost(rules::economy::ResourceCost::new(1_000, 0)));
    let pos = (640.0, 640.0);
    let target = (pos.0 + config::TILE_SIZE as f32 * 30.0, pos.1);
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    {
        let unit = entities.get_mut(artillery).expect("artillery should exist");
        unit.set_weapon_setup(WeaponSetup::Deployed);
        unit.set_emplacement_facing(Some(0.0));
        unit.set_weapon_facing(0.0);
        unit.set_order(Order::artillery_blanket_fire(target.0, target.1));
    }
    let mut artillery_shells = ArtilleryShellStore::default();
    let mut firing_reveals = Vec::new();
    let mut events: HashMap<u32, Vec<Event>> = HashMap::from([(1, Vec::new()), (2, Vec::new())]);
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2], &entities, &map);

    artillery_point_fire_system(
        &map,
        &mut entities,
        &mut players,
        &mut artillery_shells,
        &mut firing_reveals,
        &mut events,
        &fog,
        7,
    );

    assert_eq!(
        players[0].steel, 0,
        "failed blanket fire should not spend unavailable ammo"
    );
    assert_eq!(
        entities
            .get(artillery)
            .expect("artillery should exist")
            .attack_cd(),
        config::ARTILLERY_RELOAD_TICKS,
        "promotion-time Blanket Fire ammo failure still applies the current reload penalty"
    );
    assert_notice(&events, 1, "Not enough steel");
    assert!(
        events
            .values()
            .flat_map(|events| events.iter())
            .all(|event| !matches!(event, Event::ArtilleryTarget { .. })),
        "unaffordable Blanket Fire should not schedule a visible target marker"
    );
}

#[test]
fn artillery_point_fire_outside_arc_replaces_active_fire_with_redeploy() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let mut players = vec![player_state(1), player_state(2)];
    let pos = (320.0, 320.0);
    let old_target = (pos.0 + config::TILE_SIZE as f32 * 30.0, pos.1);
    let angle = config::ARTILLERY_FIELD_OF_FIRE_RAD;
    let distance = config::TILE_SIZE as f32 * 30.0;
    let target = (
        pos.0 + angle.cos() * distance,
        pos.1 + angle.sin() * distance,
    );
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    {
        let unit = entities.get_mut(artillery).expect("artillery should exist");
        unit.set_weapon_setup(WeaponSetup::Deployed);
        unit.set_emplacement_facing(Some(0.0));
        unit.set_weapon_facing(0.0);
        unit.set_attack_cd(config::ARTILLERY_RELOAD_TICKS);
        unit.set_order(Order::artillery_point_fire(old_target.0, old_target.1));
    }

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::PointFire,
                units: vec![artillery],
                x: Some(target.0),
                y: Some(target.1),
                queued: false,
            },
        )],
    );

    let unit = entities.get(artillery).expect("artillery should exist");
    assert!(matches!(
        unit.weapon_setup(),
        WeaponSetup::TearingDownToRedeploy { .. }
    ));
    assert!(matches!(unit.order(), Order::ArtilleryPointFire(_)));
    assert!(
        (unit.pending_redeploy_facing().unwrap_or_default() - angle).abs() < 0.001,
        "outside-arc point fire should store the requested redeploy facing"
    );
    assert_eq!(players[0].steel, 1_000);
    assert!(events
        .values()
        .flat_map(|events| events.iter())
        .all(|event| !matches!(event, Event::ArtilleryTarget { .. })));
    let Order::ArtilleryPointFire(order) = unit.order() else {
        panic!("retarget should keep an artillery point-fire order");
    };
    assert!(
        (order.intent.x - target.0).abs() < 0.001,
        "expected retarget x {}, got {}",
        target.0,
        order.intent.x
    );
    assert!((order.intent.y - target.1).abs() < 0.001);
}

#[test]
fn artillery_point_fire_can_retarget_while_redeploying() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let mut players = vec![player_state(1), player_state(2)];
    let pos = (640.0, 640.0);
    let old_angle = config::ARTILLERY_FIELD_OF_FIRE_RAD;
    let new_angle = -config::ARTILLERY_FIELD_OF_FIRE_RAD;
    let distance = config::TILE_SIZE as f32 * 30.0;
    let old_target = (
        pos.0 + old_angle.cos() * distance,
        pos.1 + old_angle.sin() * distance,
    );
    let target = (
        pos.0 + new_angle.cos() * distance,
        pos.1 + new_angle.sin() * distance,
    );
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    {
        let unit = entities.get_mut(artillery).expect("artillery should exist");
        unit.set_weapon_setup(WeaponSetup::TearingDownToRedeploy {
            ticks: config::ARTILLERY_SETUP_TICKS,
        });
        unit.set_emplacement_facing(Some(0.0));
        unit.set_pending_redeploy_facing(Some(old_angle));
        unit.set_weapon_facing(0.0);
        unit.set_order(Order::artillery_point_fire(old_target.0, old_target.1));
    }

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::PointFire,
                units: vec![artillery],
                x: Some(target.0),
                y: Some(target.1),
                queued: false,
            },
        )],
    );

    let unit = entities.get(artillery).expect("artillery should exist");
    assert!(matches!(
        unit.weapon_setup(),
        WeaponSetup::TearingDownToRedeploy { .. }
    ));
    let Order::ArtilleryPointFire(order) = unit.order() else {
        panic!("retarget should keep an artillery point-fire order");
    };
    assert!(
        (order.intent.x - target.0).abs() < 0.001,
        "expected retarget x {}, got {}",
        target.0,
        order.intent.x
    );
    assert!((order.intent.y - target.1).abs() < 0.001);
    assert!(
        (unit.pending_redeploy_facing().unwrap_or_default() - new_angle).abs() < 0.001,
        "retargeting during redeploy should update the pending facing"
    );
}

#[test]
fn teardown_anti_tank_guns_only_affects_setting_up_or_deployed_anti_tank_guns() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let deployed = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("at gun should spawn");
    let packed = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 130.0, 100.0)
        .expect("at gun should spawn");
    entities
        .get_mut(deployed)
        .unwrap()
        .set_weapon_setup(WeaponSetup::Deployed);
    entities
        .get_mut(packed)
        .unwrap()
        .set_emplacement_facing(Some(1.0));

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::TearDownAntiTankGuns {
                units: vec![deployed, packed],
            },
        )],
    );

    assert!(matches!(
        entities.get(deployed).unwrap().weapon_setup(),
        WeaponSetup::TearingDown { .. }
    ));
    assert_eq!(
        entities.get(packed).unwrap().weapon_setup(),
        WeaponSetup::Packed
    );
    assert_eq!(
        entities.get(packed).unwrap().emplacement_facing(),
        None,
        "teardown should cancel a packed anti-tank gun's staged setup facing"
    );
}

#[test]
fn move_order_tears_down_deployed_anti_tank_guns_before_moving() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let deployed = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("at gun should spawn");
    let packed = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 130.0, 100.0)
        .expect("at gun should spawn");
    {
        let at = entities.get_mut(deployed).unwrap();
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.25));
        at.set_facing(0.25);
        at.set_weapon_facing(0.25);
    }

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: vec![deployed, packed],
                x: 220.0,
                y: 100.0,
                queued: false,
            },
        )],
    );

    let deployed = entities.get(deployed).expect("at gun should exist");
    assert!(matches!(
        deployed.weapon_setup(),
        WeaponSetup::TearingDown { .. }
    ));
    assert_eq!(
        deployed.facing(),
        0.25,
        "move order should not instantly rotate a deployed anti-tank gun before it moves"
    );
    assert!(
        matches!(deployed.order(), Order::Move(_)),
        "move should replace the deployed anti-tank gun order"
    );
    assert!(
        deployed.path_goal().is_some(),
        "move should preserve the movement destination while the anti-tank gun tears down"
    );
    assert_eq!(deployed.emplacement_facing(), None);
    assert_eq!(deployed.pending_redeploy_facing(), None);

    let packed = entities.get(packed).expect("packed at gun should exist");
    assert!(
        matches!(packed.order(), Order::Move(_)),
        "packed anti-tank guns should still accept move orders"
    );
}

#[test]
fn replacement_move_preserves_support_weapon_teardown_progress() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let support_weapons = [
        (EntityKind::MachineGunner, 100.0),
        (EntityKind::AntiTankGun, 120.0),
        (EntityKind::Artillery, 140.0),
    ]
    .map(|(kind, y)| {
        let id = entities
            .spawn_unit(1, kind, 100.0, y)
            .expect("support weapon should spawn");
        entities
            .get_mut(id)
            .expect("support weapon should exist")
            .set_weapon_setup(WeaponSetup::Deployed);
        id
    });

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: support_weapons.to_vec(),
                x: 220.0,
                y: 100.0,
                queued: false,
            },
        )],
    );
    for _ in 0..5 {
        for id in support_weapons {
            entities
                .get_mut(id)
                .expect("support weapon should exist")
                .tick_weapon_setup();
        }
    }
    let teardown_states = support_weapons.map(|id| {
        let weapon = entities.get(id).expect("support weapon should exist");
        (weapon.weapon_setup(), weapon.path_goal())
    });

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: support_weapons.to_vec(),
                x: 260.0,
                y: 120.0,
                queued: false,
            },
        )],
    );

    for (id, (teardown_state, first_goal)) in support_weapons.into_iter().zip(teardown_states) {
        let weapon = entities.get(id).expect("support weapon should exist");
        assert!(matches!(teardown_state, WeaponSetup::TearingDown { .. }));
        assert_eq!(
            weapon.weapon_setup(),
            teardown_state,
            "replacement movement should not restart an in-progress teardown for {:?}",
            weapon.kind
        );
        assert!(
            matches!(weapon.order(), Order::Move(_)),
            "replacement movement should still update the active destination"
        );
        assert_ne!(
            weapon.path_goal(),
            first_goal,
            "replacement movement should still update the path goal"
        );
    }
}

#[test]
fn move_preserves_redeploy_teardown_progress_and_cancels_redeploy() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    entities
        .get_mut(anti_tank_gun)
        .expect("anti-tank gun should exist")
        .set_weapon_setup(WeaponSetup::Deployed);

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::SetupAntiTankGuns {
                units: vec![anti_tank_gun],
                x: 100.0,
                y: 220.0,
                queued: false,
            },
        )],
    );
    for _ in 0..5 {
        entities
            .get_mut(anti_tank_gun)
            .expect("anti-tank gun should exist")
            .tick_weapon_setup();
    }
    let remaining_ticks = match entities
        .get(anti_tank_gun)
        .expect("anti-tank gun should exist")
        .weapon_setup()
    {
        WeaponSetup::TearingDownToRedeploy { ticks } => ticks,
        setup => panic!("expected redeploy teardown, got {setup:?}"),
    };

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: vec![anti_tank_gun],
                x: 220.0,
                y: 100.0,
                queued: false,
            },
        )],
    );

    let anti_tank_gun = entities
        .get(anti_tank_gun)
        .expect("anti-tank gun should exist");
    assert_eq!(
        anti_tank_gun.weapon_setup(),
        WeaponSetup::TearingDown {
            ticks: remaining_ticks
        }
    );
    assert_eq!(anti_tank_gun.pending_redeploy_facing(), None);
}

#[test]
fn attack_move_order_tears_down_deployed_anti_tank_guns_before_moving() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let deployed = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("at gun should spawn");
    {
        let at = entities.get_mut(deployed).unwrap();
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(-0.5));
        at.set_facing(-0.5);
        at.set_weapon_facing(-0.5);
    }

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::AttackMove {
                units: vec![deployed],
                x: 220.0,
                y: 100.0,
                queued: false,
            },
        )],
    );

    let deployed = entities.get(deployed).expect("at gun should exist");
    assert!(matches!(
        deployed.weapon_setup(),
        WeaponSetup::TearingDown { .. }
    ));
    assert_eq!(
        deployed.facing(),
        -0.5,
        "attack-move should not instantly rotate a deployed anti-tank gun before it moves"
    );
    assert!(
        matches!(deployed.order(), Order::AttackMove(_)),
        "attack-move should replace the deployed anti-tank gun order"
    );
    assert!(
        deployed.path_goal().is_some(),
        "attack-move should preserve the movement destination while the anti-tank gun tears down"
    );
    assert_eq!(deployed.emplacement_facing(), None);
    assert_eq!(deployed.pending_redeploy_facing(), None);
}

#[test]
fn deployed_anti_tank_gun_rejects_explicit_attack_outside_field_of_fire() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let at = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("at gun should spawn");
    let front_target = entities
        .spawn_unit(2, EntityKind::Tank, 220.0, 100.0)
        .expect("target should spawn");
    let side_target = entities
        .spawn_unit(2, EntityKind::Tank, 100.0, 220.0)
        .expect("target should spawn");
    {
        let at = entities.get_mut(at).unwrap();
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
    }

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Attack {
                units: vec![at],
                target: side_target,
                queued: false,
            },
        )],
    );
    assert!(
        !matches!(entities.get(at).unwrap().order(), Order::Attack(_)),
        "out-of-arc attack should be ignored for the deployed anti-tank gun"
    );

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Attack {
                units: vec![at],
                target: front_target,
                queued: false,
            },
        )],
    );
    assert!(
        matches!(entities.get(at).unwrap().order(), Order::Attack(_)),
        "in-arc attack should still be accepted"
    );
}
