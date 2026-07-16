use super::test_support::speed_scaled_escape_deadline_ticks;
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
    assert_eq!((command_car.pos_x, command_car.pos_y), (3507.0, 3664.0));
    assert!((command_car.facing() - 2.823_079_3).abs() <= 0.000_001);
    assert_units_do_not_intersect_buildings(&setup.game);
}

#[test]
fn clears_west_edge_of_building_corner() {
    let setup = Game::new_command_car_corner_scenario(EntityKind::CommandCar, 1, 0x5150_0011)
        .expect("scenario setup should succeed");
    assert_escapes_corner(setup);
}

#[test]
fn west_southwest_variant_targets_ten_left_four_down_and_escapes_corner() {
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
    assert_escapes_corner(setup);
}

fn assert_escapes_corner(mut setup: DevScenarioSetup) {
    for _ in 0..setup.issue_after_ticks {
        setup.game.tick();
    }
    let unit_id = setup.units[0];
    let western_building_edge = setup
        .game
        .state
        .entities
        .iter()
        .filter_map(|entity| {
            services::geometry::building_rect_for_entity(&setup.game.state.map, entity)
                .map(|rect| rect.min_x)
        })
        .min_by(f32::total_cmp)
        .expect("scenario should include buildings");
    let start_body = services::geometry::unit_body_for_entity(
        setup
            .game
            .state
            .entities
            .get(unit_id)
            .expect("scenario Command Car should exist"),
    )
    .expect("Command Car should have a movement body");
    let escape_distance_px = (start_body.aabb().max_x - western_building_edge).max(0.0);
    let deadline_ticks =
        speed_scaled_escape_deadline_ticks(EntityKind::CommandCar, escape_distance_px, 6);
    setup.game.enqueue(
        setup.player_id,
        SimCommand::Move {
            units: setup.units.clone(),
            x: setup.goal.0,
            y: setup.goal.1,
            queued: false,
        },
    );

    let mut escaped_tick = None;
    for tick in 1..=deadline_ticks {
        setup.game.tick();
        assert_units_do_not_intersect_buildings(&setup.game);
        let command_car = setup
            .game
            .state
            .entities
            .get(unit_id)
            .expect("scenario Command Car should survive");
        let body = services::geometry::unit_body_for_entity(command_car)
            .expect("Command Car should have a movement body");
        if body.aabb().max_x < western_building_edge {
            escaped_tick = Some(tick);
            break;
        }
    }

    let command_car = setup
        .game
        .state
        .entities
        .get(unit_id)
        .expect("scenario Command Car should survive");
    let body = services::geometry::unit_body_for_entity(command_car)
        .expect("Command Car should have a movement body");
    let remaining_escape_distance_px = (body.aabb().max_x - western_building_edge).max(0.0);
    assert!(
        escaped_tick.is_some(),
        "Command Car should clear the west edge of the building corner within {deadline_ticks} speed-scaled ticks, still {remaining_escape_distance_px:.2}px inside it"
    );
}
