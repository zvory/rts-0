use super::*;

#[derive(Debug)]
struct DevScenarioTiming {
    scenario: &'static str,
    unit: EntityKind,
    count: usize,
    issue_delay_ticks: u32,
    clear_ticks: Option<u32>,
    clear_seconds: Option<f32>,
    final_state: Vec<String>,
}

fn units_clear(game: &Game, units: &[u32]) -> bool {
    units.iter().all(|&id| {
        game.state
            .entities
            .get(id)
            .is_some_and(|e| e.move_phase().is_none() && e.path_is_empty())
    })
}

fn describe_state(game: &Game, units: &[u32]) -> Vec<String> {
    units
        .iter()
        .filter_map(|&id| {
            let e = game.state.entities.get(id)?;
            Some(format!(
                "#{id}: pos=({:.1},{:.1}) facing={:.3} phase={:?} path_len={} next={:?} goal={:?}",
                e.pos_x,
                e.pos_y,
                e.facing(),
                e.move_phase(),
                e.movement.as_ref().map(|m| m.path.len()).unwrap_or(0),
                e.next_waypoint(),
                e.path_goal(),
            ))
        })
        .collect()
}

fn measure_clear_time(
    scenario: &'static str,
    unit: EntityKind,
    count: usize,
    setup: DevScenarioSetup,
) -> DevScenarioTiming {
    let mut game = setup.game;
    let units = setup.units;
    while game.tick_count() < setup.issue_after_ticks {
        game.tick();
    }
    let issued_at = game.tick_count();
    game.enqueue(
        setup.player_id,
        SimCommand::Move {
            units: units.clone(),
            x: setup.goal.0,
            y: setup.goal.1,
            queued: false,
        },
    );

    for _ in 0..12_000u32 {
        game.tick();
        if units_clear(&game, &units) {
            let ticks = game.tick_count().saturating_sub(issued_at);
            return DevScenarioTiming {
                scenario,
                unit,
                count,
                issue_delay_ticks: setup.issue_after_ticks,
                clear_ticks: Some(ticks),
                clear_seconds: Some(ticks as f32 / config::TICK_HZ as f32),
                final_state: describe_state(&game, &units),
            };
        }
    }

    DevScenarioTiming {
        scenario,
        unit,
        count,
        issue_delay_ticks: setup.issue_after_ticks,
        clear_ticks: None,
        clear_seconds: None,
        final_state: describe_state(&game, &units),
    }
}

fn print_results(results: &[DevScenarioTiming], scenario_width: usize) {
    println!(
        "scenario | unit | count | issue_delay_ticks | clear_ticks | clear_seconds | final_state"
    );
    for result in results {
        match (result.clear_ticks, result.clear_seconds) {
            (Some(ticks), Some(seconds)) => println!(
                "{:>width$} | {:>14} | {:>5} | {:>17} | {:>11} | {:>13.2} | {:?}",
                result.scenario,
                result.unit,
                result.count,
                result.issue_delay_ticks,
                ticks,
                seconds,
                result.final_state,
                width = scenario_width,
            ),
            _ => println!(
                "{:>width$} | {:>14} | {:>5} | {:>17} | {:>11} | {:>13} | {:?}",
                result.scenario,
                result.unit,
                result.count,
                result.issue_delay_ticks,
                "timeout",
                "timeout",
                result.final_state,
                width = scenario_width,
            ),
        }
    }
}

// Run these manual measurements with:
// cargo test --manifest-path server/Cargo.toml -p rts-sim experimental_ -- --ignored --nocapture
#[test]
#[ignore = "manual clear-time measurement; prints results without asserting behavior"]
fn experimental_direct_reverse_and_corner_wall_clear_time_matrix() {
    let mut results = Vec::new();
    for unit in [
        EntityKind::AntiTankGun,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
        let setup = Game::new_direct_reverse_order_scenario(unit, 1, 0x5150_0007)
            .expect("scenario setup should succeed");
        results.push(measure_clear_time("direct_reverse_order", unit, 1, setup));
    }
    for unit in [
        EntityKind::AntiTankGun,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
        for count in [1usize, 3, 5] {
            let setup = Game::new_vehicle_corner_wall_scenario(unit, count, 0x5150_0008)
                .expect("scenario setup should succeed");
            results.push(measure_clear_time(
                "vehicle_corner_wall",
                unit,
                count,
                setup,
            ));
        }
    }

    println!("EXPERIMENTAL_DIRECT_REVERSE_AND_CORNER_WALL_CLEAR_TIMES");
    print_results(&results, 24);
}

#[test]
#[ignore = "manual clear-time measurement; prints results without asserting behavior"]
fn experimental_factory_zero_gap_perpendicular_clear_time_matrix() {
    let mut results = Vec::new();
    for unit in [
        EntityKind::AntiTankGun,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
        let setup = Game::new_factory_zero_gap_perpendicular_scenario(unit, 1, 0x5150_0010)
            .expect("scenario setup should succeed");
        results.push(measure_clear_time(
            "factory_zero_gap_perpendicular",
            unit,
            1,
            setup,
        ));
    }

    println!("FACTORY_ZERO_GAP_PERPENDICULAR_CLEAR_TIMES");
    print_results(&results, 32);
}
