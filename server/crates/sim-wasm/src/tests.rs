use super::*;
use rts_contract::{PlayerStart, SnapshotNetStatus};

fn start_payload() -> StartPayload {
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
        tick: 10,
        map: MapInfo {
            width: 64,
            height: 64,
            tile_size: balance::TILE_SIZE,
            terrain: vec![0; 64 * 64],
            resources: Vec::new(),
            doodads: Vec::new(),
            concealment_tiles: Vec::new(),
            no_vehicle_tiles: Vec::new(),
            damage_reduction_tiles: Vec::new(),
            slow_movement_tiles: Vec::new(),
        },
        players: vec![
            PlayerStart {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "A".to_string(),
                color: "#f00".to_string(),
                is_ai: false,
                start_tile_x: 5,
                start_tile_y: 5,
            },
            PlayerStart {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "B".to_string(),
                color: "#00f".to_string(),
                is_ai: false,
                start_tile_x: 50,
                start_tile_y: 50,
            },
        ],
    }
}

fn snapshot() -> Snapshot {
    let mut owned = EntityView::new(101, 1, "worker", 100.0, 100.0, 40, 40, "move");
    owned.order_plan = vec![OrderPlanMarker {
        kind: "move".to_string(),
        x: 120.0,
        y: 100.0,
    }];
    let mut hidden_shape = EntityView::new(202, 2, "rifleman", 500.0, 500.0, 45, 45, "attack");
    hidden_shape.target_id = Some(101);
    hidden_shape.order_plan = vec![OrderPlanMarker {
        kind: "attack".to_string(),
        x: 100.0,
        y: 100.0,
    }];
    let neutral_tank_trap = EntityView::new(303, 0, "tank_trap", 300.0, 300.0, 100, 100, "idle");
    Snapshot {
        tick: 10,
        ground_decal_revision: 0,
        ground_decal_delta: None,
        world_combat_position: None,
        steel: 75,
        oil: 0,
        supply_used: 1,
        supply_cap: 10,
        auto_build: None,
        entities: vec![owned, hidden_shape, neutral_tank_trap],
        resource_deltas: Vec::new(),
        smokes: Vec::new(),
        ability_objects: Vec::new(),
        trenches: Vec::new(),
        visible_tiles: vec![0; 64 * 64],
        explored_tiles: vec![0; 64 * 64],
        remembered_buildings: Vec::new(),
        remembered_anti_tank_guns: Vec::new(),
        events: Vec::new(),
        upgrades: Vec::new(),
        player_resources: vec![],
        net_status: SnapshotNetStatus::default(),
    }
}

fn construction_view(id: u32, owner: u32, fraction: f32, active: bool) -> EntityView {
    let mut entity = EntityView::new(id, owner, "barracks", 200.0, 200.0, 100, 500, "construct");
    entity.build_progress = Some(fraction);
    entity.build_active = active;
    entity
}

fn production_view(id: u32, owner: u32, fraction: f32) -> EntityView {
    let mut entity = EntityView::new(
        id,
        owner,
        "resource_depot",
        300.0,
        300.0,
        1000,
        1000,
        "idle",
    );
    entity.prod_kind = Some("worker".to_string());
    entity.prod_progress = Some(fraction);
    entity.prod_queue = Some(1);
    entity
}

fn research_view(id: u32, owner: u32, fraction: f32) -> EntityView {
    let mut entity = EntityView::new(
        id,
        owner,
        "engineering_complex",
        400.0,
        400.0,
        500,
        500,
        "idle",
    );
    entity.prod_upgrade = Some("tank_unlock".to_string());
    entity.prod_progress = Some(fraction);
    entity.prod_queue = Some(1);
    entity
}

fn with_entities(extra: Vec<EntityView>) -> Snapshot {
    let mut snapshot = snapshot();
    snapshot.entities.extend(extra);
    snapshot
}

fn approx_eq(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.000_01,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn baseline_from_snapshot_is_owner_safe() {
    let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
    assert_eq!(baseline.tick, 10);
    assert_eq!(baseline.owned_entities.len(), 1);
    assert_eq!(baseline.owned_entities[0].id, 101);
    assert_eq!(baseline.owned_entities[0].order_plan.len(), 1);
    assert_eq!(baseline.visible_obstacles.len(), 2);
    assert!(baseline
        .visible_obstacles
        .iter()
        .any(|obstacle| obstacle.kind == "tank_trap"));
    let json = serde_json::to_value(&baseline).unwrap();
    let serialized = serde_json::to_string(&json).unwrap();
    assert!(!serialized.contains("202"));
    assert!(!serialized.contains("target"));
    assert!(!serialized.contains("production"));
    assert!(!serialized.contains("playerResources"));
}

#[test]
fn progress_baseline_is_exact_owner_only_and_contains_only_safe_fields() {
    let authoritative = with_entities(vec![
        construction_view(401, 1, 0.25, true),
        construction_view(402, 2, 0.4, true),
        production_view(403, 3, 0.5),
    ]);
    let baseline = OwnedPredictionBaseline::from_snapshot(1, &authoritative);

    assert_eq!(baseline.progress.len(), 1);
    assert_eq!(baseline.progress[0].id, 401);
    assert_eq!(
        serde_json::to_value(&baseline.progress[0]).unwrap(),
        serde_json::json!({
            "id": 401,
            "kind": "construction",
            "identity": "build:barracks",
            "fraction": 0.25,
            "totalTicks": balance::building_stats(EntityKind::Barracks).unwrap().build_ticks
        })
    );
    let serialized = serde_json::to_string(&baseline.progress).unwrap();
    assert!(!serialized.contains("402"));
    assert!(!serialized.contains("403"));
    assert!(!serialized.contains("owner"));
    assert!(!serialized.contains("hp"));
    assert!(!serialized.contains("state"));
}

#[test]
fn imported_progress_must_reference_a_matching_owned_building() {
    let mut missing = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
    missing.progress.push(OwnedProgressBaseline {
        id: 999,
        kind: ProgressPredictionKind::Production,
        identity: "unit:worker".to_string(),
        fraction: 0.2,
        total_ticks: balance::unit_stats(EntityKind::Worker).unwrap().build_ticks,
    });
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    assert!(predictor.import_baseline(missing).is_err());

    let mut mismatched = OwnedPredictionBaseline::from_snapshot(
        1,
        &with_entities(vec![production_view(410, 1, 0.2)]),
    );
    mismatched.progress[0].total_ticks += 1;
    assert!(predictor.import_baseline(mismatched).is_err());
}

#[test]
fn construction_progress_supports_fractional_visual_ticks_and_sparse_output() {
    let baseline = OwnedPredictionBaseline::from_snapshot(
        1,
        &with_entities(vec![construction_view(401, 1, 0.25, true)]),
    );
    let total = baseline.progress[0].total_ticks;
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline).unwrap();

    assert!(predictor.render_prediction_frame(0.0).progress.is_empty());
    let frame = predictor.render_prediction_frame(0.5);
    assert_eq!(frame.progress.len(), 1);
    approx_eq(frame.progress[0].fraction, 0.25 + 0.5 / total as f32);
    assert_eq!(
        serde_json::to_value(&frame.progress[0]).unwrap(),
        serde_json::json!({
            "id": 401,
            "kind": "construction",
            "identity": "build:barracks",
            "fraction": frame.progress[0].fraction
        })
    );

    let negative = predictor.render_prediction_frame(-2.0);
    let not_finite = predictor.render_prediction_frame(f32::NAN);
    assert!(negative.progress.is_empty());
    assert!(not_finite.progress.is_empty());
    let clamped = predictor.render_prediction_frame(4.0);
    assert!(clamped.progress[0].fraction <= 0.25 + 1.0 / total as f32);
}

#[test]
fn production_and_research_use_rust_authoritative_durations() {
    let baseline = OwnedPredictionBaseline::from_snapshot(
        1,
        &with_entities(vec![
            production_view(410, 1, 0.2),
            research_view(411, 1, 0.4),
        ]),
    );
    assert_eq!(baseline.progress.len(), 2);
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline).unwrap();
    predictor.advance_ticks(1);
    let frame = predictor.render_prediction_frame(0.0);

    let production = frame.progress.iter().find(|patch| patch.id == 410).unwrap();
    assert_eq!(production.identity, "unit:worker");
    approx_eq(
        production.fraction,
        0.2 + 1.0 / balance::unit_stats(EntityKind::Worker).unwrap().build_ticks as f32,
    );
    let research = frame.progress.iter().find(|patch| patch.id == 411).unwrap();
    assert_eq!(research.identity, "upgrade:tank_unlock");
    let research_ticks =
        rts_sim::game::upgrade::definition(rts_sim::game::upgrade::UpgradeKind::TankUnlock)
            .research_ticks;
    approx_eq(research.fraction, 0.4 + 1.0 / research_ticks as f32);
}

#[test]
fn progress_admission_is_fail_closed_for_inactive_waiting_complete_and_invalid_identity() {
    let mut inactive = construction_view(420, 1, 0.2, false);
    inactive.state = "construct".to_string();
    let mut waiting = production_view(421, 1, 0.2);
    waiting.prod_waiting = true;
    let complete = production_view(422, 1, 1.0);
    let mut unknown = production_view(423, 1, 0.2);
    unknown.prod_kind = Some("not_a_unit".to_string());
    let mut ambiguous = production_view(424, 1, 0.2);
    ambiguous.prod_upgrade = Some("tank_unlock".to_string());
    let baseline = OwnedPredictionBaseline::from_snapshot(
        1,
        &with_entities(vec![inactive, waiting, complete, unknown, ambiguous]),
    );
    assert!(baseline.progress.is_empty());
}

#[test]
fn progress_caps_below_completion_and_never_regresses_high_authority() {
    let baseline = OwnedPredictionBaseline::from_snapshot(
        1,
        &with_entities(vec![
            construction_view(430, 1, 0.97, true),
            production_view(431, 1, 0.99),
        ]),
    );
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline).unwrap();
    predictor.advance_ticks(10_000);
    let frame = predictor.render_prediction_frame(0.75);
    assert_eq!(frame.progress.len(), 1);
    assert_eq!(frame.progress[0].id, 430);
    approx_eq(frame.progress[0].fraction, PROGRESS_PREDICTION_MAX);
}

#[test]
fn progress_reimport_corrects_replaces_identity_and_clears_cancelled_lane() {
    let first = OwnedPredictionBaseline::from_snapshot(
        1,
        &with_entities(vec![production_view(440, 1, 0.25)]),
    );
    let total = first.progress[0].total_ticks;
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(first).unwrap();
    predictor.advance_ticks(10);

    let mut corrected_snapshot = with_entities(vec![production_view(440, 1, 0.2)]);
    corrected_snapshot.tick = 20;
    predictor
        .import_baseline(OwnedPredictionBaseline::from_snapshot(
            1,
            &corrected_snapshot,
        ))
        .unwrap();
    approx_eq(
        predictor.diagnostics().progress_correction_magnitude,
        (0.25 + 10.0 / total as f32) - 0.2,
    );
    assert!(predictor.render_prediction_frame(0.0).progress.is_empty());

    let mut changed = production_view(440, 1, 0.1);
    changed.prod_kind = Some("rifleman".to_string());
    let mut changed_snapshot = with_entities(vec![changed]);
    changed_snapshot.tick = 21;
    predictor
        .import_baseline(OwnedPredictionBaseline::from_snapshot(1, &changed_snapshot))
        .unwrap();
    assert_eq!(predictor.diagnostics().progress_correction_magnitude, 0.0);
    predictor.advance_ticks(1);
    assert_eq!(
        predictor.render_prediction_frame(0.0).progress[0].identity,
        "unit:rifleman"
    );

    let mut cancelled_building = production_view(440, 1, 0.0);
    cancelled_building.prod_kind = None;
    cancelled_building.prod_progress = None;
    cancelled_building.prod_queue = Some(0);
    let mut cancelled = with_entities(vec![cancelled_building]);
    cancelled.tick = 22;
    predictor
        .import_baseline(OwnedPredictionBaseline::from_snapshot(1, &cancelled))
        .unwrap();
    predictor.advance_ticks(5);
    assert!(predictor.render_prediction_frame(0.5).progress.is_empty());
}

#[test]
fn command_enqueue_does_not_advance_global_or_progress_time() {
    let baseline = OwnedPredictionBaseline::from_snapshot(
        1,
        &with_entities(vec![production_view(450, 1, 0.25)]),
    );
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline).unwrap();
    let before = predictor.render_prediction_frame(0.5);

    predictor.enqueue_command(
        7,
        Command::Move {
            units: vec![101],
            x: 140.0,
            y: 100.0,
            queued: false,
        },
    );

    let after = predictor.render_prediction_frame(0.5);
    assert_eq!(after.tick, before.tick);
    assert_eq!(after.progress, before.progress);
    assert_eq!(predictor.diagnostics().tick, 10);
}

#[test]
fn prediction_frame_contains_only_owned_pose_patches() {
    let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline).unwrap();

    let rendered = predictor.render_prediction_frame(0.0);
    assert_eq!(rendered.entities.len(), 1);
    assert_eq!(rendered.entities[0].id, 101);
    assert_eq!(rendered.entities[0].x, 100.0);
    assert_eq!(rendered.entities[0].y, 100.0);
    assert_eq!(rendered.entities[0].motion, None);

    let json = serde_json::to_value(&rendered).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "tick": 10,
            "entities": [{ "id": 101, "x": 100.0, "y": 100.0 }],
            "progress": []
        })
    );

    let diagnostics = predictor.diagnostics();
    assert!(diagnostics
        .unsupported_fields
        .contains(&"combat".to_string()));
    assert!(diagnostics
        .unsupported_fields
        .contains(&"fogReconstruction".to_string()));
    assert_eq!(diagnostics.visible_obstacle_count, 2);
}

#[test]
fn baseline_gather_and_build_activity_are_preserved_by_absence() {
    for authoritative_state in ["gather", "build"] {
        let mut authoritative = snapshot();
        authoritative.entities[0].state = authoritative_state.to_string();
        authoritative.entities[0].order_plan.clear();
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &authoritative);
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();
        predictor.enqueue_command(
            5,
            Command::Move {
                units: vec![101],
                x: 140.0,
                y: 100.0,
                queued: true,
            },
        );

        predictor.advance_ticks(5);
        let frame = predictor.render_prediction_frame(0.0);
        assert!(frame.entities.is_empty(), "{authoritative_state}");
    }
}

#[test]
fn unsupported_active_order_plan_stage_discards_unreachable_supported_suffix() {
    for unsupported_kind in ["gather", "build"] {
        let mut authoritative = snapshot();
        authoritative.entities[0].state = unsupported_kind.to_string();
        authoritative.entities[0].order_plan = vec![
            OrderPlanMarker {
                kind: unsupported_kind.to_string(),
                x: 100.0,
                y: 100.0,
            },
            OrderPlanMarker {
                kind: "move".to_string(),
                x: 140.0,
                y: 100.0,
            },
        ];

        let baseline = OwnedPredictionBaseline::from_snapshot(1, &authoritative);
        assert_eq!(
            baseline.owned_entities[0].authoritative_barrier_after,
            Some(0)
        );
        assert!(baseline.owned_entities[0].order_plan.is_empty());

        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();
        predictor.advance_ticks(30);
        assert!(
            predictor.render_prediction_frame(0.0).entities.is_empty(),
            "{unsupported_kind}"
        );
    }
}

#[test]
fn middle_authoritative_barrier_stops_retained_and_local_queued_moves() {
    for barrier_kind in ["gather", "build"] {
        let mut authoritative = snapshot();
        authoritative.entities[0].state = "move".to_string();
        authoritative.entities[0].order_plan = vec![
            OrderPlanMarker {
                kind: "move".to_string(),
                x: 102.0,
                y: 100.0,
            },
            OrderPlanMarker {
                kind: barrier_kind.to_string(),
                x: 102.0,
                y: 100.0,
            },
            OrderPlanMarker {
                kind: "move".to_string(),
                x: 140.0,
                y: 100.0,
            },
        ];
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &authoritative);
        assert_eq!(
            baseline.owned_entities[0].authoritative_barrier_after,
            Some(1)
        );
        assert_eq!(baseline.owned_entities[0].order_plan.len(), 1);

        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();
        predictor.enqueue_command(
            9,
            Command::Move {
                units: vec![101],
                x: 160.0,
                y: 100.0,
                queued: true,
            },
        );
        predictor.advance_ticks(60);

        let frame = predictor.render_prediction_frame(0.0);
        assert_eq!(frame.entities[0].x, 102.0, "{barrier_kind}");
        assert_eq!(frame.entities[0].motion, None, "{barrier_kind}");
        let summary = predictor.local_lane_summary();
        assert!(summary.owned_entities[0].order_plan.is_empty());
        assert_eq!(summary.owned_entities[0].queued_order_stages[0].x, 160.0);
    }
}

#[test]
fn non_movement_authoritative_state_blocks_visible_move_marker() {
    for authoritative_state in ["attack", "gather", "build", "idle"] {
        let mut authoritative = snapshot();
        authoritative.entities[0].state = authoritative_state.to_string();
        authoritative.entities[0].order_plan = vec![OrderPlanMarker {
            kind: "move".to_string(),
            x: 140.0,
            y: 100.0,
        }];
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &authoritative);
        assert_eq!(
            baseline.owned_entities[0].authoritative_barrier_after,
            Some(0)
        );

        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();
        predictor.advance_ticks(60);
        assert!(
            predictor.render_prediction_frame(0.0).entities.is_empty(),
            "{authoritative_state}"
        );
        assert_eq!(
            predictor.local_lane_summary().owned_entities[0].queued_order_stages[0].x,
            140.0
        );
    }
}

#[test]
fn baseline_movement_keeps_terminal_pose_claim_until_reconciliation() {
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor
        .import_baseline(OwnedPredictionBaseline::from_snapshot(1, &snapshot()))
        .unwrap();

    predictor.advance_ticks(100);
    assert_eq!(
        serde_json::to_value(predictor.render_prediction_frame(0.0)).unwrap(),
        serde_json::json!({
            "tick": 110,
            "entities": [{
                "id": 101,
                "x": 120.0,
                "y": 100.0,
                "facing": 0.0
            }],
            "progress": []
        })
    );

    predictor.advance_ticks(30);
    let retained = predictor.render_prediction_frame(0.0);
    assert_eq!(retained.entities[0].x, 120.0);
    assert_eq!(retained.entities[0].motion, None);

    let mut reconciled = snapshot();
    reconciled.tick = 140;
    reconciled.entities[0].x = 120.0;
    reconciled.entities[0].state = "idle".to_string();
    reconciled.entities[0].order_plan.clear();
    predictor
        .import_baseline(OwnedPredictionBaseline::from_snapshot(1, &reconciled))
        .unwrap();
    assert!(predictor.render_prediction_frame(0.0).entities.is_empty());
}

#[test]
fn serialized_pose_patch_has_no_identity_or_full_state_claims() {
    let patch = EntityPredictionPatch {
        id: 101,
        x: 101.5,
        y: 99.25,
        facing: Some(0.5),
        motion: Some(PredictedMotion::Move),
    };

    assert_eq!(
        serde_json::to_value(patch).unwrap(),
        serde_json::json!({
            "id": 101,
            "x": 101.5,
            "y": 99.25,
            "facing": 0.5,
            "motion": "move"
        })
    );
}

#[test]
fn attack_command_is_authoritative_only() {
    let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline).unwrap();
    let before = predictor.render_prediction_frame(0.0);

    predictor.enqueue_command(
        7,
        Command::Attack {
            units: vec![101],
            target: 202,
            tank_trap_cluster: false,
            queued: false,
        },
    );

    let after = predictor.render_prediction_frame(0.0);
    assert_eq!(after.entities[0].x, before.entities[0].x);
    assert_eq!(after.entities[0].y, before.entities[0].y);
    assert_eq!(after.entities[0].motion, None);
    let diagnostics = predictor.diagnostics();
    assert_eq!(diagnostics.pending_client_seqs, vec![7]);
    assert!(diagnostics
        .disabled_reasons
        .contains(&"commandUnsupported".to_string()));
    assert!(diagnostics
        .unsupported_fields
        .contains(&"combat".to_string()));
}

#[test]
fn repeat_production_command_is_tracked_as_authoritative_only() {
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.enqueue_command(
        8,
        Command::AdjustProductionRepeat {
            buildings: vec![301],
            unit: "rifleman".to_string(),
            delta: 1,
        },
    );

    let diagnostics = predictor.diagnostics();
    assert_eq!(
        diagnostics.pending_command_kinds,
        vec!["adjustProductionRepeat"]
    );
    assert!(diagnostics
        .disabled_reasons
        .contains(&"commandUnsupported".to_string()));
}

#[test]
fn no_op_ticks_are_deterministic() {
    let baseline = OwnedPredictionBaseline {
        tick: 1,
        player_id: 1,
        steel: Some(75),
        oil: Some(0),
        supply_used: Some(1),
        supply_cap: Some(10),
        owned_entities: vec![OwnedEntityBaseline {
            id: 1,
            kind: "worker".to_string(),
            x: 10.0,
            y: 10.0,
            hp: 40,
            max_hp: 40,
            state: Some("idle".to_string()),
            facing: None,
            weapon_facing: None,
            order_plan: Vec::new(),
            authoritative_barrier_after: None,
        }],
        progress: Vec::new(),
        visible_obstacles: Vec::new(),
    };
    let mut a = predictor_from_start_payload(start_payload(), 1);
    let mut b = predictor_from_start_payload(start_payload(), 1);
    a.import_baseline(baseline.clone()).unwrap();
    b.import_baseline(baseline).unwrap();
    a.advance_ticks(30);
    b.advance_ticks(30);
    assert_eq!(
        a.render_prediction_frame(0.0),
        b.render_prediction_frame(0.0)
    );
    assert!(a.render_prediction_frame(0.0).entities.is_empty());
}

#[test]
fn simple_move_command_advances_owned_unit() {
    let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline).unwrap();
    predictor.enqueue_command(
        1,
        Command::Move {
            units: vec![101],
            x: 110.0,
            y: 100.0,
            queued: false,
        },
    );
    predictor.advance_ticks(3);
    let frame = predictor.render_prediction_frame(0.0);
    let entity = &frame.entities[0];
    assert!(entity.x > 100.0);
    assert_eq!(entity.y, 100.0);
    assert_eq!(entity.motion, Some(PredictedMotion::Move));
    assert_eq!(entity.facing, Some(0.0));
    assert_eq!(predictor.diagnostics().pending_commands, 1);
}

#[test]
fn rectangular_map_clamps_move_targets_per_axis() {
    let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
    let mut start = start_payload();
    start.map.width = 8;
    start.map.height = 4;
    start.map.terrain = vec![0; 8 * 4];
    let mut predictor = predictor_from_start_payload(start, 1);
    predictor.import_baseline(baseline).unwrap();

    predictor.enqueue_command(
        1,
        Command::Move {
            units: vec![101],
            x: 10_000.0,
            y: 10_000.0,
            queued: false,
        },
    );

    let order = &predictor.local_lane_summary().owned_entities[0].order_plan[0];
    assert_eq!(order.x, 8.0 * balance::TILE_SIZE as f32 - 0.01);
    assert_eq!(order.y, 4.0 * balance::TILE_SIZE as f32 - 0.01);
}

#[test]
fn queued_move_commands_are_preserved_in_order() {
    let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline).unwrap();
    predictor.enqueue_command(
        1,
        Command::Move {
            units: vec![101],
            x: 102.0,
            y: 100.0,
            queued: false,
        },
    );
    predictor.enqueue_command(
        2,
        Command::Move {
            units: vec![101],
            x: 102.0,
            y: 104.0,
            queued: true,
        },
    );
    let summary = predictor.local_lane_summary();
    assert_eq!(summary.owned_entities[0].order_plan[0].x, 102.0);
    assert_eq!(summary.owned_entities[0].queued_order_stages[0].y, 104.0);
    predictor.advance_ticks(2);
    assert_eq!(
        predictor.render_prediction_frame(0.0).entities[0].motion,
        Some(PredictedMotion::Move)
    );
}

#[test]
fn queued_hold_position_follows_the_last_move_then_stands_ground() {
    let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline).unwrap();
    predictor.enqueue_command(
        1,
        Command::Move {
            units: vec![101],
            x: 110.0,
            y: 100.0,
            queued: false,
        },
    );
    predictor.enqueue_command(
        2,
        Command::HoldPosition {
            units: vec![101],
            queued: true,
        },
    );

    let summary = predictor.local_lane_summary();
    assert_eq!(summary.owned_entities[0].order_plan[0].kind, "move");
    assert_eq!(
        summary.owned_entities[0].queued_order_stages[0].kind,
        "holdPosition"
    );

    predictor.advance_ticks(16);
    let frame = predictor.render_prediction_frame(0.0);
    let entity = &frame.entities[0];
    assert_eq!(entity.x, 110.0);
    assert_eq!(entity.y, 100.0);
    assert_eq!(entity.motion, Some(PredictedMotion::Idle));
}

#[test]
fn authoritative_baseline_preserves_terminal_hold_position() {
    let mut authoritative = snapshot();
    authoritative.entities[0].order_plan = vec![
        OrderPlanMarker {
            kind: "move".to_string(),
            x: 110.0,
            y: 100.0,
        },
        OrderPlanMarker {
            kind: "holdPosition".to_string(),
            x: 110.0,
            y: 100.0,
        },
    ];
    let baseline = OwnedPredictionBaseline::from_snapshot(1, &authoritative);
    assert_eq!(baseline.owned_entities[0].order_plan.len(), 2);

    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline).unwrap();
    predictor.enqueue_command(
        3,
        Command::Move {
            units: vec![101],
            x: 120.0,
            y: 100.0,
            queued: true,
        },
    );

    let summary = predictor.local_lane_summary();
    assert_eq!(summary.owned_entities[0].order_plan[0].kind, "move");
    assert_eq!(summary.owned_entities[0].queued_order_stages.len(), 1);
    assert_eq!(
        summary.owned_entities[0].queued_order_stages[0].kind,
        "holdPosition"
    );
}

#[test]
fn held_unit_promotes_a_later_queued_move() {
    let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline).unwrap();
    predictor.enqueue_command(
        1,
        Command::HoldPosition {
            units: vec![101],
            queued: false,
        },
    );
    let held = &predictor.render_prediction_frame(0.0).entities[0];
    assert_eq!(held.motion, Some(PredictedMotion::Idle));

    predictor.enqueue_command(
        2,
        Command::Move {
            units: vec![101],
            x: 110.0,
            y: 100.0,
            queued: true,
        },
    );

    let queued = &predictor.render_prediction_frame(0.0).entities[0];
    assert_eq!(queued.motion, Some(PredictedMotion::Idle));
    assert_eq!(
        predictor.local_lane_summary().owned_entities[0].queued_order_stages[0].kind,
        "move"
    );

    predictor.advance_ticks(2);
    let moving = &predictor.render_prediction_frame(0.0).entities[0];
    assert!(moving.x > 100.0);
    assert_eq!(moving.motion, Some(PredictedMotion::Move));
}

#[test]
fn importing_authoritative_baseline_clears_replayed_pending_commands() {
    let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline.clone()).unwrap();
    predictor.enqueue_command(
        7,
        Command::Move {
            units: vec![101],
            x: 140.0,
            y: 100.0,
            queued: false,
        },
    );
    assert_eq!(predictor.diagnostics().pending_client_seqs, vec![7]);

    predictor.import_baseline(baseline).unwrap();
    assert!(predictor.diagnostics().pending_client_seqs.is_empty());
    predictor.enqueue_command(
        7,
        Command::Move {
            units: vec![101],
            x: 140.0,
            y: 100.0,
            queued: false,
        },
    );
    assert_eq!(predictor.diagnostics().pending_client_seqs, vec![7]);
}

#[test]
fn invalid_build_is_reported_unsupported_without_mutating_baseline() {
    let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
    let mut predictor = predictor_from_start_payload(start_payload(), 1);
    predictor.import_baseline(baseline).unwrap();
    predictor.enqueue_command(
        1,
        Command::Build {
            units: vec![101],
            building: "not_a_building".to_string(),
            tile_x: u32::MAX,
            tile_y: u32::MAX,
            queued: false,
        },
    );
    predictor.advance_ticks(1);
    let diagnostics = predictor.diagnostics();
    assert!(diagnostics
        .disabled_reasons
        .contains(&"buildPredictionUnsupported".to_string()));
    assert!(diagnostics
        .unsupported_fields
        .contains(&"construction".to_string()));
}

#[test]
fn json_api_round_trips_like_wasm_binding() {
    let start_json = serde_json::to_string(&start_payload()).unwrap();
    let baseline_json =
        serde_json::to_string(&OwnedPredictionBaseline::from_snapshot(1, &snapshot())).unwrap();
    let command_json = serde_json::to_string(&Command::Move {
        units: vec![101],
        x: 108.0,
        y: 100.0,
        queued: false,
    })
    .unwrap();
    let mut predictor =
        CorePredictor::from_start_payload(serde_json::from_str(&start_json).unwrap(), 1);
    predictor
        .import_baseline(serde_json::from_str(&baseline_json).unwrap())
        .unwrap();
    predictor.enqueue_command(1, serde_json::from_str(&command_json).unwrap());
    predictor.advance_ticks(5);
    let render_json = serde_json::to_string(&predictor.prediction_frame(0.0)).unwrap();
    assert!(render_json.contains("\"tick\":15"));
    assert!(serde_json::to_string(&predictor.diagnostics())
        .unwrap()
        .contains("pendingCommands"));
}
