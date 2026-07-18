use super::*;

#[test]
fn command_budget_allows_twenty_four_one_supply_units() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let units = spawn_units(&mut entities, 1, EntityKind::Rifleman, 24);
    mark_units_moving(&mut entities, &units);

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Stop {
                units: units.clone(),
            },
        )],
    );

    assert!(
        units.iter().all(|id| matches!(
            entities.get(*id).map(|entity| entity.order()),
            Some(Order::Idle)
        )),
        "24 one-supply units should fit the base command budget"
    );
}

#[test]
fn command_budget_rejects_fourth_tank_without_command_car() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let units = spawn_units(&mut entities, 1, EntityKind::Tank, 4);
    mark_units_moving(&mut entities, &units);
    let events = apply_with_players(
        &map,
        &mut entities,
        &mut [player_state(1), player_state(2)],
        vec![(
            1,
            SimCommand::Stop {
                units: units.clone(),
            },
        )],
    );

    assert!(
        units.iter().all(|id| matches!(
            entities.get(*id).map(|entity| entity.order()),
            Some(Order::Move(_))
        )),
        "four tanks should exceed the base command budget and keep their orders"
    );
    assert_notice(&events, 1, "Command supply exceeded");
}

#[test]
fn command_car_bonus_offsets_own_supply_and_stacks() {
    let map = flat_map(64);

    let mut one_car_entities = EntityStore::new();
    let mut one_car_units = spawn_units(&mut one_car_entities, 1, EntityKind::Tank, 5);
    one_car_units.extend(spawn_units(
        &mut one_car_entities,
        1,
        EntityKind::CommandCar,
        1,
    ));
    mark_units_moving(&mut one_car_entities, &one_car_units);
    apply(
        &map,
        &mut one_car_entities,
        vec![(
            1,
            SimCommand::Stop {
                units: one_car_units.clone(),
            },
        )],
    );
    assert!(
        one_car_units.iter().all(|id| matches!(
            one_car_entities.get(*id).map(|entity| entity.order()),
            Some(Order::Idle)
        )),
        "one Command Car should make five tanks legal: 44 used / 48 cap"
    );

    let mut two_car_entities = EntityStore::new();
    let mut two_car_units = spawn_units(&mut two_car_entities, 1, EntityKind::Tank, 8);
    two_car_units.extend(spawn_units(
        &mut two_car_entities,
        1,
        EntityKind::CommandCar,
        2,
    ));
    mark_units_moving(&mut two_car_entities, &two_car_units);
    apply(
        &map,
        &mut two_car_entities,
        vec![(
            1,
            SimCommand::Stop {
                units: two_car_units.clone(),
            },
        )],
    );
    assert!(
        two_car_units.iter().all(|id| matches!(
            two_car_entities.get(*id).map(|entity| entity.order()),
            Some(Order::Idle)
        )),
        "two Command Cars should stack: 72 used / 72 cap"
    );
}

#[test]
fn ordinary_raw_cap_accepts_then_dedupes_and_cap_plus_one_rejects_whole_command() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
        .expect("worker should spawn");
    entities
        .get_mut(worker)
        .expect("worker should exist")
        .set_order(Order::move_to(10.0, 10.0));
    let units = vec![worker; MAX_UNITS_PER_COMMAND];

    apply(&map, &mut entities, vec![(1, SimCommand::Stop { units })]);

    assert!(
        matches!(
            entities.get(worker).map(|entity| entity.order()),
            Some(Order::Idle)
        ),
        "a raw list exactly at the ordinary cap should be accepted and deduped"
    );

    entities
        .get_mut(worker)
        .expect("worker should exist")
        .set_order(Order::move_to(10.0, 10.0));
    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Stop {
                units: vec![worker; MAX_UNITS_PER_COMMAND + 1],
            },
        )],
    );
    assert!(matches!(
        entities.get(worker).map(|entity| entity.order()),
        Some(Order::Move(_))
    ));
}

#[test]
fn lab_command_admission_ignores_budget_and_uses_large_bounded_window() {
    let map = flat_map(256);
    let mut entities = EntityStore::new();
    let units = spawn_units(&mut entities, 1, EntityKind::Rifleman, 1_000);
    mark_units_moving(&mut entities, &units);
    let mut players = vec![player_state(1), player_state(2)];
    let mut smokes = SmokeCloudStore::new();

    apply_with_players_and_smokes(
        &map,
        &mut entities,
        &mut players,
        &mut smokes,
        vec![PendingCommand {
            player: 1,
            command: SimCommand::Stop {
                units: units.clone(),
            },
            admission: CommandAdmission::LabIgnoreCommandLimits,
        }],
    );

    assert!(
        units.iter().all(|id| matches!(
            entities.get(*id).map(|entity| entity.order()),
            Some(Order::Idle)
        )),
        "lab command-limit bypass should let one command affect 1,000 units"
    );
}

#[test]
fn lab_raw_cap_accepts_then_dedupes_and_cap_plus_one_rejects_whole_command() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
        .expect("worker should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    let mut smokes = SmokeCloudStore::new();

    for (count, should_apply) in [
        (LAB_MAX_UNITS_PER_COMMAND, true),
        (LAB_MAX_UNITS_PER_COMMAND + 1, false),
    ] {
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .set_order(Order::move_to(10.0, 10.0));
        apply_with_players_and_smokes(
            &map,
            &mut entities,
            &mut players,
            &mut smokes,
            vec![PendingCommand {
                player: 1,
                command: SimCommand::Stop {
                    units: vec![worker; count],
                },
                admission: CommandAdmission::LabIgnoreCommandLimits,
            }],
        );
        assert_eq!(
            matches!(
                entities.get(worker).map(|entity| entity.order()),
                Some(Order::Idle)
            ),
            should_apply
        );
    }
}
