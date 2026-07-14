use super::*;

fn scout_plane_command(units: Vec<u32>, x: f32, y: f32) -> SimCommand {
    SimCommand::UseAbility {
        ability: AbilityKind::ScoutPlane,
        units,
        x: Some(x),
        y: Some(y),
        queued: false,
    }
}

#[test]
fn scout_plane_is_no_longer_trained_at_city_centres() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 6, 6);
    let city_centre = entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    let resources_before = (
        players[0].steel,
        players[0].oil,
        players[0].supply_used,
        players[0].supply_cap,
    );

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Train {
                building: city_centre,
                unit: EntityKind::ScoutPlane,
            },
        )],
    );

    assert!(
        entities
            .get(city_centre)
            .expect("city centre")
            .prod_queue()
            .is_empty(),
        "Scout Plane should not enter the City Centre production queue"
    );
    assert_eq!(
        (
            players[0].steel,
            players[0].oil,
            players[0].supply_used,
            players[0].supply_cap,
        ),
        resources_before,
        "rejected Scout Plane training must not spend resources or reserve supply"
    );
    assert_notice(&events, 1, "Cannot train that here");
}

#[test]
fn command_car_scout_plane_ability_launches_from_caster_without_a_city_centre() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let command_car = entities
        .spawn_unit(1, EntityKind::CommandCar, 128.0, 128.0)
        .expect("command car should spawn");
    let mut players = vec![player_state(1), player_state(2)];

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, scout_plane_command(vec![command_car], 592.0, 592.0))],
    );

    assert_eq!(players[0].steel, 950);
    assert_eq!(players[0].oil, 925);
    assert_eq!(
        entities
            .get(command_car)
            .expect("command car")
            .ability_cooldown_ticks(AbilityKind::ScoutPlane),
        config::SCOUT_PLANE_ABILITY_COOLDOWN_TICKS
    );
    let plane = entities
        .iter()
        .find(|entity| entity.kind == EntityKind::ScoutPlane)
        .expect("Scout Plane should spawn");
    assert_eq!(plane.owner, 1);
    assert_eq!(plane.pos_x, 128.0);
    assert_eq!(plane.pos_y, 128.0);
    let state = plane.scout_plane_state().expect("plane state");
    assert_eq!(state.source_command_car, Some(command_car));
    assert_eq!(state.orbit_center, (592.0, 592.0));
    assert_notice(&events, 1, "Scout Plane");
}

#[test]
fn each_command_car_can_launch_its_own_scout_plane() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let first_car = entities
        .spawn_unit(1, EntityKind::CommandCar, 128.0, 128.0)
        .expect("first command car should spawn");
    let second_car = entities
        .spawn_unit(1, EntityKind::CommandCar, 192.0, 128.0)
        .expect("second command car should spawn");
    let mut players = vec![player_state(1), player_state(2)];

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![
            (1, scout_plane_command(vec![first_car], 512.0, 512.0)),
            (1, scout_plane_command(vec![second_car], 640.0, 512.0)),
        ],
    );

    assert_eq!((players[0].steel, players[0].oil), (900, 850));
    let mut source_cars = entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::ScoutPlane && entity.owner == 1)
        .filter_map(|plane| plane.scout_plane_state()?.source_command_car)
        .collect::<Vec<_>>();
    source_cars.sort_unstable();
    assert_eq!(source_cars, vec![first_car, second_car]);
    for command_car in [first_car, second_car] {
        assert_eq!(
            entities
                .get(command_car)
                .expect("command car")
                .ability_cooldown_ticks(AbilityKind::ScoutPlane),
            config::SCOUT_PLANE_ABILITY_COOLDOWN_TICKS
        );
    }
    assert_notice(&events, 1, "Scout Plane");
}

#[test]
fn command_car_scout_plane_ability_does_not_interrupt_caster_orders() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let command_car = entities
        .spawn_unit(1, EntityKind::CommandCar, 128.0, 128.0)
        .expect("command car should spawn");
    {
        let caster = entities
            .get_mut(command_car)
            .expect("command car should exist");
        caster.set_order(Order::move_to(640.0, 640.0));
        caster.append_queued_order(OrderIntent::move_to(672.0, 672.0));
    }
    let mut players = vec![player_state(1), player_state(2)];

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, scout_plane_command(vec![command_car], 512.0, 512.0))],
    );

    let caster = entities
        .get(command_car)
        .expect("command car should survive launch");
    assert!(
        matches!(caster.order(), Order::Move(_)),
        "Scout Plane launch should not replace the Command Car's active order"
    );
    assert_eq!(
        caster.queued_orders().len(),
        1,
        "Scout Plane launch should not clear the Command Car's queued orders"
    );
    assert_notice(&events, 1, "Scout Plane");
}

#[test]
fn command_car_scout_plane_ability_rejects_active_plane_before_spending() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let command_car = entities
        .spawn_unit(1, EntityKind::CommandCar, 128.0, 128.0)
        .expect("command car should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    let _ = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, scout_plane_command(vec![command_car], 512.0, 512.0))],
    );
    entities
        .get_mut(command_car)
        .expect("command car")
        .start_ability_cooldown(AbilityKind::ScoutPlane, 0);
    let resources_before = (players[0].steel, players[0].oil);

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, scout_plane_command(vec![command_car], 512.0, 512.0))],
    );

    assert_eq!((players[0].steel, players[0].oil), resources_before);
    assert_eq!(
        entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::ScoutPlane && entity.owner == 1)
            .count(),
        1
    );
    assert_notice(
        &events,
        1,
        "Scout Plane already active for this Command Car",
    );
}

#[test]
fn scout_plane_command_skips_selected_car_with_active_plane() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let active_car = entities
        .spawn_unit(1, EntityKind::CommandCar, 128.0, 128.0)
        .expect("active command car should spawn");
    let available_car = entities
        .spawn_unit(1, EntityKind::CommandCar, 192.0, 128.0)
        .expect("available command car should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    let _ = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, scout_plane_command(vec![active_car], 512.0, 512.0))],
    );
    entities
        .get_mut(active_car)
        .expect("active command car")
        .start_ability_cooldown(AbilityKind::ScoutPlane, 0);

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            scout_plane_command(vec![active_car, available_car], 640.0, 512.0),
        )],
    );

    assert_eq!((players[0].steel, players[0].oil), (900, 850));
    let mut source_cars = entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::ScoutPlane && entity.owner == 1)
        .filter_map(|plane| plane.scout_plane_state()?.source_command_car)
        .collect::<Vec<_>>();
    source_cars.sort_unstable();
    assert_eq!(source_cars, vec![active_car, available_car]);
    assert_eq!(
        entities
            .get(available_car)
            .expect("available command car")
            .ability_cooldown_ticks(AbilityKind::ScoutPlane),
        config::SCOUT_PLANE_ABILITY_COOLDOWN_TICKS
    );
    assert_notice(&events, 1, "Scout Plane");
}
