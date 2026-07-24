use super::*;
use crate::game::entity::Order;
use crate::rules::combat::WeaponKind;
use rayon::prelude::*;
use rts_rules::faction::DEFAULT_FACTION_ID;

fn owned_kind_count(game: &Game, owner: u32, kind: EntityKind) -> usize {
    game.state
        .entities
        .iter()
        .filter(|e| e.owner == owner && e.kind == kind)
        .count()
}

fn assert_units_do_not_intersect_buildings(game: &Game) {
    let buildings: Vec<_> = game
        .state
        .entities
        .iter()
        .filter_map(|entity| {
            crate::game::services::geometry::building_rect_for_entity(&game.state.map, entity)
                .map(|rect| (entity.id, entity.kind, rect))
        })
        .collect();
    for unit in game.state.entities.iter().filter(|entity| entity.is_unit()) {
        let Some(body) = crate::game::services::geometry::unit_body_for_entity(unit) else {
            continue;
        };
        for &(building_id, building_kind, rect) in &buildings {
            assert!(
                !crate::game::services::geometry::unit_body_intersects_rect(body, rect),
                "scenario unit {} ({}) intersects building {} ({})",
                unit.id,
                unit.kind,
                building_id,
                building_kind
            );
        }
    }
}

fn assert_enemy_units_are_static_inspection_targets(game: &Game) {
    for entity in game
        .state
        .entities
        .iter()
        .filter(|entity| entity.owner == 2 && entity.is_unit())
    {
        assert!(
            matches!(entity.order(), Order::HoldPosition),
            "scenario target {} ({}) should hold position for static inspection",
            entity.id,
            entity.kind
        );
        if entity.can_attack() {
            for weapon in WeaponKind::ALL {
                assert_eq!(
                    entity.weapon_cooldown(weapon),
                    config::TICK_HZ * 120,
                    "scenario target {} ({}) should have delayed {weapon:?} fire",
                    entity.id,
                    entity.kind
                );
            }
        }
    }
}

fn assert_dev_scenario_starts_as_kriegsia(setup: &DevScenarioSetup) {
    let payload = setup.game.start_payload();
    let player = payload
        .players
        .iter()
        .find(|player| player.id == setup.player_id)
        .expect("scenario player should be present in start payload");
    assert_eq!(player.faction_id, DEFAULT_FACTION_ID);
}

#[test]
fn dynamic_construction_path_block_repaths_around_new_building() {
    for scenario_case in ["head_on", "slight_angle", "major_angle"] {
        let setup = Game::new_dynamic_construction_path_block_scenario(
            Some(scenario_case),
            EntityKind::Worker,
            1,
            0x5150_0718,
        )
        .expect("scenario setup should succeed");
        let mut game = setup.game;
        let mover = setup.units[0];
        game.enqueue(
            setup.player_id,
            SimCommand::Move {
                units: setup.units,
                x: setup.goal.0,
                y: setup.goal.1,
                queued: false,
            },
        );

        let mut saw_construction = false;
        let mut saw_static_block = false;
        let mut saw_repath_request = false;
        let mut arrived = false;
        for _ in 0..900 {
            game.tick();
            saw_construction |= game
                .state
                .entities
                .iter()
                .any(|entity| entity.kind == EntityKind::Barracks);
            let mover_state = game
                .state
                .entities
                .get(mover)
                .expect("mover should remain alive");
            saw_static_block |= mover_state
                .movement
                .as_ref()
                .is_some_and(|movement| movement.static_blocked_ticks > 0);
            saw_repath_request |= mover_state.movement.as_ref().is_some_and(|movement| {
                movement.last_repath_tick > config::STATIC_BLOCKED_REPATH_TICKS as u32
            });
            let dx = mover_state.pos_x - setup.goal.0;
            let dy = mover_state.pos_y - setup.goal.1;
            if (dx * dx + dy * dy).sqrt() <= config::TILE_SIZE as f32 {
                arrived = true;
                break;
            }
        }

        assert!(
            saw_construction,
            "{scenario_case}: the second worker should start the Barracks"
        );
        assert!(
            saw_static_block,
            "{scenario_case}: the moving worker should encounter the Barracks on its stale route"
        );
        assert!(
            saw_repath_request,
            "{scenario_case}: the static obstruction should assign a fresh path"
        );
        assert!(
            arrived,
            "{scenario_case}: the moving worker should repath around the new Barracks"
        );
    }
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
    issue_delay_ticks: u32,
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
        Some(EntityKind::AntiTankGun) => "anti_tank_gun",
        Some(_) => "unsupported",
    }
}

const VEHICLE_SMALL_BLOCK_BASELINES: &[(EntityKind, usize, Option<EntityKind>, u32)] = &[
    (EntityKind::ScoutCar, 1, Some(EntityKind::AntiTankGun), 291),
    (
        EntityKind::ScoutCar,
        1,
        Some(EntityKind::MachineGunner),
        295,
    ),
    (EntityKind::ScoutCar, 1, None, 272),
    (EntityKind::ScoutCar, 1, Some(EntityKind::Rifleman), 272),
    (EntityKind::ScoutCar, 1, Some(EntityKind::Worker), 272),
    (EntityKind::ScoutCar, 3, Some(EntityKind::AntiTankGun), 326),
    (
        EntityKind::ScoutCar,
        3,
        Some(EntityKind::MachineGunner),
        338,
    ),
    (EntityKind::ScoutCar, 3, None, 298),
    (EntityKind::ScoutCar, 3, Some(EntityKind::Rifleman), 298),
    (EntityKind::ScoutCar, 3, Some(EntityKind::Worker), 298),
    // Five vehicles now approach as a compact two-rank blob. Braced blockers make that deliberate
    // merge slower than the old one-rank line, while soft/no-blocker variants remain faster.
    (EntityKind::ScoutCar, 5, Some(EntityKind::AntiTankGun), 431),
    (
        EntityKind::ScoutCar,
        5,
        Some(EntityKind::MachineGunner),
        429,
    ),
    (EntityKind::ScoutCar, 5, None, 333),
    (EntityKind::ScoutCar, 5, Some(EntityKind::Rifleman), 333),
    (EntityKind::ScoutCar, 5, Some(EntityKind::Worker), 333),
    (EntityKind::Tank, 1, Some(EntityKind::AntiTankGun), 330),
    (EntityKind::Tank, 1, Some(EntityKind::MachineGunner), 330),
    (EntityKind::Tank, 1, None, 320),
    (EntityKind::Tank, 1, Some(EntityKind::Rifleman), 320),
    (EntityKind::Tank, 1, Some(EntityKind::Worker), 320),
    (EntityKind::Tank, 3, Some(EntityKind::AntiTankGun), 466),
    (EntityKind::Tank, 3, Some(EntityKind::MachineGunner), 462),
    (EntityKind::Tank, 3, None, 446),
    (EntityKind::Tank, 3, Some(EntityKind::Rifleman), 446),
    (EntityKind::Tank, 3, Some(EntityKind::Worker), 446),
    (EntityKind::Tank, 5, Some(EntityKind::AntiTankGun), 546),
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

fn vehicle_small_block_vehicles_clear(game: &Game, units: &[u32]) -> bool {
    units.iter().all(|&id| {
        game.state
            .entities
            .get(id)
            .is_some_and(|e| e.move_phase().is_none() && e.path_is_empty())
    })
}

fn dev_scenario_units_clear(game: &Game, units: &[u32]) -> bool {
    units.iter().all(|&id| {
        game.state
            .entities
            .get(id)
            .is_some_and(|e| e.move_phase().is_none() && e.path_is_empty())
    })
}

fn describe_dev_scenario_state(game: &Game, units: &[u32]) -> Vec<String> {
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

fn measure_dev_scenario_clear_time(
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

    let max_ticks = 12_000u32;
    for _ in 0..max_ticks {
        game.tick();
        if dev_scenario_units_clear(&game, &units) {
            let ticks = game.tick_count().saturating_sub(issued_at);
            return DevScenarioTiming {
                scenario,
                unit,
                count,
                issue_delay_ticks: setup.issue_after_ticks,
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
        issue_delay_ticks: setup.issue_after_ticks,
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
    let setup =
        Game::new_vehicle_small_block_baseline_scenario(vehicle, pair_count, blocker, 0x5150_0004)
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
    for unit in [
        EntityKind::AntiTankGun,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
        let setup = Game::new_direct_reverse_order_scenario(unit, 1, 0x5150_0003)
            .expect("scenario setup should succeed");
        let unit_id = *setup.units.first().expect("scenario should spawn one unit");
        let entity = setup
            .game
            .state
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
fn dev_scenarios_default_to_kriegsia_start_faction() {
    let scenarios = [
        Game::new_snaking_corridor_scenario(EntityKind::ScoutCar, 1, 0x5150_030d),
        Game::new_direct_reverse_order_scenario(EntityKind::Tank, 1, 0x5150_030d),
        Game::new_replay_142_vehicle_lock_scenario(EntityKind::ScoutCar, 2, 0x5150_030d),
        Game::new_scout_car_wall_chokepoint_scenario(EntityKind::ScoutCar, 3, 0x5150_030d),
        Game::new_vehicle_corner_wall_scenario(EntityKind::Tank, 1, 0x5150_030d),
        Game::new_vehicle_small_block_baseline_scenario(
            EntityKind::ScoutCar,
            1,
            Some(EntityKind::Worker),
            0x5150_030d,
        ),
        Game::new_factory_zero_gap_perpendicular_scenario(EntityKind::Tank, 1, 0x5150_030d),
        Game::new_command_car_corner_scenario(EntityKind::CommandCar, 1, 0x5150_030d),
        Game::new_factory_wall_rally_spawn_scenario(EntityKind::Tank, 1, 0x5150_030d),
        Game::new_tank_trap_line_build_scenario(
            "tank_trap_line_horizontal",
            EntityKind::ScoutCar,
            1,
            0x5150_030d,
        ),
        Game::new_entrenchment_inspection_scenario(EntityKind::Rifleman, 1, 0x5150_030d),
        Game::new_tank_coax_inspection_scenario(EntityKind::Tank, 1, 0x5150_030d),
    ];

    for setup in scenarios {
        assert_dev_scenario_starts_as_kriegsia(&setup.expect("scenario setup should succeed"));
    }
}

#[test]
fn replay_142_vehicle_lock_scenario_clears_without_slow_overlap() {
    let setup = Game::new_replay_142_vehicle_lock_scenario(EntityKind::ScoutCar, 2, 0x5150_0142)
        .expect("replay-142 scenario setup should succeed");
    assert_eq!(setup.issue_after_ticks, config::TICK_HZ);
    assert_eq!(setup.units.len(), 5);
    assert_eq!(owned_kind_count(&setup.game, 1, EntityKind::CommandCar), 1);
    assert_eq!(owned_kind_count(&setup.game, 1, EntityKind::ScoutCar), 2);
    assert_eq!(owned_kind_count(&setup.game, 1, EntityKind::Tank), 1);
    assert_eq!(
        owned_kind_count(&setup.game, 1, EntityKind::MachineGunner),
        1
    );
    assert_eq!(owned_kind_count(&setup.game, 1, EntityKind::CityCentre), 1);

    let mut game = setup.game;
    while game.tick_count() < setup.issue_after_ticks {
        game.tick();
    }
    let command_car_id = setup.units[0];
    let colliding_scout_id = setup.units[2];
    game.enqueue(
        setup.player_id,
        SimCommand::Move {
            units: setup.units,
            x: setup.goal.0,
            y: setup.goal.1,
            queued: false,
        },
    );

    let mut saw_contact = false;
    let mut clear_tick = None;
    let mut final_separation = 0.0;
    for elapsed in 1..=180 {
        game.tick();
        let scout = game
            .state
            .entities
            .get(colliding_scout_id)
            .expect("colliding Scout Car should survive");
        let command_car = game
            .state
            .entities
            .get(command_car_id)
            .expect("Command Car should survive");
        let separation = (scout.pos_x - command_car.pos_x).hypot(scout.pos_y - command_car.pos_y);
        if separation < 30.0 {
            saw_contact = true;
        } else if saw_contact && separation >= 40.0 && clear_tick.is_none() {
            clear_tick = Some(elapsed);
        }
        final_separation = separation;
    }

    assert!(
        saw_contact,
        "replay-142 fixture should recreate vehicle contact"
    );
    let clear_tick = clear_tick.expect("replay-142 pair should separate after making contact");
    assert!(
        clear_tick <= 90,
        "replay-142 pair should clear within three seconds, cleared after {clear_tick} ticks"
    );
    assert!(
        final_separation >= 40.0,
        "replay-142 pair should remain separated, final separation was {final_separation:.2}px"
    );
}

#[test]
fn tank_coax_inspection_scenario_sets_up_static_mixed_targets() {
    let setup = Game::new_tank_coax_inspection_scenario(EntityKind::Tank, 1, 0x5150_0606)
        .expect("Tank coax inspection scenario setup should succeed");
    assert_eq!(setup.issue_after_ticks, u32::MAX);
    assert_eq!(setup.units.len(), 11);
    assert_eq!(owned_kind_count(&setup.game, 1, EntityKind::Tank), 1);
    assert_eq!(owned_kind_count(&setup.game, 2, EntityKind::Tank), 1);
    assert_eq!(owned_kind_count(&setup.game, 2, EntityKind::Worker), 1);
    assert_eq!(owned_kind_count(&setup.game, 2, EntityKind::Rifleman), 1);
    assert_eq!(
        owned_kind_count(&setup.game, 2, EntityKind::MachineGunner),
        1
    );
    assert_eq!(owned_kind_count(&setup.game, 2, EntityKind::ScoutCar), 1);
    assert_eq!(owned_kind_count(&setup.game, 2, EntityKind::Golem), 1);
    assert_eq!(owned_kind_count(&setup.game, 2, EntityKind::Ekat), 1);
    assert_eq!(owned_kind_count(&setup.game, 2, EntityKind::MortarTeam), 1);
    assert_eq!(owned_kind_count(&setup.game, 2, EntityKind::AntiTankGun), 1);
    assert_eq!(owned_kind_count(&setup.game, 2, EntityKind::Artillery), 1);
    assert_eq!(owned_kind_count(&setup.game, 2, EntityKind::Depot), 1);
    assert_eq!(owned_kind_count(&setup.game, 0, EntityKind::TankTrap), 1);
    assert_eq!(owned_kind_count(&setup.game, 0, EntityKind::Steel), 1);
    assert_eq!(owned_kind_count(&setup.game, 0, EntityKind::Oil), 1);
    assert_eq!(setup.game.state.smokes.iter().count(), 1);
    let tank_id = setup.units[0];
    let tank = setup
        .game
        .state
        .entities
        .get(tank_id)
        .expect("tank should exist");
    assert_eq!(tank.weapon_facing(), Some(0.0));
    assert_eq!(
        tank.weapon_cooldown(crate::rules::combat::WeaponKind::TankCannon),
        config::TICK_HZ * 4,
        "inspection scenario should delay cannon fire so coax feedback is easy to see"
    );
    assert_units_do_not_intersect_buildings(&setup.game);
    assert_enemy_units_are_static_inspection_targets(&setup.game);
    let tank_hp_before = tank.hp;
    let enemy_hp_total = |game: &Game| {
        game.state
            .entities
            .iter()
            .filter(|entity| entity.owner == 2 && entity.is_unit())
            .map(|entity| entity.hp)
            .sum::<u32>()
    };
    let enemy_hp_before = enemy_hp_total(&setup.game);
    let mut ticked = setup.game.clone();
    ticked.tick();
    let ticked_tank = ticked
        .state
        .entities
        .get(tank_id)
        .expect("tank should survive first tick");
    assert_eq!(
        ticked_tank.weapon_cooldown(WeaponKind::TankCannon),
        config::TICK_HZ * 4 - 1,
        "inspection scenario should keep the Tank cannon delayed on the first tick"
    );
    assert_eq!(
        ticked_tank.weapon_cooldown(WeaponKind::TankCoax),
        crate::rules::combat::weapon_profile(WeaponKind::TankCoax)
            .expect("Tank coax profile should exist")
            .cooldown,
        "inspection scenario should make the held Tank fire its coax immediately"
    );
    assert!(
        enemy_hp_total(&ticked) < enemy_hp_before,
        "inspection scenario should place at least one enemy target inside the coax arc"
    );
    assert_eq!(
        ticked_tank.hp, tank_hp_before,
        "static inspection targets should not fire back on the first tick"
    );
    assert_dev_scenario_starts_as_kriegsia(&setup);
}

#[test]
fn entrenchment_inspection_scenario_seeds_research_trenches_and_reuse_units() {
    let setup = Game::new_entrenchment_inspection_scenario(EntityKind::Rifleman, 1, 0x5150_0505)
        .expect("entrenchment inspection scenario setup should succeed");
    assert_eq!(setup.issue_after_ticks, u32::MAX);
    assert_eq!(setup.units.len(), 4);
    assert_eq!(
        owned_kind_count(&setup.game, 1, EntityKind::TrainingCentre),
        1
    );
    assert_eq!(owned_kind_count(&setup.game, 1, EntityKind::Rifleman), 2);
    assert_eq!(owned_kind_count(&setup.game, 1, EntityKind::Worker), 0);
    assert_eq!(
        owned_kind_count(&setup.game, 1, EntityKind::MachineGunner),
        1
    );
    assert_eq!(owned_kind_count(&setup.game, 2, EntityKind::Rifleman), 1);
    assert_eq!(setup.game.state.trenches.all().len(), 3);
    assert!(setup
        .game
        .state
        .players
        .iter()
        .find(|player| player.id == 1)
        .expect("scenario player should exist")
        .upgrades
        .contains(&upgrade::UpgradeKind::Entrenchment));
    assert!(!setup.game.snapshot_full_for(1).trenches.is_empty());
    assert_dev_scenario_starts_as_kriegsia(&setup);
}

#[test]
fn tank_trap_line_build_scenarios_start_with_builders_tech_and_test_units() {
    let scenarios = [
        Game::new_tank_trap_line_build_scenario(
            "tank_trap_line_horizontal",
            EntityKind::ScoutCar,
            1,
            0x5150_0011,
        ),
        Game::new_tank_trap_line_build_scenario(
            "tank_trap_line_vertical",
            EntityKind::Tank,
            1,
            0x5150_0012,
        ),
        Game::new_tank_trap_line_build_scenario(
            "tank_trap_line_diagonal",
            EntityKind::Tank,
            1,
            0x5150_0013,
        ),
    ];

    for setup in scenarios {
        let setup = setup.expect("Tank Trap line scenario setup should succeed");
        assert_eq!(setup.issue_after_ticks, config::TICK_HZ * 30);
        assert_eq!(setup.units.len(), 2);
        assert_eq!(owned_kind_count(&setup.game, 1, EntityKind::Worker), 3);
        assert_eq!(
            owned_kind_count(&setup.game, 1, EntityKind::TrainingCentre),
            1
        );
        assert_eq!(owned_kind_count(&setup.game, 1, EntityKind::Rifleman), 1);
        assert!(
            owned_kind_count(&setup.game, 1, EntityKind::ScoutCar) == 1
                || owned_kind_count(&setup.game, 1, EntityKind::Tank) == 1
        );
        let player = setup
            .game
            .state
            .players
            .iter()
            .find(|p| p.id == setup.player_id)
            .expect("scenario player should exist");
        assert_eq!((player.steel, player.oil), (1_000, 1_000));
        assert_dev_scenario_starts_as_kriegsia(&setup);
    }
}

#[test]
fn tank_trap_pathing_scenarios_spawn_prebuilt_walls_and_expected_units() {
    let scenarios = [
        (
            "friendly_vehicle_reroute",
            EntityKind::ScoutCar,
            7usize,
            7usize,
        ),
        ("enemy_vehicle_reroute", EntityKind::Tank, 7usize, 8usize),
        (
            "infantry_pass_through",
            EntityKind::MachineGunner,
            7usize,
            8usize,
        ),
        (
            "explicit_infantry_attack",
            EntityKind::Rifleman,
            1usize,
            2usize,
        ),
    ];

    for (scenario_id, unit, trap_count, expected_buildings) in scenarios {
        let setup = Game::new_tank_trap_pathing_scenario(scenario_id, unit, 1, 0x5150_0101)
            .expect("Tank Trap pathing scenario setup should succeed");
        assert_eq!(setup.issue_after_ticks, config::TICK_HZ);
        assert_eq!(setup.units.len(), 1);
        assert_eq!(owned_kind_count(&setup.game, 1, unit), 1);
        assert_eq!(
            setup
                .game
                .state
                .entities
                .iter()
                .filter(|entity| entity.kind == EntityKind::TankTrap)
                .count(),
            trap_count,
            "{scenario_id} should spawn a complete Tank Trap wall"
        );
        assert_eq!(
            setup
                .game
                .state
                .entities
                .iter()
                .filter(|entity| entity.is_building())
                .count(),
            expected_buildings,
            "{scenario_id} should only add the authored wall plus remote enemy base when needed"
        );
        assert_dev_scenario_starts_as_kriegsia(&setup);
    }
}

#[test]
fn tank_trap_friendly_reroute_wall_uses_neutral_blockers() {
    let setup = Game::new_tank_trap_pathing_scenario(
        "friendly_vehicle_reroute",
        EntityKind::Tank,
        1,
        0x5150_0102,
    )
    .expect("scenario setup should succeed");
    assert_eq!(setup.game.team_of_player(1), Some(1));
    assert_eq!(setup.game.team_of_player(2), Some(1));
    assert!(setup
        .game
        .state
        .entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::TankTrap)
        .all(|entity| entity.owner == 0));

    let occ =
        services::occupancy::Occupancy::build(&setup.game.state.map, &setup.game.state.entities);
    let trap = setup
        .game
        .state
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::TankTrap)
        .expect("scenario should spawn Tank Traps");
    let (tx, ty) = setup.game.state.map.tile_of(trap.pos_x, trap.pos_y);
    assert!(
        !occ.passable_for_kind(tx as i32, ty as i32, EntityKind::Tank),
        "neutral Tank Traps should remain physical vehicle-body blockers"
    );
    assert!(
        occ.passable_for_kind(tx as i32, ty as i32, EntityKind::Rifleman),
        "Tank Traps should remain infantry-passable"
    );
}

#[test]
fn tank_trap_enemy_reroute_scenario_closes_sparse_vehicle_gaps() {
    let setup = Game::new_tank_trap_pathing_scenario(
        "enemy_vehicle_reroute",
        EntityKind::ScoutCar,
        1,
        0x5150_0103,
    )
    .expect("scenario setup should succeed");
    assert_eq!(setup.game.team_of_player(1), Some(1));
    assert_eq!(setup.game.team_of_player(2), Some(2));
    assert_eq!(
        setup
            .game
            .state
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::TankTrap)
            .count(),
        7
    );

    let occ =
        services::occupancy::Occupancy::build(&setup.game.state.map, &setup.game.state.entities);
    let mut enemy_trap_tiles: Vec<_> = setup
        .game
        .state
        .entities
        .iter()
        .filter(|entity| entity.owner == 0 && entity.kind == EntityKind::TankTrap)
        .map(|entity| setup.game.state.map.tile_of(entity.pos_x, entity.pos_y))
        .collect();
    enemy_trap_tiles.sort_unstable();
    let (tx, ty) = enemy_trap_tiles
        .iter()
        .copied()
        .find(|&(tx, ty)| enemy_trap_tiles.contains(&(tx, ty + 2)))
        .expect("scenario should spawn an enemy Tank Trap pair with one gap tile");
    assert!(
        !occ.passable_for_kind(tx as i32, ty as i32, EntityKind::ScoutCar),
        "enemy Tank Traps should remain physical vehicle-body blockers"
    );
    assert!(
        !occ.passable_for_kind(tx as i32, ty as i32 + 1, EntityKind::ScoutCar),
        "sparse enemy Tank Trap pairs should still be physically too tight for vehicles"
    );
    assert!(
        occ.passable_for_kind(tx as i32, ty as i32 + 1, EntityKind::Rifleman),
        "closed Tank Trap pair gaps should remain infantry-passable"
    );
}

#[test]
fn tank_trap_infantry_move_orders_cross_enemy_wall_without_auto_attacks() {
    for unit in [
        EntityKind::Worker,
        EntityKind::Rifleman,
        EntityKind::MachineGunner,
    ] {
        let setup =
            Game::new_tank_trap_pathing_scenario("infantry_pass_through", unit, 1, 0x5150_0104)
                .expect("scenario setup should succeed");
        let mut game = setup.game;
        let unit_id = setup.units[0];
        let wall_x = game
            .state
            .entities
            .iter()
            .find(|entity| entity.owner == 0 && entity.kind == EntityKind::TankTrap)
            .expect("neutral trap should exist")
            .pos_x;
        game.enqueue(
            setup.player_id,
            SimCommand::Move {
                units: vec![unit_id],
                x: setup.goal.0,
                y: setup.goal.1,
                queued: false,
            },
        );

        let mut emitted_attack = false;
        for _ in 0..900 {
            let events = game.tick();
            emitted_attack |= events
                .iter()
                .flat_map(|(_, events)| events.iter())
                .any(|event| matches!(event, Event::Attack { from, .. } if *from == unit_id));
            if game
                .state
                .entities
                .get(unit_id)
                .is_some_and(|entity| entity.pos_x > wall_x + config::TILE_SIZE as f32)
            {
                break;
            }
        }

        let entity = game
            .state
            .entities
            .get(unit_id)
            .expect("unit should survive");
        assert!(
            entity.pos_x > wall_x + config::TILE_SIZE as f32,
            "{unit} should cross the enemy Tank Trap wall on a normal move order"
        );
        assert_eq!(entity.target_id(), None);
        assert!(
            !emitted_attack,
            "{unit} should not attack traps while moving"
        );
    }
}

#[test]
fn tank_trap_explicit_rifleman_attack_order_remains_valid() {
    let setup = Game::new_tank_trap_pathing_scenario(
        "explicit_infantry_attack",
        EntityKind::Rifleman,
        1,
        0x5150_0105,
    )
    .expect("scenario setup should succeed");
    let mut game = setup.game;
    let rifleman = setup.units[0];
    let trap = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 0 && entity.kind == EntityKind::TankTrap)
        .expect("neutral Tank Trap should exist")
        .id;

    game.enqueue(
        setup.player_id,
        SimCommand::Attack {
            units: vec![rifleman],
            target: trap,
            queued: false,
        },
    );

    let mut attacked = false;
    for _ in 0..300 {
        let events = game.tick();
        attacked |= events
            .iter()
            .flat_map(|(_, events)| events.iter())
            .any(|event| matches!(event, Event::Attack { from, to, .. } if *from == rifleman && *to == trap));
        if attacked {
            break;
        }
    }

    let entity = game
        .state
        .entities
        .get(rifleman)
        .expect("rifleman should survive");
    assert_eq!(entity.order().attack_target(), Some(trap));
    assert_eq!(entity.target_id(), Some(trap));
    assert!(
        attacked,
        "direct Rifleman attack orders against visible enemy Tank Traps should still produce attacks"
    );
}

#[test]
fn vehicle_corner_wall_scenario_matches_authored_layout() {
    let setup = Game::new_vehicle_corner_wall_scenario(EntityKind::Tank, 5, 0x5150_0005)
        .expect("scenario setup should succeed");
    assert_eq!(setup.units.len(), 5);
    assert_eq!(owned_kind_count(&setup.game, 1, EntityKind::Tank), 5);

    let map = &setup.game.state.map;
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
        .state
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
            .state
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
    for unit in [
        EntityKind::AntiTankGun,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
        for count in [1usize, 3, 5] {
            let setup = Game::new_vehicle_corner_wall_scenario(unit, count, 0x5150_0006)
                .expect("scenario setup should succeed");
            assert_eq!(setup.units.len(), count);
            assert_eq!(owned_kind_count(&setup.game, 1, unit), count);
        }
    }
}

#[test]
fn factory_zero_gap_perpendicular_scenario_matches_authored_layout() {
    for unit in [
        EntityKind::AntiTankGun,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
        let setup = Game::new_factory_zero_gap_perpendicular_scenario(unit, 1, 0x5150_0009)
            .expect("scenario setup should succeed");
        assert_eq!(setup.issue_after_ticks, config::TICK_HZ / 2);
        assert_eq!(setup.units.len(), 1);
        assert_eq!(owned_kind_count(&setup.game, 1, EntityKind::Factory), 1);
        assert_eq!(owned_kind_count(&setup.game, 1, unit), 1);

        let factory = setup
            .game
            .state
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Factory)
            .expect("factory should exist");
        let rect = services::geometry::building_rect_for_entity(&setup.game.state.map, factory)
            .expect("factory rect should exist");
        let unit_entity = setup
            .game
            .state
            .entities
            .get(setup.units[0])
            .expect("scenario unit should exist");
        let body = services::geometry::unit_body_for_entity(unit_entity)
            .expect("scenario unit should have a movement body");
        let body_aabb = body.aabb();
        let gap = body_aabb.min_x - rect.max_x;
        assert!(
            (0.0..=0.025).contains(&gap),
            "{unit} should start visually zero-gap against factory east side, got {gap:.4}px"
        );
        assert!(
            (unit_entity.pos_y - (rect.min_y + rect.max_y) * 0.5).abs() <= 0.001,
            "{unit} should be centered along the factory side"
        );
        assert!(
            (unit_entity.facing() + std::f32::consts::FRAC_PI_2).abs() <= 0.001,
            "{unit} should begin with its hull north/south"
        );
        assert!(
            (setup.goal.0 - unit_entity.pos_x - config::TILE_SIZE as f32 * 10.0).abs() <= 0.001,
            "{unit} move goal should be ten tiles east"
        );
        assert!(
            (setup.goal.1 - unit_entity.pos_y).abs() <= 0.001,
            "{unit} move goal should be perpendicular to the hull on the same y axis"
        );
    }
}

#[test]
fn experimental_direct_reverse_and_corner_wall_clear_time_matrix() {
    let mut results = Vec::new();
    for unit in [
        EntityKind::AntiTankGun,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
        let setup = Game::new_direct_reverse_order_scenario(unit, 1, 0x5150_0007)
            .expect("scenario setup should succeed");
        results.push(measure_dev_scenario_clear_time(
            "direct_reverse_order",
            unit,
            1,
            setup,
        ));
    }
    for unit in [
        EntityKind::AntiTankGun,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
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
    println!(
        "scenario | unit | count | issue_delay_ticks | clear_ticks | clear_seconds | final_state"
    );
    for result in &results {
        match (result.clear_ticks, result.clear_seconds) {
            (Some(ticks), Some(seconds)) => println!(
                "{:>24} | {:>14} | {:>5} | {:>17} | {:>11} | {:>13.2} | {:?}",
                result.scenario,
                result.unit,
                result.count,
                result.issue_delay_ticks,
                ticks,
                seconds,
                result.final_state
            ),
            _ => println!(
                "{:>24} | {:>14} | {:>5} | {:>17} | {:>11} | {:>13} | {:?}",
                result.scenario,
                result.unit,
                result.count,
                result.issue_delay_ticks,
                "timeout",
                "timeout",
                result.final_state
            ),
        }
    }
}

#[test]
fn experimental_factory_zero_gap_perpendicular_clear_time_matrix() {
    let mut results = Vec::new();
    for unit in [
        EntityKind::AntiTankGun,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
        let setup = Game::new_factory_zero_gap_perpendicular_scenario(unit, 1, 0x5150_0010)
            .expect("scenario setup should succeed");
        results.push(measure_dev_scenario_clear_time(
            "factory_zero_gap_perpendicular",
            unit,
            1,
            setup,
        ));
    }

    println!("FACTORY_ZERO_GAP_PERPENDICULAR_CLEAR_TIMES");
    println!(
        "scenario | unit | count | issue_delay_ticks | clear_ticks | clear_seconds | final_state"
    );
    for result in &results {
        match (result.clear_ticks, result.clear_seconds) {
            (Some(ticks), Some(seconds)) => println!(
                "{:>32} | {:>14} | {:>5} | {:>17} | {:>11} | {:>13.2} | {:?}",
                result.scenario,
                result.unit,
                result.count,
                result.issue_delay_ticks,
                ticks,
                seconds,
                result.final_state
            ),
            _ => println!(
                "{:>32} | {:>14} | {:>5} | {:>17} | {:>11} | {:>13} | {:?}",
                result.scenario,
                result.unit,
                result.count,
                result.issue_delay_ticks,
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
    let setup =
        Game::new_vehicle_small_block_baseline_scenario(vehicle, pair_count, blocker, 0x5150_0004)
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
        EntityKind::AntiTankGun,
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
                .state
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
        .state
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
        for (vehicle_pos, blocker_pos) in vehicle_positions.iter().zip(blocker_positions.iter()) {
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
        Some(EntityKind::AntiTankGun),
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
                    Some(EntityKind::AntiTankGun),
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
