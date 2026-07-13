use super::fixtures::*;
use super::*;

fn queued_move_fixture() -> (Game, u32, (f32, f32), (f32, f32), (f32, f32)) {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x5150_0001);
    for tile in &mut game.state.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }

    let start = game.state.map.tile_center(8, 8);
    let first = game.state.map.tile_center(10, 8);
    let second = game.state.map.tile_center(12, 8);
    let replacement = game.state.map.tile_center(8, 10);
    let unit = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .expect("rifleman should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    game.assert_invariants();

    (game, unit, first, second, replacement)
}

struct MixedQueuedFixture {
    game: Game,
    worker_builder: u32,
    worker_gatherer: u32,
    rifleman: u32,
    enemy: u32,
    node: u32,
    move_goal: (f32, f32),
    attack_move_goal: (f32, f32),
}

fn mixed_queued_fixture() -> MixedQueuedFixture {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Alpha".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Bravo".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game =
        Game::new_for_replay_with_starting_resources(&players, 5_000, 5_000, 0x5150_0601);
    for tile in &mut game.state.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }

    let (cc_x, cc_y) =
        services::occupancy::footprint_center(&game.state.map, EntityKind::CityCentre, 4, 4);
    game.state
        .entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("player city centre should spawn");
    let (enemy_cc_x, enemy_cc_y) =
        services::occupancy::footprint_center(&game.state.map, EntityKind::CityCentre, 24, 4);
    game.state
        .entities
        .spawn_building(2, EntityKind::CityCentre, enemy_cc_x, enemy_cc_y, true)
        .expect("enemy city centre should spawn");

    let node = game
        .state
        .entities
        .spawn_node(EntityKind::Steel, cc_x + 96.0, cc_y)
        .expect("resource node should spawn");
    let worker_builder = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, cc_x + 96.0, cc_y + 32.0)
        .expect("builder should spawn");
    let worker_gatherer = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, cc_x, cc_y + 96.0)
        .expect("gatherer should spawn");
    let rifleman = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, cc_x + 96.0, cc_y + 160.0)
        .expect("rifleman should spawn");
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, cc_x + 224.0, cc_y + 160.0)
        .expect("enemy should spawn");

    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    game.assert_invariants();

    MixedQueuedFixture {
        game,
        worker_builder,
        worker_gatherer,
        rifleman,
        enemy,
        node,
        move_goal: (cc_x + 128.0, cc_y + 160.0),
        attack_move_goal: (cc_x + 192.0, cc_y + 160.0),
    }
}

struct PhaseSixIntentFixture {
    game: Game,
    scout_a: u32,
    scout_b: u32,
    rifleman: u32,
    anti_tank_gun: u32,
    first_move: (f32, f32),
    second_move: (f32, f32),
    smoke_targets: [(f32, f32); 4],
    charge_goal: (f32, f32),
    attack_move_goal: (f32, f32),
    setup_facing: (f32, f32),
}

fn phase_six_intent_fixture() -> PhaseSixIntentFixture {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Alpha".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Bravo".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game =
        Game::new_for_replay_with_starting_resources(&players, 5_000, 5_000, 0x5150_0602);
    for tile in &mut game.state.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }

    let (steelworks_x, steelworks_y) =
        services::occupancy::footprint_center(&game.state.map, EntityKind::Steelworks, 4, 4);
    game.state
        .entities
        .spawn_building(1, EntityKind::Steelworks, steelworks_x, steelworks_y, true)
        .expect("steelworks should spawn");
    let (training_x, training_y) =
        services::occupancy::footprint_center(&game.state.map, EntityKind::TrainingCentre, 8, 4);
    game.state
        .entities
        .spawn_building(1, EntityKind::TrainingCentre, training_x, training_y, true)
        .expect("training centre should spawn");
    let scout_a_pos = game.state.map.tile_center(8, 10);
    let scout_b_pos = game.state.map.tile_center(8, 12);
    let scout_a = game
        .state
        .entities
        .spawn_unit(1, EntityKind::ScoutCar, scout_a_pos.0, scout_a_pos.1)
        .expect("first scout should spawn");
    let scout_b = game
        .state
        .entities
        .spawn_unit(1, EntityKind::ScoutCar, scout_b_pos.0, scout_b_pos.1)
        .expect("second scout should spawn");
    let rifle_pos = game.state.map.tile_center(9, 14);
    let rifleman = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, rifle_pos.0, rifle_pos.1)
        .expect("rifleman should spawn");
    let at_pos = game.state.map.tile_center(10, 14);
    let anti_tank_gun = game
        .state
        .entities
        .spawn_unit(1, EntityKind::AntiTankGun, at_pos.0, at_pos.1)
        .expect("Anti-Tank Gun should spawn");
    let enemy_pos = game.state.map.tile_center(18, 14);
    game.state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");

    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state.fog.recompute_with_smoke(
        &ids,
        &game.state.entities,
        &game.state.map,
        &game.state.smokes,
    );
    game.assert_invariants();

    let first_move = game.state.map.tile_center(12, 10);
    let second_move = game.state.map.tile_center(14, 12);
    let smoke_targets = [
        game.state.map.tile_center(13, 10),
        game.state.map.tile_center(13, 12),
        game.state.map.tile_center(14, 10),
        game.state.map.tile_center(14, 12),
    ];
    let charge_goal = game.state.map.tile_center(12, 14);
    let attack_move_goal = game.state.map.tile_center(16, 14);
    let setup_facing = game.state.map.tile_center(18, 14);

    PhaseSixIntentFixture {
        game,
        scout_a,
        scout_b,
        rifleman,
        anti_tank_gun,
        first_move,
        second_move,
        smoke_targets,
        charge_goal,
        attack_move_goal,
        setup_facing,
    }
}

#[test]
fn tank_move_command_preserves_exact_goal_and_repeats_deterministically() {
    let (mut live, tank, goal) = flat_tank_move_fixture();

    live.enqueue(
        1,
        Command::Move {
            units: vec![tank],
            x: goal.0,
            y: goal.1,
            queued: false,
        },
    );
    live.tick();

    assert_eq!(
        live.command_log(),
        &[super::replay::CommandLogEntry {
            tick: 1,
            player_id: 1,
            command: crate::protocol::Command::Move {
                units: vec![tank],
                x: goal.0,
                y: goal.1,
                queued: false,
            },
        }]
    );
    let moved_tank = live.state.entities.get(tank).expect("tank should exist");
    assert_eq!(moved_tank.path_goal(), Some(goal));
    let path = moved_tank
        .movement
        .as_ref()
        .expect("tank should have movement")
        .path
        .as_slice();
    assert_eq!(
        path.first().copied(),
        Some(goal),
        "reverse-ordered tank path should preserve the exact command goal"
    );
    assert!(
        path.len() > 1,
        "tank movement should keep clearance-shaped intermediate waypoints"
    );

    let (mut repeat_a, tank_a, goal_a) = flat_tank_move_fixture();
    let (mut repeat_b, tank_b, goal_b) = flat_tank_move_fixture();
    assert_eq!(tank_a, tank_b, "fixture entity ids should be reproducible");
    assert_eq!(goal_a, goal_b, "fixture goals should be reproducible");
    for game in [&mut repeat_a, &mut repeat_b] {
        game.enqueue(
            1,
            Command::Move {
                units: vec![tank_a],
                x: goal_a.0,
                y: goal_a.1,
                queued: false,
            },
        );
    }

    for _ in 0..120 {
        repeat_a.tick();
        repeat_b.tick();
    }

    let a = repeat_a
        .state
        .entities
        .get(tank_a)
        .expect("tank A should exist");
    let b = repeat_b
        .state
        .entities
        .get(tank_b)
        .expect("tank B should exist");
    assert_eq!(
        (a.pos_x, a.pos_y, a.facing()),
        (b.pos_x, b.pos_y, b.facing())
    );
    assert_eq!(a.path_goal(), b.path_goal());
    assert_eq!(
        a.movement.as_ref().map(|movement| movement.path.clone()),
        b.movement.as_ref().map(|movement| movement.path.clone())
    );
    assert_eq!(repeat_a.command_log(), repeat_b.command_log());
}

#[test]
fn queued_move_commands_follow_waypoints_in_order() {
    let (mut game, unit, first, second, _) = queued_move_fixture();

    game.enqueue(
        1,
        Command::Move {
            units: vec![unit],
            x: first.0,
            y: first.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Move {
            units: vec![unit],
            x: second.0,
            y: second.1,
            queued: true,
        },
    );
    game.tick();

    let entity = game.state.entities.get(unit).expect("unit should exist");
    assert_eq!(
        entity.move_intent(),
        Some(first),
        "idle unit should immediately promote the first queued move"
    );
    assert_eq!(entity.queued_orders().len(), 1);

    for _ in 0..120 {
        game.tick();
    }

    let entity = game.state.entities.get(unit).expect("unit should exist");
    assert!(
        entity_distance_to(&game, unit, second) <= 3.0,
        "unit should end at the second queued waypoint"
    );
    assert!(entity.queued_orders().is_empty());
    assert!(matches!(entity.order(), Order::Idle));
    assert_eq!(game.command_log().len(), 2);
    assert!(game.command_log().iter().all(|entry| {
        matches!(
            &entry.command,
            crate::protocol::Command::Move { queued: true, .. }
        )
    }));
}

#[test]
fn out_of_range_smoke_moves_into_range_launches_then_idles() {
    let (mut game, scout, target, _) = smoke_command_fixture();

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::Smoke,
            units: vec![scout],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
    game.tick();

    assert!(
        matches!(
            game.state.entities.get(scout).unwrap().order(),
            Order::Ability(_)
        ),
        "out-of-range Smoke should become an active ability movement order"
    );

    for _ in 0..240 {
        if game.state.smokes.iter().count() > 0 {
            break;
        }
        game.tick();
    }

    let scout_entity = game.state.entities.get(scout).expect("scout should exist");
    assert_eq!(
        game.state.smokes.iter().count(),
        1,
        "Smoke cloud should spawn once the scout car reaches launch range"
    );
    assert!(matches!(scout_entity.order(), Order::Idle));
    assert_eq!(
        scout_entity.ability_cooldown_ticks(ability::AbilityKind::Smoke),
        config::SMOKE_ABILITY_COOLDOWN_TICKS
            .saturating_sub(config::SMOKE_LAUNCH_MAX_DELAY_TICKS as u16)
    );
    assert_eq!(game.state.players[0].steel, 500);
    assert_eq!(game.state.players[0].oil, 500);
}

#[test]
fn queued_out_of_range_smoke_command_log_replays_deterministically() {
    let (mut live, scout, first_target, second_target) = smoke_command_fixture();

    live.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::Smoke,
            units: vec![scout],
            x: Some(first_target.0),
            y: Some(first_target.1),
            queued: true,
        },
    );
    live.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::Smoke,
            units: vec![scout],
            x: Some(second_target.0),
            y: Some(second_target.1),
            queued: true,
        },
    );

    let mut live_events = Vec::new();
    for tick in 1..=180 {
        for (player_id, events) in live.tick() {
            for event in events {
                live_events.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    assert!(
        live.command_log().iter().any(|entry| matches!(
            entry.command,
            crate::protocol::Command::UseAbility {
                ref ability,
                queued: true,
                ..
            } if ability == crate::protocol::abilities::SMOKE
        )),
        "command log should preserve queued Smoke intent"
    );

    let mut replay = smoke_command_fixture().0;
    let command_log = live.command_log().to_vec();
    let mut next_command = 0usize;
    let mut replay_events = Vec::new();
    for tick in 1..=live.tick_count() {
        while let Some(entry) = command_log.get(next_command) {
            if entry.tick != tick {
                break;
            }
            replay.enqueue(
                entry.player_id,
                Command::from_protocol(entry.command.clone()),
            );
            next_command += 1;
        }
        for (player_id, events) in replay.tick() {
            for event in events {
                replay_events.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    assert_eq!(next_command, command_log.len());
    assert_eq!(live_events, replay_events);
    assert_eq!(live.snapshot_for(1), replay.snapshot_for(1));
}

#[test]
fn mixed_queued_command_log_replays_deterministically() {
    let MixedQueuedFixture {
        mut game,
        worker_builder,
        worker_gatherer,
        rifleman,
        enemy,
        node,
        move_goal,
        attack_move_goal,
    } = mixed_queued_fixture();

    game.enqueue(
        1,
        Command::Move {
            units: vec![rifleman],
            x: move_goal.0,
            y: move_goal.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![rifleman],
            x: attack_move_goal.0,
            y: attack_move_goal.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Attack {
            units: vec![rifleman],
            target: enemy,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Gather {
            units: vec![worker_gatherer],
            node,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Build {
            units: vec![worker_builder],
            building: EntityKind::CityCentre,
            tile_x: 12,
            tile_y: 12,
            queued: true,
        },
    );

    let mut live_events = Vec::new();
    for tick in 1..=180 {
        for (player_id, events) in game.tick() {
            for event in events {
                live_events.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    let command_log = game.command_log().to_vec();
    assert!(
        command_log.iter().any(|entry| matches!(
            entry.command,
            crate::protocol::Command::Attack { queued: true, .. }
        )),
        "command log should preserve queued mixed attack intent"
    );

    let mut replay = mixed_queued_fixture().game;
    let mut next_command = 0usize;
    let mut replay_events = Vec::new();
    for tick in 1..=game.tick_count() {
        while let Some(entry) = command_log.get(next_command) {
            if entry.tick != tick {
                break;
            }
            replay.enqueue(
                entry.player_id,
                Command::from_protocol(entry.command.clone()),
            );
            next_command += 1;
        }
        for (player_id, events) in replay.tick() {
            for event in events {
                replay_events.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    assert_eq!(next_command, command_log.len());
    assert_eq!(live_events, replay_events);
    assert_eq!(game.snapshot_for(1), replay.snapshot_for(1));
    assert_eq!(game.snapshot_for(2), replay.snapshot_for(2));
}

#[test]
fn phase_six_mixed_intent_command_log_replays_deterministically() {
    let PhaseSixIntentFixture {
        mut game,
        scout_a,
        scout_b,
        rifleman,
        anti_tank_gun,
        first_move,
        second_move,
        smoke_targets,
        charge_goal,
        attack_move_goal,
        setup_facing,
    } = phase_six_intent_fixture();

    game.enqueue(
        1,
        Command::Move {
            units: vec![scout_a, scout_b],
            x: first_move.0,
            y: first_move.1,
            queued: false,
        },
    );
    for target in smoke_targets {
        game.enqueue(
            1,
            Command::UseAbility {
                ability: ability::AbilityKind::Smoke,
                units: vec![scout_a, scout_b],
                x: Some(target.0),
                y: Some(target.1),
                queued: true,
            },
        );
    }
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![scout_a, scout_b],
            x: second_move.0,
            y: second_move.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Move {
            units: vec![rifleman],
            x: charge_goal.0,
            y: charge_goal.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::Charge,
            units: vec![rifleman],
            x: None,
            y: None,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![rifleman],
            x: attack_move_goal.0,
            y: attack_move_goal.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Move {
            units: vec![anti_tank_gun],
            x: charge_goal.0,
            y: charge_goal.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::SetupAntiTankGuns {
            units: vec![anti_tank_gun],
            x: setup_facing.0,
            y: setup_facing.1,
            queued: true,
        },
    );

    let mut live_events = Vec::new();
    for tick in 1..=260 {
        for (player_id, events) in game.tick() {
            for event in events {
                live_events.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    let command_log = game.command_log().to_vec();
    assert_eq!(
        command_log
            .iter()
            .filter(|entry| matches!(
                entry.command,
                crate::protocol::Command::UseAbility {
                    ref ability,
                    queued: true,
                    ..
                } if ability == crate::protocol::abilities::SMOKE
            ))
            .count(),
        4,
        "command log should preserve all queued Smoke intents"
    );
    assert!(command_log.iter().any(|entry| matches!(
        entry.command,
        crate::protocol::Command::UseAbility {
            ref ability,
            queued: true,
            ..
        } if ability == crate::protocol::abilities::CHARGE
    )));
    assert!(command_log.iter().any(|entry| matches!(
        entry.command,
        crate::protocol::Command::SetupAntiTankGuns { queued: true, .. }
    )));

    let mut replay = phase_six_intent_fixture().game;
    let mut next_command = 0usize;
    let mut replay_events = Vec::new();
    for tick in 1..=game.tick_count() {
        while let Some(entry) = command_log.get(next_command) {
            if entry.tick != tick {
                break;
            }
            replay.enqueue(
                entry.player_id,
                Command::from_protocol(entry.command.clone()),
            );
            next_command += 1;
        }
        for (player_id, events) in replay.tick() {
            for event in events {
                replay_events.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    assert_eq!(next_command, command_log.len());
    assert_eq!(live_events, replay_events);
    assert_eq!(game.snapshot_for(1), replay.snapshot_for(1));
    assert_eq!(game.snapshot_for(2), replay.snapshot_for(2));
}

#[test]
fn normal_move_then_queued_move_snapshot_shows_active_and_future_waypoints() {
    let (mut game, unit, first, second, _) = queued_move_fixture();

    game.enqueue(
        1,
        Command::Move {
            units: vec![unit],
            x: first.0,
            y: first.1,
            queued: false,
        },
    );
    game.tick();
    game.enqueue(
        1,
        Command::Move {
            units: vec![unit],
            x: second.0,
            y: second.1,
            queued: true,
        },
    );
    game.tick();

    let view = game
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|entity| entity.id == unit)
        .expect("selected unit should be visible to owner");
    assert_eq!(
        view.order_plan,
        vec![
            crate::protocol::OrderPlanMarker {
                kind: "move".to_string(),
                x: first.0,
                y: first.1,
            },
            crate::protocol::OrderPlanMarker {
                kind: "move".to_string(),
                x: second.0,
                y: second.1,
            },
        ]
    );
}

#[test]
fn snapshot_options_control_runtime_movement_debug_path() {
    for (options, expected_debug_path) in [
        (SnapshotOptions::default(), false),
        (
            SnapshotOptions {
                include_movement_paths: true,
                movement_paths_for_all_projected: false,
            },
            true,
        ),
    ] {
        let (mut game, unit, first, _, _) = queued_move_fixture();

        game.enqueue(
            1,
            Command::Move {
                units: vec![unit],
                x: first.0,
                y: first.1,
                queued: false,
            },
        );
        game.tick();

        let view = game
            .snapshot_for_with_options(1, options)
            .entities
            .into_iter()
            .find(|entity| entity.id == unit)
            .expect("selected unit should be visible to owner");
        assert_eq!(
            view.debug_path.is_some(),
            expected_debug_path,
            "debug path visibility should follow neutral snapshot options"
        );
        if let Some(debug_path) = view.debug_path {
            assert_eq!(
                debug_path.waypoints.first().map(|point| (point.x, point.y)),
                game.state
                    .entities
                    .get(unit)
                    .and_then(|entity| entity.next_waypoint())
            );
        }
    }
}

#[test]
fn dev_scenario_snapshot_shows_runtime_movement_debug_path() {
    let setup = Game::new_snaking_corridor_scenario(EntityKind::ScoutCar, 1, 0x5150_0002)
        .expect("scenario setup should succeed");
    let mut game = setup.game;
    let unit = setup.units[0];
    game.enqueue(
        setup.player_id,
        Command::Move {
            units: vec![unit],
            x: setup.goal.0,
            y: setup.goal.1,
            queued: false,
        },
    );
    game.tick();

    let view = game
        .snapshot_full_for_with_options(
            setup.player_id,
            SnapshotOptions {
                include_movement_paths: true,
                movement_paths_for_all_projected: true,
            },
        )
        .entities
        .into_iter()
        .find(|entity| entity.id == unit)
        .expect("scenario unit should be visible to owner");
    assert!(
        view.debug_path.is_some(),
        "dev scenario snapshots should include movement debug paths"
    );
}

#[test]
fn wall_chokepoint_dev_scenario_matches_authored_layout() {
    let setup = Game::new_scout_car_wall_chokepoint_scenario(EntityKind::ScoutCar, 15, 0x5150_0003)
        .expect("scenario setup should succeed");
    let mut game = setup.game;

    assert_eq!(setup.units.len(), 15);
    let wall_y = game.state.map.size - 18;
    let gap_left_x = game.state.map.size / 2 - 1;
    let gap_right_x = game.state.map.size / 2;
    assert_eq!(
        game.state.map.terrain[game.state.map.index(gap_left_x, wall_y)],
        terrain::GRASS
    );
    assert_eq!(
        game.state.map.terrain[game.state.map.index(gap_right_x, wall_y)],
        terrain::GRASS
    );
    assert_eq!(
        game.state.map.terrain[game.state.map.index(gap_left_x - 1, wall_y)],
        terrain::ROCK
    );
    assert_eq!(
        game.state.map.terrain[game.state.map.index(gap_right_x + 1, wall_y)],
        terrain::ROCK
    );

    let start_y = (wall_y + 10) as f32 * config::TILE_SIZE as f32 + config::TILE_SIZE as f32 * 0.5;
    let north = -std::f32::consts::FRAC_PI_2;
    for unit in &setup.units {
        let entity = game
            .state
            .entities
            .get(*unit)
            .expect("scenario unit exists");
        assert_eq!(entity.kind, EntityKind::ScoutCar);
        assert!((entity.pos_y - start_y).abs() < 0.1);
        assert!((entity.facing() - north).abs() < 0.001);
    }

    let command_units: Vec<u32> = setup.units.iter().copied().take(8).collect();
    game.enqueue(
        setup.player_id,
        Command::Move {
            units: command_units.clone(),
            x: setup.goal.0,
            y: setup.goal.1,
            queued: false,
        },
    );
    // The pathing work fuse may deliberately spread obstructed vehicle searches across ticks.
    for _ in 0..command_units.len() {
        game.tick();
    }
    let debug_options = SnapshotOptions {
        include_movement_paths: true,
        movement_paths_for_all_projected: false,
    };
    for unit in command_units {
        let view = game
            .snapshot_for_with_options(setup.player_id, debug_options)
            .entities
            .into_iter()
            .find(|entity| entity.id == unit)
            .expect("scenario scout car should be visible to owner");
        assert!(
            view.debug_path.is_some(),
            "wall chokepoint scenario should issue movement debug paths"
        );
    }
}

#[test]
fn wall_chokepoint_dev_scenario_supports_all_vehicles() {
    for unit in [
        EntityKind::AntiTankGun,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
        let setup = Game::new_scout_car_wall_chokepoint_scenario(unit, 5, 0x5150_0004)
            .expect("scenario setup should succeed");

        assert_eq!(setup.units.len(), 5);
        for unit_id in setup.units {
            let entity = setup
                .game
                .state
                .entities
                .get(unit_id)
                .expect("scenario unit exists");
            assert_eq!(entity.kind, unit);
        }
    }
}

#[test]
fn replacement_move_and_stop_clear_queued_movement() {
    let (mut game, unit, first, second, replacement) = queued_move_fixture();

    for goal in [first, second] {
        game.enqueue(
            1,
            Command::Move {
                units: vec![unit],
                x: goal.0,
                y: goal.1,
                queued: true,
            },
        );
    }
    game.tick();
    game.enqueue(
        1,
        Command::Move {
            units: vec![unit],
            x: replacement.0,
            y: replacement.1,
            queued: false,
        },
    );
    game.tick();

    let entity = game.state.entities.get(unit).expect("unit should exist");
    assert_eq!(entity.move_intent(), Some(replacement));
    assert!(entity.queued_orders().is_empty());

    game.enqueue(1, Command::Stop { units: vec![unit] });
    game.tick();

    let entity = game.state.entities.get(unit).expect("unit should exist");
    assert!(matches!(entity.order(), Order::Idle));
    assert!(entity.queued_orders().is_empty());
    assert!(entity.path_is_empty());
}
