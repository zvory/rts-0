use super::*;

fn assert_units_do_not_intersect_buildings(game: &Game) {
    let buildings: Vec<_> = game
        .state
        .entities
        .iter()
        .filter_map(|entity| {
            services::geometry::building_rect_for_entity(&game.state.map, entity)
                .map(|rect| (entity.id, entity.kind, rect))
        })
        .collect();
    for unit in game.state.entities.iter().filter(|entity| entity.is_unit()) {
        let Some(body) = services::geometry::unit_body_for_entity(unit) else {
            continue;
        };
        for &(building_id, building_kind, rect) in &buildings {
            assert!(
                !services::geometry::unit_body_intersects_rect(body, rect),
                "scenario unit {} ({}) intersects building {} ({})",
                unit.id,
                unit.kind,
                building_id,
                building_kind
            );
        }
    }
}

#[test]
fn matches_reduced_reproduction_layout() {
    let setup = Game::new_command_car_corner_scenario(EntityKind::CommandCar, 1, 0x5150_0011)
        .expect("scenario setup should succeed");
    assert_eq!(setup.issue_after_ticks, config::TICK_HZ);
    assert_eq!(setup.goal, (3216.0, 3472.0));
    assert_eq!(setup.units.len(), 1);
    for (kind, expected_pos) in [
        (EntityKind::Factory, (3472.0, 3728.0)),
        (EntityKind::TrainingCentre, (3440.0, 3648.0)),
        (EntityKind::Barracks, (3536.0, 3584.0)),
    ] {
        let building = setup
            .game
            .state
            .entities
            .iter()
            .find(|entity| entity.kind == kind)
            .unwrap_or_else(|| panic!("scenario should include {kind}"));
        assert_eq!((building.pos_x, building.pos_y), expected_pos);
    }
    let command_car = setup
        .game
        .state
        .entities
        .get(setup.units[0])
        .expect("scenario Command Car should exist");
    assert_eq!(command_car.kind, EntityKind::CommandCar);
    assert_eq!((command_car.pos_x, command_car.pos_y), (3536.0, 3664.0));
    assert!((command_car.facing() - 2.823_079_3).abs() <= 0.000_001);
    assert_units_do_not_intersect_buildings(&setup.game);
}

#[test]
fn backs_out_and_completes_route() {
    let setup = Game::new_command_car_corner_scenario(EntityKind::CommandCar, 1, 0x5150_0011)
        .expect("scenario setup should succeed");
    assert_completes_route(setup, true);
}

#[test]
fn west_southwest_variant_targets_ten_left_four_down_and_completes_route() {
    let setup = Game::new_command_car_corner_west_southwest_scenario(
        EntityKind::CommandCar,
        1,
        0x5150_0011,
    )
    .expect("west-southwest scenario setup should succeed");
    let command_car = setup
        .game
        .state
        .entities
        .get(setup.units[0])
        .expect("scenario Command Car should exist");
    assert_eq!(
        setup.goal.0,
        command_car.pos_x - config::TILE_SIZE as f32 * 10.0
    );
    assert_eq!(
        setup.goal.1,
        command_car.pos_y + config::TILE_SIZE as f32 * 4.0
    );
    assert_completes_route(setup, false);
}

fn assert_completes_route(mut setup: DevScenarioSetup, expect_initial_reverse: bool) {
    for _ in 0..setup.issue_after_ticks {
        setup.game.tick();
    }
    let unit_id = setup.units[0];
    let start = setup
        .game
        .state
        .entities
        .get(unit_id)
        .map(|entity| (entity.pos_x, entity.pos_y, entity.facing()))
        .expect("scenario Command Car should exist");
    setup.game.enqueue(
        setup.player_id,
        SimCommand::Move {
            units: setup.units.clone(),
            x: setup.goal.0,
            y: setup.goal.1,
            queued: false,
        },
    );

    setup.game.tick();
    let after_first_tick = setup
        .game
        .state
        .entities
        .get(unit_id)
        .expect("scenario Command Car should survive");
    assert_units_do_not_intersect_buildings(&setup.game);
    let first_delta = (
        after_first_tick.pos_x - start.0,
        after_first_tick.pos_y - start.1,
    );
    let forward = (start.2.cos(), start.2.sin());
    if expect_initial_reverse {
        assert!(
            first_delta.0 * forward.0 + first_delta.1 * forward.1 < 0.0,
            "the first maneuver should reverse toward the route exit, got delta {first_delta:?}"
        );
    }

    let mut arrived_tick = None;
    let mut saw_intermediate_reverse = after_first_tick
        .movement
        .as_ref()
        .is_some_and(|movement| {
            movement.path.len() > 1 && movement.scout_car_reverse_waypoint.is_some()
        });
    for tick in 2..=600 {
        setup.game.tick();
        assert_units_do_not_intersect_buildings(&setup.game);
        let command_car = setup
            .game
            .state
            .entities
            .get(unit_id)
            .expect("scenario Command Car should survive");
        saw_intermediate_reverse |= command_car
            .movement
            .as_ref()
            .is_some_and(|movement| {
                movement.path.len() > 1 && movement.scout_car_reverse_waypoint.is_some()
            });
        if command_car.path_is_empty() {
            arrived_tick = Some(tick);
            break;
        }
    }

    let command_car = setup
        .game
        .state
        .entities
        .get(unit_id)
        .expect("scenario Command Car should survive");
    let distance_to_goal = ((command_car.pos_x - setup.goal.0).powi(2)
        + (command_car.pos_y - setup.goal.1).powi(2))
    .sqrt();
    assert!(
        saw_intermediate_reverse,
        "the corner route should exercise a latched intermediate reverse maneuver"
    );
    assert!(
        arrived_tick.is_some(),
        "Command Car should finish the route, stopped {distance_to_goal:.2}px from the goal"
    );
    assert!(
        distance_to_goal <= config::SCOUT_CAR_FINAL_GOAL_TOLERANCE_PX,
        "Command Car should finish near the ordered goal, stopped {distance_to_goal:.2}px away"
    );
}
