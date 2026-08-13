use super::*;

#[test]
fn production_repeat_adjustment_protocol_round_trip() {
    let command = SimCommand::AdjustProductionRepeat {
        buildings: vec![3, 5, 7],
        unit: EntityKind::ScoutCar,
        delta: -1,
    };
    let protocol = command
        .to_protocol()
        .expect("production repeat adjustment should encode");
    let encoded = serde_json::to_string(&protocol).expect("command should serialize");
    assert_eq!(
        encoded,
        r#"{"c":"adjustProductionRepeat","buildings":[3,5,7],"unit":"scout_car","delta":-1}"#
    );
    assert_eq!(SimCommand::from_protocol(protocol), command);
}

#[test]
fn unfinished_producer_remembers_repeat_production_for_after_completion() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (x, y) = footprint_center(&map, EntityKind::Barracks, 6, 6);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, x, y, false)
        .expect("unfinished barracks should spawn");
    let mut players = vec![player_state(1), player_state(2)];

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::AdjustProductionRepeat {
                buildings: vec![barracks],
                unit: EntityKind::Rifleman,
                delta: 1,
            },
        )],
    );

    let barracks = entities.get(barracks).expect("unfinished barracks");
    assert!(barracks.under_construction());
    assert_eq!(barracks.repeat_production(), Some(EntityKind::Rifleman));
}

#[test]
fn auto_build_settings_protocol_round_trip() {
    let command = SimCommand::SetAutoBuildSettings {
        paused: true,
        reserve_steel: 250,
        reserve_oil: 150,
    };
    let protocol = command
        .to_protocol()
        .expect("Auto-Build settings should encode");
    let encoded = serde_json::to_string(&protocol).expect("command should serialize");
    assert_eq!(
        encoded,
        r#"{"c":"setAutoBuildSettings","paused":true,"reserveSteel":250,"reserveOil":150}"#
    );
    assert_eq!(SimCommand::from_protocol(protocol), command);
}

#[test]
fn auto_build_settings_are_player_scoped_and_clamped() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let mut players = vec![player_state(1), player_state(2)];

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::SetAutoBuildSettings {
                paused: true,
                reserve_steel: u32::MAX,
                reserve_oil: 250,
            },
        )],
    );

    assert_eq!(
        players[0].auto_build,
        crate::game::AutoBuildSettings {
            paused: true,
            reserve_steel: 9_950,
            reserve_oil: 250,
        }
    );
    assert_eq!(players[1].auto_build, Default::default());
}

#[test]
fn repeat_adjustments_spread_across_least_loaded_producers_with_stable_id_ties() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let mut barracks = Vec::new();
    for tile_x in [4, 10, 16] {
        let (x, y) = footprint_center(&map, EntityKind::Barracks, tile_x, 6);
        barracks.push(
            entities
                .spawn_building(1, EntityKind::Barracks, x, y, true)
                .expect("barracks should spawn"),
        );
    }
    let mut players = vec![player_state(1), player_state(2)];
    let reversed_barracks = barracks.iter().rev().copied().collect::<Vec<_>>();

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![
            (
                1,
                SimCommand::AdjustProductionRepeat {
                    buildings: reversed_barracks.clone(),
                    unit: EntityKind::Rifleman,
                    delta: 1,
                },
            ),
            (
                1,
                SimCommand::AdjustProductionRepeat {
                    buildings: reversed_barracks.clone(),
                    unit: EntityKind::Rifleman,
                    delta: 1,
                },
            ),
            (
                1,
                SimCommand::AdjustProductionRepeat {
                    buildings: reversed_barracks,
                    unit: EntityKind::MachineGunner,
                    delta: 1,
                },
            ),
        ],
    );

    let repeat_units = |building| {
        entities
            .get(building)
            .expect("barracks")
            .production
            .as_ref()
            .expect("production")
            .repeat_units
            .clone()
    };
    assert_eq!(repeat_units(barracks[0]), vec![EntityKind::Rifleman]);
    assert_eq!(repeat_units(barracks[1]), vec![EntityKind::Rifleman]);
    assert_eq!(repeat_units(barracks[2]), vec![EntityKind::MachineGunner]);
    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::AdjustProductionRepeat {
                buildings: barracks.clone(),
                unit: EntityKind::Rifleman,
                delta: -1,
            },
        )],
    );
    let repeat_units = |building| {
        entities
            .get(building)
            .expect("barracks")
            .production
            .as_ref()
            .expect("production")
            .repeat_units
            .clone()
    };
    assert_eq!(repeat_units(barracks[0]), vec![EntityKind::Rifleman]);
    assert!(repeat_units(barracks[1]).is_empty());
    assert_eq!(repeat_units(barracks[2]), vec![EntityKind::MachineGunner]);
}

#[test]
fn repeat_decrement_prefers_the_most_loaded_producer() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let mut barracks = Vec::new();
    for tile_x in [4, 10] {
        let (x, y) = footprint_center(&map, EntityKind::Barracks, tile_x, 6);
        barracks.push(
            entities
                .spawn_building(1, EntityKind::Barracks, x, y, true)
                .expect("barracks should spawn"),
        );
    }
    entities
        .get_mut(barracks[0])
        .expect("first barracks")
        .set_repeat_production(Some(EntityKind::Rifleman), true);
    entities
        .get_mut(barracks[0])
        .expect("first barracks")
        .set_repeat_production(Some(EntityKind::MachineGunner), true);
    entities
        .get_mut(barracks[1])
        .expect("second barracks")
        .set_repeat_production(Some(EntityKind::Rifleman), true);
    let mut players = vec![player_state(1), player_state(2)];

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::AdjustProductionRepeat {
                buildings: barracks.clone(),
                unit: EntityKind::Rifleman,
                delta: -1,
            },
        )],
    );

    let first_repeat = &entities
        .get(barracks[0])
        .expect("first barracks")
        .production
        .as_ref()
        .expect("production")
        .repeat_units;
    let second_repeat = &entities
        .get(barracks[1])
        .expect("second barracks")
        .production
        .as_ref()
        .expect("production")
        .repeat_units;
    assert_eq!(first_repeat, &[EntityKind::MachineGunner]);
    assert_eq!(second_repeat, &[EntityKind::Rifleman]);
}

#[test]
fn repeat_adjustment_can_clear_stale_incompatible_intent() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, bx, by, true)
        .expect("barracks should spawn");
    entities
        .get_mut(barracks)
        .expect("barracks")
        .set_repeat_production(Some(EntityKind::Tank), true);
    entities
        .get_mut(barracks)
        .expect("barracks")
        .set_repeat_production(Some(EntityKind::Rifleman), true);
    let mut players = vec![player_state(1), player_state(2)];

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::AdjustProductionRepeat {
                buildings: vec![barracks],
                unit: EntityKind::Tank,
                delta: 0,
            },
        )],
    );
    assert_eq!(
        entities
            .get(barracks)
            .expect("barracks")
            .repeat_production(),
        Some(EntityKind::Tank),
        "out-of-contract deltas should be ignored"
    );

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::AdjustProductionRepeat {
                buildings: vec![barracks],
                unit: EntityKind::Tank,
                delta: -1,
            },
        )],
    );

    assert_eq!(
        entities
            .get(barracks)
            .expect("barracks")
            .repeat_production(),
        Some(EntityKind::Rifleman)
    );
}

#[test]
fn repeat_adjustment_accepts_raw_cap_then_rejects_cap_plus_one_whole() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (x, y) = footprint_center(&map, EntityKind::Barracks, 6, 6);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, x, y, true)
        .expect("barracks should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    let mut smokes = SmokeCloudStore::new();

    for (cap, admission) in [
        (MAX_UNITS_PER_COMMAND, CommandAdmission::Normal),
        (
            LAB_MAX_UNITS_PER_COMMAND,
            CommandAdmission::LabIgnoreCommandLimits,
        ),
    ] {
        apply_with_players_and_smokes(
            &map,
            &mut entities,
            &mut players,
            &mut smokes,
            None,
            vec![PendingCommand {
                player: 1,
                command: SimCommand::AdjustProductionRepeat {
                    buildings: vec![barracks; cap],
                    unit: EntityKind::Rifleman,
                    delta: 1,
                },
                admission,
            }],
        );
        assert_eq!(
            entities
                .get(barracks)
                .expect("barracks")
                .repeat_production(),
            Some(EntityKind::Rifleman),
            "a raw list at the selected cap should be accepted and deduped"
        );

        entities
            .get_mut(barracks)
            .expect("barracks")
            .set_repeat_production(Some(EntityKind::Rifleman), false);
        apply_with_players_and_smokes(
            &map,
            &mut entities,
            &mut players,
            &mut smokes,
            None,
            vec![PendingCommand {
                player: 1,
                command: SimCommand::AdjustProductionRepeat {
                    buildings: vec![barracks; cap + 1],
                    unit: EntityKind::Rifleman,
                    delta: 1,
                },
                admission,
            }],
        );
        assert!(
            entities
                .get(barracks)
                .expect("barracks")
                .repeat_production()
                .is_none(),
            "a raw list above the selected cap must reject the whole command"
        );
    }
}
