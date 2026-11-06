use super::*;
use rand::SeedableRng;
use rts_sim::game::PlayerInit;
use rts_sim::protocol::{terrain, MapInfo, PlayerStart, ResourceNode};

pub(super) fn cache_test_start_payload() -> StartPayload {
    StartPayload {
        player_id: 1,
        spectator: false,
        prediction_build_id: None,
        prediction_version: 0,
        match_run_id: None,
        capabilities: Default::default(),
        diagnostics: Default::default(),
        replay: None,
        lab: None,
        observer_view: None,
        tick: 0,
        map: MapInfo {
            width: 8,
            height: 8,
            tile_size: crate::config::TILE_SIZE,
            terrain: vec![terrain::GRASS; 8 * 8],
            elevation: vec![0; 8 * 8],
            sun: None,
            resources: vec![ResourceNode {
                id: 10,
                kind: rts_sim::protocol::kinds::STEEL.to_string(),
                x: crate::config::TILE_SIZE as f32 * 5.5,
                y: crate::config::TILE_SIZE as f32 * 1.5,
            }],
            doodads: Vec::new(),
            concealment_tiles: Vec::new(),
            no_vehicle_tiles: Vec::new(),
            no_building_tiles: Vec::new(),
            no_entrenchment_tiles: Vec::new(),
            damage_reduction_tiles: Vec::new(),
            slow_movement_tiles: Vec::new(),
        },
        players: vec![PlayerStart {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "P1".to_string(),
            color: "#111".to_string(),
            is_ai: true,
            start_tile_x: 1,
            start_tile_y: 1,
        }],
    }
}

pub(super) fn driver_test_players() -> Vec<PlayerInit> {
    vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "AI One".to_string(),
            color: "#111111".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "AI Two".to_string(),
            color: "#222222".to_string(),
            is_ai: true,
        },
    ]
}

#[test]
fn canonical_driver_selects_host_alive_policy_and_skips_eliminated_controllers() {
    let players = driver_test_players();
    let mut normal_game = Game::new_without_ai_controllers(&players, 7);
    let expected_normal = normal_game.alive_players();
    let mut controllers = vec![AiController::new(2), AiController::new(99)];
    let normal = CanonicalAiTickDriver::run_with_report(
        &mut normal_game,
        &mut controllers,
        AiAlivePolicy::Normal,
    );
    assert_eq!(normal.alive_player_ids, expected_normal);
    assert_eq!(normal.controllers[0].player_id, 2);
    assert!(matches!(
        normal.controllers[0].invocation,
        AiControllerTickInvocation::Invoked { .. }
    ));
    assert!(matches!(
        normal.controllers[1].invocation,
        AiControllerTickInvocation::SkippedDead
    ));

    let mut ai_only_game = Game::new_without_ai_controllers(&players, 7);
    let expected_primary_base = ai_only_game.primary_base_alive_players();
    let mut ai_only_controllers = vec![AiController::new(1), AiController::new(2)];
    let ai_only = CanonicalAiTickDriver::run_with_report(
        &mut ai_only_game,
        &mut ai_only_controllers,
        AiAlivePolicy::StartingPrimaryBase,
    );
    assert_eq!(ai_only.alive_player_ids, expected_primary_base);
}

#[test]
fn canonical_driver_preserves_controller_order_inputs_and_staggered_traces() {
    let players = driver_test_players();
    let mut game = Game::new_without_ai_controllers(&players, 11);
    let mut controllers = vec![AiController::new(2), AiController::new(1)];

    for tick in 0u32..10 {
        let before_one = game.snapshot_for(1);
        let before_two = game.snapshot_for(2);
        let report = CanonicalAiTickDriver::run_with_report(
            &mut game,
            &mut controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
        assert_eq!(
            report
                .controllers
                .iter()
                .map(|controller| controller.player_id)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );
        for controller in &report.controllers {
            let AiControllerTickInvocation::Invoked {
                snapshot,
                decision_trace,
                ..
            } = &controller.invocation
            else {
                panic!("starting controller unexpectedly skipped");
            };
            let expected_snapshot = if controller.player_id == 1 {
                &before_one
            } else {
                &before_two
            };
            assert_eq!(snapshot.as_ref(), expected_snapshot);
            assert_eq!(
                decision_trace.is_some(),
                tick.wrapping_add(controller.player_id)
                    .is_multiple_of(DECISION_INTERVAL)
            );
        }
        game.tick();
    }
}

#[test]
fn canonical_driver_report_capture_does_not_change_execution() {
    let players = driver_test_players();
    let mut normal_game = Game::new_without_ai_controllers(&players, 17);
    let mut reported_game = Game::new_without_ai_controllers(&players, 17);
    let mut normal_controllers = vec![AiController::new(1), AiController::new(2)];
    let mut reported_controllers = vec![AiController::new(1), AiController::new(2)];

    for _ in 0..12 {
        CanonicalAiTickDriver::run(
            &mut normal_game,
            &mut normal_controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
        CanonicalAiTickDriver::run_with_report(
            &mut reported_game,
            &mut reported_controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
        assert_eq!(normal_game.command_log(), reported_game.command_log());
        assert_eq!(normal_game.tick(), reported_game.tick());
        assert_eq!(
            normal_game.snapshot_full_for(1),
            reported_game.snapshot_full_for(1)
        );
    }
}

#[test]
fn canonical_prepared_driver_keeps_retreats_on_non_decision_ticks() {
    let players = driver_test_players();
    let game = Game::new_without_ai_controllers(&players, 13);
    let start = game.start_payload();
    let snapshot = game.snapshot_for(1);
    assert!(!snapshot
        .tick
        .wrapping_add(1)
        .is_multiple_of(DECISION_INTERVAL));
    let retreat = SimCommand::Move {
        units: vec![42],
        x: 64.0,
        y: 96.0,
        queued: false,
    };
    let mut controller = AiController::new(1);

    let (commands, trace) = CanonicalAiTickDriver::run_prepared_controller(
        &mut controller,
        AiThinkContext {
            known_losses: Default::default(),
            match_seed: 0,
            start: &start,
            snapshot: &snapshot,
            alive_player_ids: &[1, 2],
            retreat_commands: vec![retreat.clone()],
        },
    );

    assert_eq!(commands, vec![retreat]);
    assert_eq!(trace, None);
}

#[test]
fn custom_strategy_actions_follow_host_retreats_on_decision_ticks() {
    struct MoveStrategy;

    impl AiStrategy for MoveStrategy {
        fn initialize(&mut self, frame: &AiFrame) {
            assert!(WorldQueries::new(frame).base(1, 3).is_some());
            assert!(WorldQueries::new(frame).base(2, 3).is_some());
        }

        fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
            assert!(!frame.base_sites().is_empty());
            let site_resources: std::collections::BTreeSet<_> = frame
                .base_sites()
                .iter()
                .flat_map(|site| site.resource_ids.iter().copied())
                .collect();
            assert_eq!(
                site_resources,
                frame.resources().iter().map(|node| node.id).collect()
            );
            assert!(WorldQueries::new(frame)
                .known_base_depots(crate::sdk::AiBaseSiteId::Start(2))
                .is_empty());
            let group = crate::sdk::UnitGroup::new([7]).unwrap();
            actions.move_group(&group, 128.0, 160.0, false).unwrap();
        }
    }

    let players = driver_test_players();
    let game = Game::new_without_ai_controllers(&players, 13);
    let start = game.start_payload();
    let mut snapshot = game.snapshot_for(1);
    snapshot.tick = 8;
    let retreat = SimCommand::Move {
        units: vec![42],
        x: 64.0,
        y: 96.0,
        queued: false,
    };
    let mut controller = AiController::with_strategy(1, Box::new(MoveStrategy));

    let commands = controller.think(AiThinkContext {
        known_losses: Default::default(),
        match_seed: 0,
        start: &start,
        snapshot: &snapshot,
        alive_player_ids: &[1, 2],
        retreat_commands: vec![retreat.clone()],
    });

    assert_eq!(commands[0], retreat);
    assert_eq!(
        commands[1],
        SimCommand::Move {
            units: vec![7],
            x: 128.0,
            y: 160.0,
            queued: false,
        }
    );
}

#[test]
fn sdk_base_rankings_follow_ground_paths_and_mark_unreachable() {
    use crate::sdk::AiBaseSiteId::Start;
    struct InspectBases;
    impl AiStrategy for InspectBases {
        fn step(&mut self, frame: &AiFrame, _actions: &mut AiActions) {
            let world = WorldQueries::new(frame);
            let first = world.ranked_bases(1);
            // Start 2 is only two tiles away across water, but twelve tiles by ground.
            // Start 3 is four tiles straight down, so it must rank before Start 2.
            assert_eq!(
                first.iter().map(|base| base.site.id).collect::<Vec<_>>(),
                vec![Start(1), Start(3), Start(2), Start(4)]
            );
            let tile_size = frame.map().tile_size as f32;
            assert_eq!(first[0].distance_from_main, Some(0.0));
            assert_eq!(first[1].distance_from_main, Some(4.0 * tile_size));
            // Twelve, rather than a shorter diagonal route, proves no blocked-corner cuts.
            assert_eq!(first[2].distance_from_main, Some(12.0 * tile_size));
            assert_eq!(first[3].distance_from_main, None);
            assert_eq!(world.base(2, 2).unwrap().site.id, Start(3));
            assert_eq!(world.base(2, 3).unwrap().site.id, Start(1));
            let isolated = world.ranked_bases(4);
            assert_eq!(isolated[0].site.id, Start(4));
            assert_eq!(isolated[0].distance_from_main, Some(0.0));
            assert_eq!(
                isolated[1..]
                    .iter()
                    .map(|base| base.site.id)
                    .collect::<Vec<_>>(),
                vec![Start(1), Start(2), Start(3)]
            );
            assert!(isolated[1..]
                .iter()
                .all(|base| base.distance_from_main.is_none()));
        }
    }

    let mut start = cache_test_start_payload();
    start.map.resources.clear();
    // A partial water wall requires a detour; a full wall isolates the fourth start.
    for y in 0..6 {
        start.map.terrain[y * 8 + 2] = terrain::WATER;
    }
    for y in 0..8 {
        start.map.terrain[y * 8 + 6] = terrain::WATER;
    }
    let template = start.players[0].clone();
    start.players = [(1, 1, 1), (2, 3, 1), (3, 1, 5), (4, 7, 1)]
        .into_iter()
        .map(|(id, x, y)| {
            let mut player = template.clone();
            player.id = id;
            player.team_id = id;
            player.start_tile_x = x;
            player.start_tile_y = y;
            player
        })
        .collect();
    let game = Game::new_without_ai_controllers(&driver_test_players(), 13);
    let mut snapshot = game.snapshot_for(1);
    snapshot.tick = 8;
    let mut controller = AiController::with_strategy(1, Box::new(InspectBases));
    let context = AiThinkContext {
        known_losses: Default::default(),
        match_seed: 0,
        start: &start,
        snapshot: &snapshot,
        alive_player_ids: &[1, 2, 3, 4],
        retreat_commands: Vec::new(),
    };
    assert!(controller.think(context).is_empty());
}

#[test]
fn live_controller_uses_default_profile_id() {
    let ai = AiController::new(2);

    assert_eq!(ai.player_id(), 2);
    assert_eq!(ai.profile_id(), DEFAULT_LIVE_PROFILE_ID);
    assert_eq!(ai.latest_decision_trace(), None);
}

#[test]
fn live_decision_trace_snapshot_is_bounded() {
    let long_line = "x".repeat(LIVE_DECISION_TRACE_MAX_LINE_CHARS + 8);
    let lines = std::iter::once(long_line)
        .chain((1..(LIVE_DECISION_TRACE_MAX_LINES + 3)).map(|index| format!("line={index}")))
        .collect();

    let bounded = bounded_decision_trace_lines(lines);

    assert_eq!(bounded.len(), LIVE_DECISION_TRACE_MAX_LINES);
    assert_eq!(bounded[0].len(), LIVE_DECISION_TRACE_MAX_LINE_CHARS);
    assert_eq!(
        bounded.last().map(String::as_str),
        Some(LIVE_DECISION_TRACE_TRUNCATED_LINE)
    );
}

#[test]
fn live_stage_filter_sends_hold_position_once_per_staged_unit() {
    let mut ai = AiController::new(1);
    let intents = [crate::ai_core::decision::AiIntent::Stage { units: vec![42] }];
    let hold = SimCommand::HoldPosition {
        units: vec![42],
        queued: false,
    };

    let first = ai.filter_repeated_stage_commands(10, &intents, vec![hold.clone()]);
    let second = ai.filter_repeated_stage_commands(16, &intents, vec![hold]);

    assert_eq!(
        first,
        vec![SimCommand::HoldPosition {
            units: vec![42],
            queued: false,
        }]
    );
    assert!(second.is_empty());
}

#[test]
fn live_stage_filter_keeps_clearance_move_for_already_staged_unit() {
    let mut ai = AiController::new(1);
    let stage_intent = [crate::ai_core::decision::AiIntent::Stage { units: vec![42] }];
    let initial = SimCommand::Move {
        units: vec![42],
        x: 100.0,
        y: 100.0,
        queued: false,
    };
    assert_eq!(
        ai.filter_repeated_stage_commands(10, &stage_intent, vec![initial.clone()]),
        vec![initial]
    );

    let clearance_intent = [crate::ai_core::decision::AiIntent::Move { units: vec![42] }];
    let clearance = SimCommand::Move {
        units: vec![42],
        x: 228.0,
        y: 100.0,
        queued: false,
    };
    assert_eq!(
        ai.filter_repeated_stage_commands(19, &clearance_intent, vec![clearance.clone()]),
        vec![clearance]
    );
}

#[test]
fn assembly_supersedes_staging_and_recent_attack_suppression() {
    use crate::ai_core::decision::AiIntent;
    let mut ai = AiController::with_profile_id(1, JEFFS_AI_ID);
    let stage = [AiIntent::Stage {
        units: vec![42, 43, 99],
    }];
    let initial = SimCommand::Move {
        units: vec![42, 43, 99],
        x: 100.0,
        y: 100.0,
        queued: false,
    };
    assert_eq!(
        ai.filter_repeated_stage_commands(10, &stage, vec![initial.clone()]),
        vec![initial.clone()]
    );
    // Ordinary staging still suppresses repeats, even with a changed destination.
    let regroup = SimCommand::Move {
        units: vec![42, 43],
        x: 228.0,
        y: 196.0,
        queued: false,
    };
    assert!(ai
        .filter_repeated_stage_commands(19, &stage, vec![regroup.clone()])
        .is_empty());
    ai.filter_repeated_stage_commands(20, &[AiIntent::Attack { units: vec![43] }], vec![]);
    let assembly = [
        AiIntent::Assemble {
            units: vec![42, 43],
        },
        AiIntent::Stage {
            units: vec![42, 43, 99],
        },
    ];
    let hold = SimCommand::HoldPosition {
        units: vec![42, 43],
        queued: false,
    };
    let earlier_stage = SimCommand::Move {
        units: vec![42, 43],
        x: 100.0,
        y: 100.0,
        queued: false,
    };
    // Home staging may precede cohort selection on the first assembly decision. It must not
    // swallow the later assembly destination; the unrelated staged defender remains filtered.
    assert_eq!(
        ai.filter_repeated_stage_commands(
            28,
            &assembly,
            vec![initial, regroup.clone(), hold.clone()]
        ),
        vec![earlier_stage, regroup.clone(), hold.clone()],
    );
    // A bounded planner retry must still pass if units failed to reach the slot.
    assert_eq!(
        ai.filter_repeated_stage_commands(88, &assembly, vec![regroup.clone(), hold.clone()]),
        vec![regroup.clone(), hold]
    );
    // Leaving assembly must allow an ordinary defensive stage to retask the cohort.
    assert_eq!(
        ai.filter_repeated_stage_commands(97, &stage, vec![regroup.clone()]),
        vec![regroup]
    );
}

#[test]
fn assembly_without_other_staging_preserves_moves_and_escort_attack_moves() {
    use crate::ai_core::decision::AiIntent;
    let mut ai = AiController::with_profile_id(1, JEFFS_AI_ID);
    ai.staged_units.extend([42, 43]);
    ai.held_stage_units.insert(42);
    ai.active_attack_units.insert(43, 10);
    let intents = [AiIntent::Assemble {
        units: vec![42, 43],
    }];
    let commands = vec![
        SimCommand::Move {
            units: vec![42],
            x: 228.0,
            y: 196.0,
            queued: false,
        },
        SimCommand::AttackMove {
            units: vec![43],
            x: 260.0,
            y: 196.0,
            queued: false,
        },
    ];
    assert_eq!(
        ai.filter_repeated_stage_commands(19, &intents, commands.clone()),
        commands
    );
    assert!(!ai.staged_units.contains(&42));
    assert!(!ai.held_stage_units.contains(&42));
    assert!(!ai.active_attack_units.contains_key(&43));
}

#[test]
fn live_adapter_knows_public_and_internal_profiles() {
    assert_eq!(
        LIVE_PROFILE_IDS,
        [
            AI_2_3_ID,
            AI_2_1_ID,
            JEFFS_AI_ID,
            JEFFS_AI_V1_1_ID,
            AI_TURTLE_ID,
            RUSH_AI_ID,
            PRESSURE_AI_ID
        ]
    );
}

#[test]
fn live_default_is_ai_2_3() {
    assert_eq!(DEFAULT_LIVE_PROFILE_ID, AI_2_3_ID);
}

#[test]
#[cfg(target_pointer_width = "64")]
fn random_live_profile_selection_preserves_historical_native_stream() {
    use rand::{rngs::SmallRng, RngCore};
    use rts_sim::rng::PortableRng;

    // At this boundary, switching to u32 sampling changes both the selected
    // bucket and rejection sampling's stream consumption.
    let boundary = (u64::MAX / RANDOM_LIVE_PROFILE_IDS.len() as u64) + 1;
    let mut historical = rand::rngs::mock::StepRng::new(boundary, boundary.wrapping_neg());
    let mut candidate = historical.clone();
    let expected = RANDOM_LIVE_PROFILE_IDS[historical.gen_range(0..RANDOM_LIVE_PROFILE_IDS.len())];
    assert_eq!(random_live_profile_id(&mut candidate), expected);
    assert_eq!(candidate.next_u64(), historical.next_u64());

    for seed in 0..128 {
        let mut historical = SmallRng::seed_from_u64(seed);
        let mut portable = PortableRng::seed_from_u64(seed);
        for _ in 0..32 {
            let expected =
                RANDOM_LIVE_PROFILE_IDS[historical.gen_range(0..RANDOM_LIVE_PROFILE_IDS.len())];
            assert_eq!(
                random_live_profile_id(&mut portable),
                expected,
                "seed {seed}"
            );
            assert_eq!(portable.next_u64(), historical.next_u64(), "seed {seed}");
        }
    }
}

#[test]
fn random_live_profile_selection_uses_the_full_internal_pool() {
    let mut rng = rts_sim::rng::PortableRng::seed_from_u64(0xA1);
    let mut selected = BTreeSet::new();
    for _ in 0..32 {
        selected.insert(random_live_profile_id(&mut rng));
    }
    assert_eq!(
        selected,
        BTreeSet::from([
            AI_2_3_ID,
            AI_2_1_ID,
            JEFFS_AI_V1_1_ID,
            AI_TURTLE_ID,
            RUSH_AI_ID,
            PRESSURE_AI_ID
        ])
    );
}

#[test]
fn unknown_profile_id_falls_back_to_default_profile() {
    let ai = AiController::with_profile_id(2, "missing_profile");

    assert_eq!(ai.profile_id(), DEFAULT_LIVE_PROFILE_ID);
}

#[test]
fn live_profile_aliases_are_bounded_to_supported_profiles() {
    assert_eq!(
        canonical_live_profile_id("ai"),
        Some(DEFAULT_LIVE_PROFILE_ID)
    );
    assert_eq!(
        canonical_live_profile_id("default"),
        Some(DEFAULT_LIVE_PROFILE_ID)
    );
    assert_eq!(canonical_live_profile_id(AI_2_1_ID), Some(AI_2_1_ID));
    assert_eq!(canonical_live_profile_id(JEFFS_AI_ID), Some(JEFFS_AI_ID));
    assert_eq!(
        canonical_live_profile_id(JEFFS_AI_V1_1_ID),
        Some(JEFFS_AI_V1_1_ID)
    );
    assert_eq!(canonical_live_profile_id(AI_TURTLE_ID), Some(AI_TURTLE_ID));
    assert_eq!(canonical_live_profile_id("unsupported_profile"), None);
}

#[test]
fn live_profile_ids_resolve_to_their_canonical_match_profiles() {
    assert_eq!(resolve_live_profile_id_for_match(AI_2_1_ID), AI_2_1_ID);
    assert_eq!(resolve_live_profile_id_for_match(JEFFS_AI_ID), JEFFS_AI_ID);
    assert_eq!(
        resolve_live_profile_id_for_match(AI_TURTLE_ID),
        AI_TURTLE_ID
    );
}

#[test]
fn live_profile_labels_match_lobby_selector_names() {
    assert_eq!(live_profile_label(AI_2_1_ID), "AI 2.1");
    assert_eq!(live_profile_label(JEFFS_AI_ID), "Jeff's AI V1.0");
    assert_eq!(live_profile_label(JEFFS_AI_V1_1_ID), "Jeff's AI");
    assert_eq!(live_profile_label(AI_TURTLE_ID), "AI Turtle");
    assert_eq!(live_profile_label("default"), "AI 2.3");
    assert_eq!(live_profile_label("unsupported_profile"), "AI");
    assert_eq!(live_profile_label("unknown"), "AI");
}
