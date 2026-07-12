use super::checkpoint_helpers::{
    assert_debug_path_visible, assert_equivalent_games, player_ids,
    repair_after_authoritative_test_spawn, restore_checkpoint_and_assert_equivalent,
    tick_pair_and_assert_equivalent, tick_pair_for,
};
use super::fixtures::empty_flat_game;
use super::lab::{LabCommandOptions, LabMoveEntity, LabOp, LabOpOutcome, LabSpawnEntity};
use super::*;
use crate::game::entity::{BuildPhase, DeconstructPhase, MovePhase, RallyKind};
use crate::game::services::occupancy::footprint_center;
use crate::game::upgrade::UpgradeKind;
use crate::protocol::Command as WireCommand;

#[test]
fn derived_state_wipe_rebuild_preserves_pathing_state_and_snapshots() {
    let (mut baseline, tank, goal, return_goal) = derived_state_pathing_fixture();
    let mut wiped = baseline.clone_for_replay_keyframe();

    enqueue_pair(
        &mut baseline,
        &mut wiped,
        1,
        Command::Move {
            units: vec![tank],
            x: goal.0,
            y: goal.1,
            queued: false,
        },
    );
    assert_equivalent_games(&baseline, &wiped, "queued warm-path command");
    tick_pair_and_assert_equivalent(&mut baseline, &mut wiped, "warm path cache tick");

    assert!(
        baseline.pathing_cache_len_for_test() > 0,
        "pathing-heavy setup should warm the baseline path cache before the wipe"
    );
    assert_eq!(
        baseline.pathing_cache_len_for_test(),
        wiped.pathing_cache_len_for_test(),
        "paired games should warm the same cache entries before the wipe"
    );
    let baseline_pathing_config = baseline.pathing_config_for_test();
    assert_eq!(
        baseline_pathing_config,
        wiped.pathing_config_for_test(),
        "paired games should use the same live pathing budget/cache configuration before the wipe"
    );
    assert!(
        !baseline
            .state
            .entities
            .get(tank)
            .expect("tank should survive")
            .path_is_empty(),
        "the selected movement path must live on the entity, not only in the pathing cache"
    );

    wiped.clear_and_rebuild_derived_state_for_test();
    assert_eq!(
        wiped.pathing_cache_len_for_test(),
        0,
        "the derived-state wipe should clear the persistent pathing cache"
    );
    assert_eq!(
        baseline_pathing_config,
        wiped.pathing_config_for_test(),
        "clearing derived pathing state must preserve the live default budget/cache capacity"
    );
    assert_equivalent_games(&baseline, &wiped, "after derived-state wipe/rebuild");

    for tick in 0..24 {
        tick_pair_and_assert_equivalent(
            &mut baseline,
            &mut wiped,
            &format!("post-wipe selected path tick {tick}"),
        );
    }

    enqueue_pair(
        &mut baseline,
        &mut wiped,
        1,
        Command::Move {
            units: vec![tank],
            x: return_goal.0,
            y: return_goal.1,
            queued: false,
        },
    );
    wiped.clear_and_rebuild_derived_state_for_test();
    assert_equivalent_games(&baseline, &wiped, "queued post-wipe repath command");
    tick_pair_and_assert_equivalent(&mut baseline, &mut wiped, "post-wipe repath tick");
    assert!(
        wiped.pathing_cache_len_for_test() > 0,
        "future path requests should rebuild pathing cache entries after the wipe"
    );

    for tick in 0..36 {
        tick_pair_and_assert_equivalent(
            &mut baseline,
            &mut wiped,
            &format!("post-repath movement tick {tick}"),
        );
    }
}

#[test]
fn checkpoint_export_import_rebuilds_derived_state_and_preserves_semantics() {
    let (mut baseline, tank, goal, return_goal) = derived_state_pathing_fixture();
    baseline.enqueue(
        1,
        Command::Move {
            units: vec![tank],
            x: goal.0,
            y: goal.1,
            queued: false,
        },
    );
    baseline.tick();
    assert!(
        baseline.pathing_cache_len_for_test() > 0,
        "pathing-heavy setup should warm the reusable cache before checkpoint export"
    );

    baseline.enqueue(
        1,
        Command::Move {
            units: vec![tank],
            x: return_goal.0,
            y: return_goal.1,
            queued: false,
        },
    );
    let mut restored =
        restore_checkpoint_and_assert_equivalent(&baseline, "after cold checkpoint import");

    let spawn_pos = baseline.state.map.tile_center(30, 30);
    let baseline_spawn = baseline
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, spawn_pos.0, spawn_pos.1)
        .expect("baseline allocator should spawn a post-checkpoint unit");
    let restored_spawn = restored
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, spawn_pos.0, spawn_pos.1)
        .expect("restored allocator should spawn a post-checkpoint unit");
    assert_eq!(
        baseline_spawn, restored_spawn,
        "same post-checkpoint allocation should receive the same stable entity id"
    );
    repair_after_authoritative_test_spawn(&mut baseline);
    repair_after_authoritative_test_spawn(&mut restored);
    assert_equivalent_games(
        &baseline,
        &restored,
        "after matching post-checkpoint allocation",
    );

    for tick in 0..32 {
        tick_pair_and_assert_equivalent(
            &mut baseline,
            &mut restored,
            &format!("post-checkpoint command tick {tick}"),
        );
    }
}

#[test]
fn movement_economy_checkpoint_applies_pending_commands_once_and_preserves_existing_log() {
    let (mut baseline, tank, goal, return_goal) = derived_state_pathing_fixture();
    baseline.enqueue(
        1,
        Command::Move {
            units: vec![tank],
            x: goal.0,
            y: goal.1,
            queued: false,
        },
    );
    baseline.tick();
    assert_eq!(
        baseline.command_log().len(),
        1,
        "fixture should have one already-applied command before the checkpoint"
    );

    let queued_attack_goal = baseline.state.map.tile_center(22, 16);
    baseline.enqueue(
        1,
        Command::Move {
            units: vec![tank],
            x: return_goal.0,
            y: return_goal.1,
            queued: true,
        },
    );
    baseline.enqueue(
        1,
        Command::AttackMove {
            units: vec![tank],
            x: queued_attack_goal.0,
            y: queued_attack_goal.1,
            queued: true,
        },
    );
    let checkpoint_tick = baseline.tick_count();
    let checkpoint_log_len = baseline.command_log().len();
    let checkpoint_pending_len = baseline.state.pending.len();
    assert_eq!(
        checkpoint_pending_len, 2,
        "fixture should checkpoint two pending commands before the next tick"
    );

    let mut restored = restore_checkpoint_and_assert_equivalent(
        &baseline,
        "pending commands preserved at checkpoint boundary",
    );
    assert_eq!(
        restored.state.pending.len(),
        checkpoint_pending_len,
        "checkpoint import should preserve queued pending commands"
    );

    tick_pair_and_assert_equivalent(
        &mut baseline,
        &mut restored,
        "pending command drain after checkpoint",
    );

    let baseline_log = baseline.command_log();
    let restored_log = restored.command_log();
    assert_eq!(baseline_log, restored_log);
    assert_eq!(
        baseline_log.len(),
        checkpoint_log_len + checkpoint_pending_len,
        "pending commands should be recorded exactly once"
    );
    let appended = &baseline_log[checkpoint_log_len..];
    assert!(
        appended
            .iter()
            .all(|entry| entry.tick == checkpoint_tick + 1),
        "pending commands should receive the first post-checkpoint tick stamp"
    );
    assert!(
        matches!(
            appended[0].command,
            crate::protocol::Command::Move { queued: true, .. }
        ),
        "first pending command should keep command-log order"
    );
    assert!(
        matches!(
            appended[1].command,
            crate::protocol::Command::AttackMove { queued: true, .. }
        ),
        "second pending command should keep command-log order"
    );

    tick_pair_for(
        &mut baseline,
        &mut restored,
        12,
        "post-pending movement/economy checkpoint",
    );
}

#[test]
fn movement_economy_checkpoint_preserves_active_paths_and_debug_projection() {
    let (mut baseline, tank, goal, return_goal) = derived_state_pathing_fixture();
    baseline.enqueue(
        1,
        Command::Move {
            units: vec![tank],
            x: goal.0,
            y: goal.1,
            queued: false,
        },
    );
    baseline.tick();
    let second_queued_goal = baseline.state.map.tile_center(22, 18);
    baseline.enqueue(
        1,
        Command::Move {
            units: vec![tank],
            x: return_goal.0,
            y: return_goal.1,
            queued: true,
        },
    );
    baseline.enqueue(
        1,
        Command::AttackMove {
            units: vec![tank],
            x: second_queued_goal.0,
            y: second_queued_goal.1,
            queued: true,
        },
    );
    baseline.tick();

    let moving_tank = baseline
        .state
        .entities
        .get(tank)
        .expect("tank should survive");
    match moving_tank.order() {
        Order::Move(order) => assert_eq!(
            order.execution.phase,
            MovePhase::Moving,
            "checkpoint should catch the active move after path selection"
        ),
        other => panic!("expected active move order, got {other:?}"),
    }
    assert_eq!(
        moving_tank.queued_orders().len(),
        2,
        "future queued movement stages should be durable entity state"
    );
    assert!(
        !moving_tank.path_is_empty(),
        "selected path/waypoints should be live at the checkpoint"
    );
    assert_eq!(moving_tank.path_goal(), Some(goal));
    assert_debug_path_visible(&baseline, 1, tank, "baseline active movement checkpoint");

    let mut restored = restore_checkpoint_and_assert_equivalent(
        &baseline,
        "active movement/order checkpoint import",
    );
    assert_debug_path_visible(&restored, 1, tank, "restored active movement checkpoint");

    tick_pair_for(
        &mut baseline,
        &mut restored,
        40,
        "active movement/order checkpoint continuation",
    );
}

#[test]
fn movement_economy_checkpoint_preserves_harvesting_state_and_resource_projection() {
    let players = phase5_players();
    let mut baseline = empty_flat_game(&players);
    let cc_pos = footprint_center(&baseline.state.map, EntityKind::CityCentre, 8, 8);
    baseline
        .state
        .entities
        .spawn_building(1, EntityKind::CityCentre, cc_pos.0, cc_pos.1, true)
        .expect("city centre should spawn");
    let enemy_cc = footprint_center(&baseline.state.map, EntityKind::CityCentre, 40, 40);
    baseline
        .state
        .entities
        .spawn_building(2, EntityKind::CityCentre, enemy_cc.0, enemy_cc.1, true)
        .expect("enemy city centre should spawn");
    let node_pos = (cc_pos.0 + config::TILE_SIZE as f32 * 3.0, cc_pos.1);
    let node = baseline
        .state
        .entities
        .spawn_node(EntityKind::Steel, node_pos.0, node_pos.1)
        .expect("steel node should spawn");
    let worker = baseline
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, node_pos.0, node_pos.1)
        .expect("worker should spawn");
    baseline.state.players[0].set_resources(25, 7);
    repair_after_authoritative_test_spawn(&mut baseline);

    baseline.enqueue(
        1,
        Command::Gather {
            units: vec![worker],
            node,
            queued: false,
        },
    );
    baseline.tick();
    for _ in 0..8 {
        baseline.tick();
    }
    assert_eq!(
        baseline
            .state
            .entities
            .get(worker)
            .and_then(|worker| worker.gather_phase()),
        Some(GatherPhase::Harvesting),
        "checkpoint should catch worker harvest progress in flight"
    );
    assert_eq!(
        baseline.state.entities.node_slot_holder(node),
        Some(worker),
        "resource-node miner reservation should be live at the checkpoint"
    );

    let mut restored =
        restore_checkpoint_and_assert_equivalent(&baseline, "harvesting checkpoint import");
    tick_pair_for(
        &mut baseline,
        &mut restored,
        config::HARVEST_TICKS + 4,
        "harvesting checkpoint continuation",
    );
    assert_eq!(
        baseline
            .state
            .entities
            .get(node)
            .and_then(|node| node.remaining()),
        restored
            .state
            .entities
            .get(node)
            .and_then(|node| node.remaining()),
        "resource-node remaining amount should stay equivalent after harvest payout"
    );
    assert_eq!(
        baseline.state.players[0].steel, restored.state.players[0].steel,
        "player steel totals should stay equivalent after harvest payout"
    );
}

#[test]
fn movement_economy_checkpoint_preserves_construction_and_deconstruction_progress() {
    let players = phase5_players();
    let mut baseline = empty_flat_game(&players);
    let cc_pos = footprint_center(&baseline.state.map, EntityKind::CityCentre, 5, 5);
    baseline
        .state
        .entities
        .spawn_building(1, EntityKind::CityCentre, cc_pos.0, cc_pos.1, true)
        .expect("city centre should spawn");
    let enemy_cc = footprint_center(&baseline.state.map, EntityKind::CityCentre, 43, 43);
    baseline
        .state
        .entities
        .spawn_building(2, EntityKind::CityCentre, enemy_cc.0, enemy_cc.1, true)
        .expect("enemy city centre should spawn");
    let (city_centre_tile_x, city_centre_tile_y) = (12, 8);
    let city_centre_site = footprint_center(
        &baseline.state.map,
        EntityKind::CityCentre,
        city_centre_tile_x,
        city_centre_tile_y,
    );
    let build_worker = baseline
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, city_centre_site.0, city_centre_site.1)
        .expect("build worker should spawn");
    let trap_pos = footprint_center(&baseline.state.map, EntityKind::TankTrap, 20, 8);
    let trap = baseline
        .state
        .entities
        .spawn_building(2, EntityKind::TankTrap, trap_pos.0, trap_pos.1, true)
        .expect("tank trap should spawn");
    let deconstruct_worker = baseline
        .state
        .entities
        .spawn_unit(
            1,
            EntityKind::Worker,
            trap_pos.0 - config::TILE_SIZE as f32 * 1.5,
            trap_pos.1,
        )
        .expect("deconstruct worker should spawn");
    baseline.state.players[0].set_resources(1_000, 1_000);
    repair_after_authoritative_test_spawn(&mut baseline);

    let handoff = baseline.state.map.tile_center(16, 8);
    baseline.enqueue(
        1,
        Command::Build {
            units: vec![build_worker],
            building: EntityKind::CityCentre,
            tile_x: city_centre_tile_x,
            tile_y: city_centre_tile_y,
            queued: false,
        },
    );
    baseline.enqueue(
        1,
        Command::Move {
            units: vec![build_worker],
            x: handoff.0,
            y: handoff.1,
            queued: true,
        },
    );
    baseline.enqueue(
        1,
        Command::Deconstruct {
            units: vec![deconstruct_worker],
            target: trap,
            queued: false,
        },
    );
    baseline.tick();

    let scaffold = baseline
        .state
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::CityCentre && entity.under_construction())
        .map(|entity| entity.id)
        .expect("build command should spawn a scaffold before checkpoint");
    assert!(baseline.state.active_construction_sites.contains(&scaffold));
    assert_eq!(
        baseline
            .state
            .entities
            .get(build_worker)
            .and_then(|worker| worker.build_phase()),
        Some(BuildPhase::Constructing { site: scaffold })
    );
    assert_eq!(
        baseline
            .state
            .entities
            .get(deconstruct_worker)
            .and_then(|worker| worker.deconstruct_phase()),
        Some(DeconstructPhase::Deconstructing)
    );

    let mut restored = restore_checkpoint_and_assert_equivalent(
        &baseline,
        "construction/deconstruction checkpoint import",
    );
    let finish_ticks = config::building_stats(EntityKind::CityCentre)
        .expect("city centre stats")
        .build_ticks
        .max(crate::game::entity::tank_trap_deconstruction_ticks())
        + 4;
    tick_pair_for(
        &mut baseline,
        &mut restored,
        finish_ticks,
        "construction/deconstruction checkpoint continuation",
    );
    assert!(
        baseline
            .state
            .entities
            .get(scaffold)
            .is_some_and(|entity| !entity.under_construction()),
        "scaffold should finish construction after checkpoint continuation"
    );
    assert!(
        baseline.state.entities.get(trap).is_none(),
        "tank trap should be removed after deconstruction continuation"
    );
}

#[test]
fn movement_economy_checkpoint_preserves_production_research_rally_and_allocator_continuity() {
    let players = phase5_players();
    let mut baseline = empty_flat_game(&players);
    let cc_pos = footprint_center(&baseline.state.map, EntityKind::CityCentre, 5, 5);
    baseline
        .state
        .entities
        .spawn_building(1, EntityKind::CityCentre, cc_pos.0, cc_pos.1, true)
        .expect("city centre should spawn");
    let enemy_cc = footprint_center(&baseline.state.map, EntityKind::CityCentre, 43, 43);
    baseline
        .state
        .entities
        .spawn_building(2, EntityKind::CityCentre, enemy_cc.0, enemy_cc.1, true)
        .expect("enemy city centre should spawn");
    let barracks_pos = footprint_center(&baseline.state.map, EntityKind::Barracks, 10, 8);
    let barracks = baseline
        .state
        .entities
        .spawn_building(
            1,
            EntityKind::Barracks,
            barracks_pos.0,
            barracks_pos.1,
            true,
        )
        .expect("barracks should spawn");
    let training_pos = footprint_center(&baseline.state.map, EntityKind::TrainingCentre, 16, 8);
    let training_centre = baseline
        .state
        .entities
        .spawn_building(
            1,
            EntityKind::TrainingCentre,
            training_pos.0,
            training_pos.1,
            true,
        )
        .expect("training centre should spawn");
    baseline.state.players[0].set_resources(1_000, 1_000);
    repair_after_authoritative_test_spawn(&mut baseline);
    baseline.state.players[0].set_resources(1_000, 1_000);

    let rally_a = baseline.state.map.tile_center(18, 14);
    let rally_b = baseline.state.map.tile_center(22, 14);
    baseline.enqueue(
        1,
        Command::SetRally {
            building: barracks,
            x: rally_a.0,
            y: rally_a.1,
            kind: RallyKind::Move,
            queued: false,
        },
    );
    baseline.enqueue(
        1,
        Command::SetRally {
            building: barracks,
            x: rally_b.0,
            y: rally_b.1,
            kind: RallyKind::AttackMove,
            queued: true,
        },
    );
    baseline.enqueue(
        1,
        Command::Train {
            building: barracks,
            unit: EntityKind::Rifleman,
        },
    );
    baseline.enqueue(
        1,
        Command::Research {
            building: training_centre,
            upgrade: UpgradeKind::Entrenchment,
        },
    );
    baseline.tick();

    {
        let producer = baseline
            .state
            .entities
            .get_mut(barracks)
            .expect("barracks should exist");
        let front = producer
            .production
            .as_mut()
            .and_then(|production| production.queue.first_mut())
            .expect("rifleman should be queued");
        front.progress = front.total.saturating_sub(1);
    }
    {
        let researcher = baseline
            .state
            .entities
            .get_mut(training_centre)
            .expect("training centre should exist");
        let front = researcher
            .research_queue_mut()
            .and_then(|queue| queue.first_mut())
            .expect("entrenchment research should be queued");
        front.progress = front.total.saturating_sub(1);
    }

    let next_spawn_id = baseline.state.entities.next_id_for_test();
    let mut restored = restore_checkpoint_and_assert_equivalent(
        &baseline,
        "production/research/rally checkpoint import",
    );
    tick_pair_and_assert_equivalent(
        &mut baseline,
        &mut restored,
        "production/research/rally completion after checkpoint",
    );

    let baseline_spawn = baseline
        .state
        .entities
        .get(next_spawn_id)
        .expect("production should allocate the next entity id");
    let restored_spawn = restored
        .state
        .entities
        .get(next_spawn_id)
        .expect("restored production should allocate the same entity id");
    assert_eq!(baseline_spawn.kind, EntityKind::Rifleman);
    assert_eq!(restored_spawn.kind, EntityKind::Rifleman);
    assert!(
        baseline.state.players[0]
            .upgrades
            .contains(&UpgradeKind::Entrenchment),
        "research completion should insert the upgrade after restore"
    );
    assert!(
        matches!(baseline_spawn.order(), Order::AttackMove(_)),
        "spawned combat unit should receive the first rally stage"
    );
    assert_eq!(
        baseline_spawn.queued_orders().len(),
        1,
        "spawned unit should keep queued rally stages"
    );

    tick_pair_for(
        &mut baseline,
        &mut restored,
        4,
        "post-production rally path checkpoint continuation",
    );
}

#[test]
fn lab_world_mutation_clears_rebuildable_pathing_cache() {
    let mut game = derived_state_lab_fixture();
    let spawn_pos = game.state.map.tile_center(30, 30);
    let LabOpOutcome::Spawned {
        entity_id: scout_id,
    } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::ScoutCar,
            x: spawn_pos.0,
            y: spawn_pos.1,
            completed: true,
        }))
        .expect("scout car should spawn")
    else {
        panic!("unexpected outcome");
    };

    let goal = game.state.map.tile_center(52, 52);
    game.issue_lab_command_as(
        1,
        WireCommand::Move {
            units: vec![scout_id],
            x: goal.0,
            y: goal.1,
            queued: false,
        },
        LabCommandOptions::default(),
    )
    .expect("move command should be accepted");
    game.tick();
    assert!(
        game.pathing_cache_len_for_test() > 0,
        "move command should warm the reusable pathing cache"
    );

    let moved = game.state.map.tile_center(34, 34);
    game.apply_lab_op(LabOp::MoveEntity(LabMoveEntity {
        entity_id: scout_id,
        x: moved.0,
        y: moved.1,
    }))
    .expect("lab move should repair derived state");

    assert_eq!(
        game.pathing_cache_len_for_test(),
        0,
        "world-changing lab repair should clear rebuildable pathing cache"
    );
    assert!(game
        .snapshot_full_for(1)
        .entities
        .iter()
        .any(|entity| { entity.id == scout_id && entity.x == moved.0 && entity.y == moved.1 }));
}

fn derived_state_pathing_fixture() -> (Game, u32, (f32, f32), (f32, f32)) {
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
        Game::new_for_replay_with_starting_resources(&players, 5_000, 5_000, 0x5150_0500);
    for tile in &mut game.state.map.terrain {
        *tile = terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }

    for (tx, ty) in pathing_obstacle_tiles() {
        let index = game.state.map.index(tx, ty);
        game.state.map.terrain[index] = terrain::ROCK;
    }

    let start = game.state.map.tile_center(3, 12);
    let goal = game.state.map.tile_center(20, 12);
    let tank = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank should spawn");
    let enemy_pos = game.state.map.tile_center(20, 15);
    game.state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    let resource_pos = game.state.map.tile_center(8, 18);
    game.state
        .entities
        .spawn_node(EntityKind::Steel, resource_pos.0, resource_pos.1)
        .expect("resource node should spawn");

    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.clear_and_rebuild_derived_state_for_test();
    let player_ids = player_ids(&game);
    game.recompute_live_fog(&player_ids);
    game.refresh_building_memory(&player_ids);
    game.refresh_trench_memory(&player_ids);
    game.assert_invariants();

    (game, tank, goal, start)
}

fn derived_state_lab_fixture() -> Game {
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
    let size = 64;
    let map = Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(16, 16), (48, 48)],
        base_sites: Vec::new(),
    };
    let metadata = MapMetadata {
        name: "Derived State Lab".to_string(),
        schema_version: crate::game::map::CURRENT_MAP_VERSION,
        content_hash: "derived-state-lab".to_string(),
    };
    Game::new_lab(&players, 0x5150_0501, map, metadata)
}

fn pathing_obstacle_tiles() -> Vec<(u32, u32)> {
    vec![
        (6, 6),
        (6, 11),
        (6, 15),
        (6, 19),
        (7, 4),
        (7, 6),
        (7, 17),
        (8, 5),
        (8, 14),
        (8, 15),
        (8, 16),
        (9, 4),
        (9, 8),
        (9, 12),
        (9, 16),
        (10, 11),
        (10, 12),
        (10, 14),
        (11, 14),
        (11, 15),
        (12, 4),
        (12, 8),
        (12, 10),
        (13, 13),
        (13, 14),
        (13, 16),
        (14, 4),
        (14, 8),
        (14, 10),
        (14, 16),
        (14, 17),
        (15, 5),
        (15, 6),
        (15, 10),
        (15, 14),
        (15, 15),
        (16, 4),
        (16, 6),
        (16, 9),
        (16, 10),
        (16, 12),
        (16, 14),
        (17, 4),
        (17, 14),
        (17, 16),
        (17, 18),
    ]
}

fn enqueue_pair(baseline: &mut Game, wiped: &mut Game, player: u32, command: Command) {
    baseline.enqueue(player, command.clone());
    wiped.enqueue(player, command);
}

fn phase5_players() -> [PlayerInit; 2] {
    [
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
    ]
}
