use super::lab::{LabCommandOptions, LabMoveEntity, LabOp, LabOpOutcome, LabSpawnEntity};
use super::*;
use crate::protocol::Command as WireCommand;
use rand::RngCore;

#[derive(Debug, PartialEq)]
struct SemanticGameView {
    tick: u32,
    seed: u32,
    map_size: u32,
    map_terrain: Vec<u8>,
    map_metadata: MapMetadata,
    starting_loadouts: Vec<PlayerStartingLoadout>,
    next_entity_id: u32,
    rng_probe: [u64; 4],
    pending_commands: Vec<String>,
    players: Vec<SemanticPlayerView>,
    entities: Vec<(u32, String)>,
    command_log: Vec<super::replay::CommandLogEntry>,
    fog_visible_tiles: Vec<(u32, Vec<u8>)>,
    scores: Vec<PlayerScore>,
    active_construction_sites: Vec<u32>,
    lab_god_mode_players: Vec<u32>,
    building_memory: Vec<(u32, Vec<BuildingMemoryEntry>)>,
    lingering_sight: String,
    firing_reveals: String,
    smokes: String,
    trenches: String,
    ability_runtime: String,
    mortar_shells: String,
    artillery_shells: String,
}

#[derive(Debug, PartialEq)]
struct SemanticPlayerView {
    id: u32,
    team_id: TeamId,
    faction_id: String,
    name: String,
    color: String,
    start_tile: (u32, u32),
    steel: u32,
    oil: u32,
    supply_used: u32,
    supply_cap: u32,
    is_ai: bool,
    score: String,
    upgrades: Vec<String>,
}

#[derive(Debug, PartialEq)]
struct ProjectionView {
    snapshots: Vec<(u32, Snapshot)>,
    full_snapshots: Vec<(u32, Snapshot)>,
    spectator_snapshot: Snapshot,
}

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
        !baseline.state.entities
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
    let checkpoint_next_id = baseline.state.entities.next_id_for_test();
    let checkpoint_pathing_config = baseline.pathing_config_for_test();
    let checkpoint = baseline.checkpoint_for_test();
    let mut restored = Game::restore_checkpoint_for_test(checkpoint);

    assert_eq!(
        restored.pathing_cache_len_for_test(),
        0,
        "checkpoint import must rebuild DerivedState instead of serializing pathing cache entries"
    );
    assert_eq!(
        checkpoint_pathing_config,
        restored.pathing_config_for_test(),
        "checkpoint import should use the same live pathing budget/cache capacity"
    );
    assert_eq!(
        checkpoint_next_id,
        restored.state.entities.next_id_for_test(),
        "entity allocator high-water state should survive checkpoint import"
    );
    assert_final_spatial_matches_entities(&restored);
    assert_equivalent_games(&baseline, &restored, "after cold checkpoint import");

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
    let tank = game.state.entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank should spawn");
    let enemy_pos = game.state.map.tile_center(20, 15);
    game.state.entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    let resource_pos = game.state.map.tile_center(8, 18);
    game.state.entities
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
        expansion_sites: Vec::new(),
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

fn tick_pair_and_assert_equivalent(baseline: &mut Game, wiped: &mut Game, label: &str) {
    let baseline_events = baseline.tick();
    let wiped_events = wiped.tick();
    assert_eq!(baseline_events, wiped_events, "{label}: events diverged");
    assert_equivalent_games(baseline, wiped, label);
}

fn assert_equivalent_games(baseline: &Game, wiped: &Game, label: &str) {
    assert_eq!(
        semantic_game_view(baseline),
        semantic_game_view(wiped),
        "{label}: semantic authoritative state diverged"
    );
    assert_eq!(
        projection_view(baseline),
        projection_view(wiped),
        "{label}: fog-filtered snapshots diverged"
    );
}

fn assert_final_spatial_matches_entities(game: &Game) {
    let mut spatial_ids = game.final_spatial().all_ids().collect::<Vec<_>>();
    spatial_ids.sort_unstable();
    assert_eq!(
        game.state.entities.ids(),
        spatial_ids,
        "rebuilt final spatial index should cover every live entity id"
    );
}

fn repair_after_authoritative_test_spawn(game: &mut Game) {
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.clear_and_rebuild_derived_state_for_test();
    let ids = player_ids(game);
    game.recompute_live_fog(&ids);
    game.refresh_building_memory(&ids);
    game.refresh_trench_memory(&ids);
    game.assert_invariants();
}

fn semantic_game_view(game: &Game) -> SemanticGameView {
    let players = game.state.players
        .iter()
        .map(|player| SemanticPlayerView {
            id: player.id,
            team_id: player.team_id,
            faction_id: player.faction_id.clone(),
            name: player.name.clone(),
            color: player.color.clone(),
            start_tile: player.start_tile,
            steel: player.steel,
            oil: player.oil,
            supply_used: player.supply_used,
            supply_cap: player.supply_cap,
            is_ai: player.is_ai,
            score: format!("{:?}", player.score),
            upgrades: player
                .upgrades
                .iter()
                .map(|upgrade| format!("{upgrade:?}"))
                .collect(),
        })
        .collect();
    let entities = game.state.entities
        .iter()
        .map(|entity| (entity.id, format!("{entity:?}")))
        .collect();
    let building_memory = player_ids(game)
        .into_iter()
        .map(|player| {
            let mut entries = game.state.building_memory
                .entries_for_player_for_test(player)
                .cloned()
                .collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.id);
            (player, entries)
        })
        .collect();

    SemanticGameView {
        tick: game.tick_count(),
        seed: game.seed(),
        map_size: game.state.map.size,
        map_terrain: game.state.map.terrain.clone(),
        map_metadata: game.map_metadata().clone(),
        starting_loadouts: game.starting_loadouts().to_vec(),
        next_entity_id: game.state.entities.next_id_for_test(),
        rng_probe: rng_probe(game),
        pending_commands: game.state.pending
            .iter()
            .map(|pending| format!("{pending:?}"))
            .collect(),
        players,
        entities,
        command_log: game.command_log().to_vec(),
        fog_visible_tiles: player_ids(game)
            .into_iter()
            .map(|player| (player, game.state.fog.visible_tiles_for(player)))
            .collect(),
        scores: game.scores(),
        active_construction_sites: game.state.active_construction_sites.iter().copied().collect(),
        lab_god_mode_players: game.state.lab_god_mode_players.iter().copied().collect(),
        building_memory,
        lingering_sight: format!("{:?}", game.state.lingering_sight),
        firing_reveals: format!("{:?}", game.state.firing_reveals),
        smokes: format!("{:?}", game.state.smokes),
        trenches: format!("{:?}", game.state.trenches),
        ability_runtime: format!("{:?}", game.state.ability_runtime),
        mortar_shells: format!("{:?}", game.state.mortar_shells),
        artillery_shells: format!("{:?}", game.state.artillery_shells),
    }
}

fn projection_view(game: &Game) -> ProjectionView {
    let player_ids = player_ids(game);
    ProjectionView {
        snapshots: player_ids
            .iter()
            .map(|&player| (player, game.snapshot_for(player)))
            .collect(),
        full_snapshots: player_ids
            .iter()
            .map(|&player| (player, game.snapshot_full_for(player)))
            .collect(),
        spectator_snapshot: game.snapshot_for_spectator(&player_ids),
    }
}

fn player_ids(game: &Game) -> Vec<u32> {
    game.state.players.iter().map(|player| player.id).collect()
}

fn rng_probe(game: &Game) -> [u64; 4] {
    let mut rng = game.state.rng.clone();
    [
        rng.next_u64(),
        rng.next_u64(),
        rng.next_u64(),
        rng.next_u64(),
    ]
}
