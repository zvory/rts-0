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
fn command_car_scout_plane_ability_spends_resources_and_uses_nearest_city_centre() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let (far_x, far_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, far_x, far_y, true)
        .expect("far city centre should spawn");
    let (near_x, near_y) = footprint_center(&map, EntityKind::CityCentre, 18, 18);
    let near_city_centre = entities
        .spawn_building(1, EntityKind::CityCentre, near_x, near_y, true)
        .expect("near city centre should spawn");
    let command_car = entities
        .spawn_unit(1, EntityKind::CommandCar, 128.0, 128.0)
        .expect("command car should spawn");
    let mut players = vec![player_state(1), player_state(2)];

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, scout_plane_command(vec![command_car], near_x + 16.0, near_y + 16.0))],
    );

    assert_eq!(players[0].steel, 950);
    assert_eq!(players[0].oil, 950);
    assert_eq!(
        players[0]
            .ability_cooldowns
            .get(&AbilityKind::ScoutPlane)
            .copied()
            .unwrap_or(0),
        config::SCOUT_PLANE_ABILITY_COOLDOWN_TICKS
    );
    let plane = entities
        .iter()
        .find(|entity| entity.kind == EntityKind::ScoutPlane)
        .expect("Scout Plane should spawn");
    assert_eq!(plane.owner, 1);
    assert_eq!(plane.pos_x, near_x);
    assert_eq!(plane.pos_y, near_y);
    let state = plane.scout_plane_state().expect("plane state");
    assert_eq!(state.home_city_centre, Some(near_city_centre));
    assert_eq!(state.orbit_center, (near_x + 16.0, near_y + 16.0));
    assert_notice(&events, 1, "Scout Plane");
}

#[test]
fn command_car_scout_plane_ability_rejects_active_plane_before_spending() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 8, 8);
    entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let command_car = entities
        .spawn_unit(1, EntityKind::CommandCar, 128.0, 128.0)
        .expect("command car should spawn");
    entities
        .spawn_unit(1, EntityKind::ScoutPlane, cc_x, cc_y)
        .expect("active Scout Plane should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    players[0]
        .ability_cooldowns
        .remove(&AbilityKind::ScoutPlane);
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
    assert_notice(&events, 1, "Scout Plane already active");
}
