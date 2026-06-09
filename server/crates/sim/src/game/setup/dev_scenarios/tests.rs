use super::*;
use rayon::prelude::*;

fn owned_kind_count(game: &Game, owner: u32, kind: EntityKind) -> usize {
    game.entities
        .iter()
        .filter(|e| e.owner == owner && e.kind == kind)
        .count()
}

#[derive(Debug)]
struct VehicleSmallBlockTiming {
    vehicle: EntityKind,
    pair_count: usize,
    blocker: Option<EntityKind>,
    clear_ticks: Option<u32>,
    clear_seconds: Option<f32>,
    final_state: Vec<String>,
}

#[derive(Debug)]
struct DevScenarioTiming {
    scenario: &'static str,
    unit: EntityKind,
    count: usize,
    clear_ticks: Option<u32>,
    clear_seconds: Option<f32>,
    final_state: Vec<String>,
}

fn blocker_label(blocker: Option<EntityKind>) -> &'static str {
    match blocker {
        None => "none",
        Some(EntityKind::Worker) => "worker",
        Some(EntityKind::Rifleman) => "rifleman",
        Some(EntityKind::MachineGunner) => "machine_gunner",
        Some(EntityKind::AtTeam) => "at_team",
        Some(_) => "unsupported",
    }
}

const VEHICLE_SMALL_BLOCK_BASELINES: &[(EntityKind, usize, Option<EntityKind>, u32)] = &[
    (EntityKind::ScoutCar, 1, Some(EntityKind::AtTeam), 291),
    (
        EntityKind::ScoutCar,
        1,
        Some(EntityKind::MachineGunner),
        295,
    ),
    (EntityKind::ScoutCar, 1, None, 272),
    (EntityKind::ScoutCar, 1, Some(EntityKind::Rifleman), 272),
    (EntityKind::ScoutCar, 1, Some(EntityKind::Worker), 272),
    (EntityKind::ScoutCar, 3, Some(EntityKind::AtTeam), 326),
    (
        EntityKind::ScoutCar,
        3,
        Some(EntityKind::MachineGunner),
        338,
    ),
    (EntityKind::ScoutCar, 3, None, 298),
    (EntityKind::ScoutCar, 3, Some(EntityKind::Rifleman), 298),
    (EntityKind::ScoutCar, 3, Some(EntityKind::Worker), 298),
    (EntityKind::ScoutCar, 5, Some(EntityKind::AtTeam), 367),
    (
        EntityKind::ScoutCar,
        5,
        Some(EntityKind::MachineGunner),
        360,
    ),
    (EntityKind::ScoutCar, 5, None, 333),
    (EntityKind::ScoutCar, 5, Some(EntityKind::Rifleman), 333),
    (EntityKind::ScoutCar, 5, Some(EntityKind::Worker), 333),
    (EntityKind::Tank, 1, Some(EntityKind::AtTeam), 330),
    (EntityKind::Tank, 1, Some(EntityKind::MachineGunner), 330),
    (EntityKind::Tank, 1, None, 320),
    (EntityKind::Tank, 1, Some(EntityKind::Rifleman), 320),
    (EntityKind::Tank, 1, Some(EntityKind::Worker), 320),
    (EntityKind::Tank, 3, Some(EntityKind::AtTeam), 466),
    (EntityKind::Tank, 3, Some(EntityKind::MachineGunner), 462),
    (EntityKind::Tank, 3, None, 446),
    (EntityKind::Tank, 3, Some(EntityKind::Rifleman), 446),
    (EntityKind::Tank, 3, Some(EntityKind::Worker), 446),
    (EntityKind::Tank, 5, Some(EntityKind::AtTeam), 546),
    (EntityKind::Tank, 5, Some(EntityKind::MachineGunner), 513),
    (EntityKind::Tank, 5, None, 525),
    (EntityKind::Tank, 5, Some(EntityKind::Rifleman), 525),
    (EntityKind::Tank, 5, Some(EntityKind::Worker), 525),
];

fn vehicle_small_block_baseline(
    vehicle: EntityKind,
    pair_count: usize,
    blocker: Option<EntityKind>,
) -> u32 {
    VEHICLE_SMALL_BLOCK_BASELINES
        .iter()
        .find_map(
            |(baseline_vehicle, baseline_count, baseline_blocker, ticks)| {
                (*baseline_vehicle == vehicle
                    && *baseline_count == pair_count
                    && *baseline_blocker == blocker)
                    .then_some(*ticks)
            },
        )
        .expect("baseline should exist for each vehicle-small-block scenario")
}

fn describe_vehicle_small_block_state(game: &Game, units: &[u32]) -> Vec<String> {
    units
        .iter()
        .filter_map(|&id| {
            let e = game.entities.get(id)?;
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

fn vehicle_small_block_vehicles_clear(game: &Game, units: &[u32]) -> bool {
    units.iter().all(|&id| {
        game.entities
            .get(id)
            .is_some_and(|e| e.move_phase().is_none() && e.path_is_empty())
    })
}

fn dev_scenario_units_clear(game: &Game, units: &[u32]) -> bool {
    units.iter().all(|&id| {
        game.entities
            .get(id)
            .is_some_and(|e| e.move_phase().is_none() && e.path_is_empty())
    })
}

fn describe_dev_scenario_state(game: &Game, units: &[u32]) -> Vec<String> {
    units
        .iter()
        .filter_map(|&id| {
            let e = game.entities.get(id)?;
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

fn measure_dev_scenario_clear_time(
    scenario: &'static str,
    unit: EntityKind,
    count: usize,
    setup: DevScenarioSetup,
) -> DevScenarioTiming {
    let mut game = setup.game;
    let units = setup.units;
    game.enqueue(
        setup.player_id,
        SimCommand::Move {
            units: units.clone(),
            x: setup.goal.0,
            y: setup.goal.1,
            queued: false,
        },
    );

    let max_ticks = 12_000u32;
    for _ in 0..max_ticks {
        game.tick();
        if dev_scenario_units_clear(&game, &units) {
            let ticks = game.tick_count();
            return DevScenarioTiming {
                scenario,
                unit,
                count,
                clear_ticks: Some(ticks),
                clear_seconds: Some(ticks as f32 / config::TICK_HZ as f32),
                final_state: describe_dev_scenario_state(&game, &units),
            };
        }
    }

    DevScenarioTiming {
        scenario,
        unit,
        count,
        clear_ticks: None,
        clear_seconds: None,
        final_state: describe_dev_scenario_state(&game, &units),
    }
}

fn measure_vehicle_small_block_clear_time(
    vehicle: EntityKind,
    pair_count: usize,
    blocker: Option<EntityKind>,
) -> VehicleSmallBlockTiming {
    let setup = Game::new_vehicle_small_block_baseline_scenario(
        vehicle,
        pair_count,
        blocker,
        0x5150_0004,
    )
    .expect("scenario setup should succeed");
    let mut game = setup.game;
    let units = setup.units;
    game.enqueue(
        setup.player_id,
        SimCommand::Move {
            units: units.clone(),
            x: setup.goal.0,
            y: setup.goal.1,
            queued: false,
        },
    );

    let max_ticks = 12_000u32;
    for _ in 0..max_ticks {
        game.tick();
        if vehicle_small_block_vehicles_clear(&game, &units) {
            let ticks = game.tick_count();
            return VehicleSmallBlockTiming {
                vehicle,
                pair_count,
                blocker,
                clear_ticks: Some(ticks),
                clear_seconds: Some(ticks as f32 / config::TICK_HZ as f32),
                final_state: describe_vehicle_small_block_state(&game, &units),
            };
        }
    }

    VehicleSmallBlockTiming {
        vehicle,
        pair_count,
        blocker,
        clear_ticks: None,
        clear_seconds: None,
        final_state: describe_vehicle_small_block_state(&game, &units),
    }
}

#[test]
fn direct_reverse_order_scenario_faces_unit_east_and_orders_goal_behind() {
    for unit in [EntityKind::AtTeam, EntityKind::ScoutCar, EntityKind::Tank] {
        let setup = Game::new_direct_reverse_order_scenario(unit, 1, 0x5150_0003)
            .expect("scenario setup should succeed");
        let unit_id = *setup.units.first().expect("scenario should spawn one unit");
        let entity = setup
            .game
            .entities
            .get(unit_id)
            .expect("scenario unit should exist");
        let goal_delta_x = entity.pos_x - setup.goal.0;
        assert!(
            (goal_delta_x - config::TILE_SIZE as f32 * 15.0).abs() <= 0.001,
            "{unit} should receive a goal 15 tiles behind, delta {goal_delta_x:.2}"
        );
        assert!(
            (entity.pos_y - setup.goal.1).abs() <= 0.001,
            "{unit} goal should be directly behind on the same y axis"
        );
        assert!(
            entity.facing().abs() <= 0.001,
            "{unit} should begin facing east, facing {:.4}",
            entity.facing()
        );
    }
}

#[test]
fn vehicle_corner_wall_scenario_matches_authored_layout() {
    let setup = Game::new_vehicle_corner_wall_scenario(EntityKind::Tank, 5, 0x5150_0005)
        .expect("scenario setup should succeed");
    assert_eq!(setup.units.len(), 5);
    assert_eq!(owned_kind_count(&setup.game, 1, EntityKind::Tank), 5);

    let map = &setup.game.map;
    let wall_left_x = map.size / 2;
    let wall_right_x = wall_left_x + 2;
    let wall_top_y = map.size / 2 - 8;
    let wall_bottom_y = wall_top_y + 16;
    let wall_tiles = (wall_right_x - wall_left_x + 1) * (wall_bottom_y - wall_top_y + 1);
    let rock_tiles = map
        .terrain
        .iter()
        .filter(|&&terrain| terrain == crate::protocol::terrain::ROCK)
        .count();
    assert_eq!(
        rock_tiles, wall_tiles as usize,
        "corner-wall map should be plain except for the stone spur"
    );
    for ty in wall_top_y..=wall_bottom_y {
        for tx in wall_left_x..=wall_right_x {
            assert_eq!(
                map.terrain[map.index(tx, ty)],
                crate::protocol::terrain::ROCK,
                "wall tile {tx},{ty} should be stone"
            );
        }
    }
    assert_eq!(
        map.terrain[map.index(wall_left_x - 1, wall_top_y)],
        crate::protocol::terrain::GRASS
    );
    assert_eq!(
        map.terrain[map.index(wall_right_x + 1, wall_top_y)],
        crate::protocol::terrain::GRASS
    );

    let ts = config::TILE_SIZE as f32;
    let lead = setup
        .game
        .entities
        .get(setup.units[0])
        .expect("lead tank should exist");
    let lead_x = wall_left_x as f32 * ts - ts;
    let lead_y = (wall_top_y as f32 + 7.5) * ts;
    assert!((lead.pos_x - lead_x).abs() <= 0.001);
    assert!((lead.pos_y - lead_y).abs() <= 0.001);
    assert!((setup.goal.0 - ((wall_right_x + 1) as f32 * ts + ts * 0.5)).abs() <= 0.001);
    assert!((setup.goal.1 - lead_y).abs() <= 0.001);

    let north = -std::f32::consts::FRAC_PI_2;
    let (side_spacing, rear_spacing) = vehicle_corner_wall_spawn_spacing(EntityKind::Tank);
    let expected = [
        (lead_x, lead_y),
        (lead_x, lead_y + rear_spacing),
        (lead_x, lead_y + rear_spacing * 2.0),
        (lead_x - side_spacing, lead_y),
        (lead_x - side_spacing * 2.0, lead_y),
    ];
    for (unit_id, (expected_x, expected_y)) in setup.units.iter().zip(expected) {
        let entity = setup
            .game
            .entities
            .get(*unit_id)
            .expect("scenario tank should exist");
        assert_eq!(entity.kind, EntityKind::Tank);
        assert!((entity.pos_x - expected_x).abs() <= 0.001);
        assert!((entity.pos_y - expected_y).abs() <= 0.001);
        assert!((entity.facing() - north).abs() <= 0.001);
    }
}

#[test]
fn vehicle_corner_wall_scenario_supports_all_vehicle_counts() {
    for unit in [EntityKind::AtTeam, EntityKind::ScoutCar, EntityKind::Tank] {
        for count in [1usize, 3, 5] {
            let setup = Game::new_vehicle_corner_wall_scenario(unit, count, 0x5150_0006)
                .expect("scenario setup should succeed");
            assert_eq!(setup.units.len(), count);
            assert_eq!(owned_kind_count(&setup.game, 1, unit), count);
        }
    }
}

#[test]
fn experimental_direct_reverse_and_corner_wall_clear_time_matrix() {
    let mut results = Vec::new();
    for unit in [EntityKind::AtTeam, EntityKind::ScoutCar, EntityKind::Tank] {
        let setup = Game::new_direct_reverse_order_scenario(unit, 1, 0x5150_0007)
            .expect("scenario setup should succeed");
        results.push(measure_dev_scenario_clear_time(
            "direct_reverse_order",
            unit,
            1,
            setup,
        ));
    }
    for unit in [EntityKind::AtTeam, EntityKind::ScoutCar, EntityKind::Tank] {
        for count in [1usize, 3, 5] {
            let setup = Game::new_vehicle_corner_wall_scenario(unit, count, 0x5150_0008)
                .expect("scenario setup should succeed");
            results.push(measure_dev_scenario_clear_time(
                "vehicle_corner_wall",
                unit,
                count,
                setup,
            ));
        }
    }

    println!("EXPERIMENTAL_DIRECT_REVERSE_AND_CORNER_WALL_CLEAR_TIMES");
    println!("scenario | unit | count | clear_ticks | clear_seconds | final_state");
    for result in &results {
        match (result.clear_ticks, result.clear_seconds) {
            (Some(ticks), Some(seconds)) => println!(
                "{:>24} | {:>14} | {:>5} | {:>11} | {:>13.2} | {:?}",
                result.scenario,
                result.unit,
                result.count,
                ticks,
                seconds,
                result.final_state
            ),
            _ => println!(
                "{:>24} | {:>14} | {:>5} | {:>11} | {:>13} | {:?}",
                result.scenario,
                result.unit,
                result.count,
                "timeout",
                "timeout",
                result.final_state
            ),
        }
    }
}

fn assert_vehicle_small_block_baseline_setup(
    vehicle: EntityKind,
    pair_count: usize,
    blocker: Option<EntityKind>,
) {
    let setup = Game::new_vehicle_small_block_baseline_scenario(
        vehicle,
        pair_count,
        blocker,
        0x5150_0004,
    )
    .expect("scenario setup should succeed");
    assert_eq!(
        setup.units.len(),
        pair_count,
        "{vehicle} scenario should command one vehicle per pair"
    );
    assert_eq!(owned_kind_count(&setup.game, 1, vehicle), pair_count);
    for kind in [
        EntityKind::Worker,
        EntityKind::Rifleman,
        EntityKind::MachineGunner,
        EntityKind::AtTeam,
    ] {
        let expected = (blocker == Some(kind)).then_some(pair_count).unwrap_or(0);
        assert_eq!(
            owned_kind_count(&setup.game, 1, kind),
            expected,
            "{vehicle} scenario should spawn expected {kind} blockers"
        );
    }

    let north = -std::f32::consts::FRAC_PI_2;
    let mut vehicle_positions: Vec<(f32, f32)> = setup
        .units
        .iter()
        .map(|id| {
            let entity = setup
                .game
                .entities
                .get(*id)
                .expect("scenario vehicle should exist");
            assert_eq!(entity.kind, vehicle);
            assert!(
                (entity.facing() - north).abs() <= 0.001,
                "{vehicle} should begin facing north, facing {:.4}",
                entity.facing()
            );
            (entity.pos_x, entity.pos_y)
        })
        .collect();
    vehicle_positions.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut blocker_positions: Vec<(f32, f32)> = setup
        .game
        .entities
        .iter()
        .filter(|e| e.owner == 1 && Some(e.kind) == blocker)
        .map(|e| (e.pos_x, e.pos_y))
        .collect();
    blocker_positions.sort_by(|a, b| a.0.total_cmp(&b.0));

    let expected_spacing = vehicle_small_block_baseline_vehicle_spacing(vehicle);
    for pair in vehicle_positions.windows(2) {
        let gap = pair[1].0 - pair[0].0;
        assert!(
            (gap - expected_spacing).abs() <= 0.001,
            "{vehicle} adjacent vehicle spacing should be {expected_spacing:.2}px, got {gap:.2}px"
        );
    }
    if blocker.is_some() {
        for (vehicle_pos, blocker_pos) in vehicle_positions.iter().zip(blocker_positions.iter())
        {
            assert!(
                (vehicle_pos.0 - blocker_pos.0).abs() <= 0.001,
                "{vehicle} blocker should be directly north on the same x"
            );
            let north_delta = vehicle_pos.1 - blocker_pos.1;
            assert!(
                (north_delta - config::TILE_SIZE as f32 * 3.0).abs() <= 0.001,
                "{vehicle} blocker should be three tiles north, got {north_delta:.2}px"
            );
        }
    }

    let center_vehicle = vehicle_positions[pair_count / 2];
    let goal_delta_y = center_vehicle.1 - setup.goal.1;
    assert!(
        (goal_delta_y - config::TILE_SIZE as f32 * 20.0).abs() <= 0.001,
        "{vehicle} move goal should be 20 tiles north of the formation center, delta {goal_delta_y:.2}"
    );
    assert!(
        (center_vehicle.0 - setup.goal.0).abs() <= 0.001,
        "{vehicle} move goal should stay on the formation center x axis"
    );
}

macro_rules! vehicle_small_block_baseline_test {
    ($name:ident, $vehicle:expr, $pair_count:expr) => {
        #[test]
        fn $name() {
            assert_vehicle_small_block_baseline_setup(
                $vehicle,
                $pair_count,
                Some(EntityKind::Worker),
            );
        }
    };
}

vehicle_small_block_baseline_test!(
    vehicle_small_block_baseline_scout_car_one_pair,
    EntityKind::ScoutCar,
    1
);
vehicle_small_block_baseline_test!(
    vehicle_small_block_baseline_scout_car_three_pairs,
    EntityKind::ScoutCar,
    3
);
vehicle_small_block_baseline_test!(
    vehicle_small_block_baseline_scout_car_five_pairs,
    EntityKind::ScoutCar,
    5
);
vehicle_small_block_baseline_test!(
    vehicle_small_block_baseline_tank_one_pair,
    EntityKind::Tank,
    1
);
vehicle_small_block_baseline_test!(
    vehicle_small_block_baseline_tank_three_pairs,
    EntityKind::Tank,
    3
);
vehicle_small_block_baseline_test!(
    vehicle_small_block_baseline_tank_five_pairs,
    EntityKind::Tank,
    5
);

#[test]
fn vehicle_small_block_baseline_supports_blocker_variants() {
    for blocker in [
        None,
        Some(EntityKind::Worker),
        Some(EntityKind::Rifleman),
        Some(EntityKind::MachineGunner),
        Some(EntityKind::AtTeam),
    ] {
        assert_vehicle_small_block_baseline_setup(EntityKind::ScoutCar, 3, blocker);
        assert_vehicle_small_block_baseline_setup(EntityKind::Tank, 3, blocker);
    }
}

#[test]
fn vehicle_small_block_baseline_clear_time_matrix() {
    let scenarios: Vec<_> = [EntityKind::ScoutCar, EntityKind::Tank]
        .into_iter()
        .flat_map(|vehicle| {
            [1usize, 3, 5].into_iter().flat_map(move |pair_count| {
                [
                    None,
                    Some(EntityKind::Worker),
                    Some(EntityKind::Rifleman),
                    Some(EntityKind::MachineGunner),
                    Some(EntityKind::AtTeam),
                ]
                .into_iter()
                .map(move |blocker| (vehicle, pair_count, blocker))
            })
        })
        .collect();
    let mut results: Vec<_> = scenarios
        .par_iter()
        .map(|&(vehicle, pair_count, blocker)| {
            measure_vehicle_small_block_clear_time(vehicle, pair_count, blocker)
        })
        .collect();
    results.sort_by(|a, b| {
        (
            a.vehicle.stable_id(),
            a.pair_count,
            blocker_label(a.blocker),
        )
            .cmp(&(
                b.vehicle.stable_id(),
                b.pair_count,
                blocker_label(b.blocker),
            ))
    });

    println!("VEHICLE_SMALL_BLOCK_BASELINE_CLEAR_TIMES");
    println!("vehicle | count | blocker | clear_ticks | clear_seconds | final_state");
    for result in &results {
        match (result.clear_ticks, result.clear_seconds) {
            (Some(ticks), Some(seconds)) => println!(
                "{:>9} | {:>5} | {:>14} | {:>11} | {:>13.2} | {:?}",
                result.vehicle,
                result.pair_count,
                blocker_label(result.blocker),
                ticks,
                seconds,
                result.final_state
            ),
            _ => println!(
                "{:>9} | {:>5} | {:>14} | {:>11} | {:>13} | {:?}",
                result.vehicle,
                result.pair_count,
                blocker_label(result.blocker),
                "timeout",
                "timeout",
                result.final_state
            ),
        }
    }

    for result in &results {
        let baseline =
            vehicle_small_block_baseline(result.vehicle, result.pair_count, result.blocker);
        let clear_ticks = result.clear_ticks.unwrap_or_else(|| {
            panic!(
                "vehicle-small-block vehicle={} count={} blocker={} timed out; final_state={:?}",
                result.vehicle,
                result.pair_count,
                blocker_label(result.blocker),
                result.final_state
            )
        });
        assert!(
            clear_ticks.saturating_mul(10) <= baseline.saturating_mul(11),
            "vehicle-small-block vehicle={} count={} blocker={} regressed: {} ticks vs baseline {} (>10% slower); final_state={:?}",
            result.vehicle,
            result.pair_count,
            blocker_label(result.blocker),
            clear_ticks,
            baseline,
            result.final_state
        );
    }
}
