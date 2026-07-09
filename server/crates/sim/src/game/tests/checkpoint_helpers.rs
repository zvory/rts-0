use super::*;
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
    panzerfaust_shots: String,
    observer_analysis: String,
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
    selected_spectator_snapshots: Vec<(u32, Snapshot)>,
    spectator_snapshot: Snapshot,
    debug_path_snapshots: Vec<(u32, Snapshot)>,
    debug_path_full_snapshots: Vec<(u32, Snapshot)>,
    debug_path_selected_spectator_snapshots: Vec<(u32, Snapshot)>,
    debug_path_spectator_snapshot: Snapshot,
}

pub(super) fn tick_pair_and_assert_equivalent(
    baseline: &mut Game,
    restored: &mut Game,
    label: &str,
) -> Vec<(u32, Vec<Event>)> {
    let baseline_events = baseline.tick();
    let restored_events = restored.tick();
    assert_eq!(baseline_events, restored_events, "{label}: events diverged");
    assert_equivalent_games(baseline, restored, label);
    baseline_events
}

pub(super) fn tick_pair_for(baseline: &mut Game, restored: &mut Game, ticks: u32, label: &str) {
    for tick in 0..ticks {
        tick_pair_and_assert_equivalent(baseline, restored, &format!("{label} tick {tick}"));
    }
}

pub(super) fn assert_equivalent_games(baseline: &Game, restored: &Game, label: &str) {
    assert_eq!(
        semantic_game_view(baseline),
        semantic_game_view(restored),
        "{label}: semantic authoritative state diverged"
    );
    assert_eq!(
        projection_view(baseline),
        projection_view(restored),
        "{label}: fog-filtered snapshots diverged"
    );
}

pub(super) fn restore_checkpoint_and_assert_equivalent(baseline: &Game, label: &str) -> Game {
    let checkpoint_next_id = baseline.state.entities.next_id_for_test();
    let checkpoint_pathing_config = baseline.pathing_config_for_test();
    let checkpoint_text = checkpoint_payload_text_for(baseline, label);
    assert!(
        !checkpoint_text.contains("\"terrain\""),
        "{label}: GameCheckpointV1 payload must not embed map terrain"
    );
    let restored = Game::restore_checkpoint_payload_text_for_test(
        &checkpoint_text,
        baseline.state.map.clone(),
        baseline.map_metadata().clone(),
    )
    .unwrap_or_else(|err| panic!("{label}: payload import failed: {err}"));
    let normalized_text = checkpoint_payload_text_for(&restored, label);
    assert_eq!(
        checkpoint_text, normalized_text,
        "{label}: checkpoint payload should have stable normalized output"
    );
    assert_eq!(
        restored.pathing_cache_len_for_test(),
        0,
        "{label}: checkpoint import must rebuild DerivedState instead of serializing pathing cache entries"
    );
    assert_eq!(
        checkpoint_pathing_config,
        restored.pathing_config_for_test(),
        "{label}: checkpoint import should preserve live pathing budget/cache configuration"
    );
    assert_eq!(
        checkpoint_next_id,
        restored.state.entities.next_id_for_test(),
        "{label}: entity allocator high-water state should survive checkpoint import"
    );
    assert_final_spatial_matches_entities(&restored);
    assert_equivalent_games(baseline, &restored, label);
    restored
}

pub(super) fn checkpoint_payload_text_for(game: &Game, label: &str) -> String {
    game.checkpoint_payload_text_for_test()
        .unwrap_or_else(|err| panic!("{label}: payload export failed: {err}"))
}

pub(super) fn repair_after_authoritative_test_spawn(game: &mut Game) {
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.clear_and_rebuild_derived_state_for_test();
    let ids = player_ids(game);
    game.recompute_live_fog(&ids);
    game.refresh_building_memory(&ids);
    game.refresh_trench_memory(&ids);
    game.assert_invariants();
}

pub(super) fn assert_debug_path_visible(game: &Game, player: u32, entity_id: u32, label: &str) {
    let snapshot = game.snapshot_for_with_options(player, owner_debug_path_options());
    let view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == entity_id)
        .unwrap_or_else(|| panic!("{label}: moving entity {entity_id} should be visible"));
    let debug_path = view
        .debug_path
        .as_ref()
        .unwrap_or_else(|| panic!("{label}: debug path should be projected"));
    assert!(
        debug_path.total_waypoints > 0,
        "{label}: debug path should include selected waypoints"
    );
}

pub(super) fn player_ids(game: &Game) -> Vec<u32> {
    game.state.players.iter().map(|player| player.id).collect()
}

pub(super) fn assert_final_spatial_matches_entities(game: &Game) {
    let mut spatial_ids = game.final_spatial().all_ids().collect::<Vec<_>>();
    spatial_ids.sort_unstable();
    assert_eq!(
        game.state.entities.ids(),
        spatial_ids,
        "rebuilt final spatial index should cover every live entity id"
    );
}

fn semantic_game_view(game: &Game) -> SemanticGameView {
    let players = game
        .state
        .players
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
    let entities = game
        .state
        .entities
        .iter()
        .map(|entity| (entity.id, format!("{entity:?}")))
        .collect();
    let building_memory = player_ids(game)
        .into_iter()
        .map(|player| {
            let mut entries = game
                .state
                .building_memory
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
        pending_commands: game
            .state
            .pending
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
        active_construction_sites: game
            .state
            .active_construction_sites
            .iter()
            .copied()
            .collect(),
        lab_god_mode_players: game.state.lab_god_mode_players.iter().copied().collect(),
        building_memory,
        lingering_sight: format!("{:?}", game.state.lingering_sight),
        firing_reveals: format!("{:?}", game.state.firing_reveals),
        smokes: format!("{:?}", game.state.smokes),
        trenches: format!("{:?}", game.state.trenches),
        ability_runtime: format!("{:?}", game.state.ability_runtime),
        mortar_shells: format!("{:?}", game.state.mortar_shells),
        artillery_shells: format!("{:?}", game.state.artillery_shells),
        panzerfaust_shots: format!("{:?}", game.state.panzerfaust_shots),
        observer_analysis: format!("{:?}", game.observer_analysis()),
    }
}

fn projection_view(game: &Game) -> ProjectionView {
    let player_ids = player_ids(game);
    let owner_debug_options = owner_debug_path_options();
    let full_debug_options = all_projected_debug_path_options();
    ProjectionView {
        snapshots: player_ids
            .iter()
            .map(|&player| (player, game.snapshot_for(player)))
            .collect(),
        full_snapshots: player_ids
            .iter()
            .map(|&player| (player, game.snapshot_full_for(player)))
            .collect(),
        selected_spectator_snapshots: player_ids
            .iter()
            .map(|&player| (player, game.snapshot_for_spectator(&[player])))
            .collect(),
        spectator_snapshot: game.snapshot_for_spectator(&player_ids),
        debug_path_snapshots: player_ids
            .iter()
            .map(|&player| {
                (
                    player,
                    game.snapshot_for_with_options(player, owner_debug_options),
                )
            })
            .collect(),
        debug_path_full_snapshots: player_ids
            .iter()
            .map(|&player| {
                (
                    player,
                    game.snapshot_full_for_with_options(player, full_debug_options),
                )
            })
            .collect(),
        debug_path_selected_spectator_snapshots: player_ids
            .iter()
            .map(|&player| {
                (
                    player,
                    game.snapshot_for_spectator_with_options(&[player], full_debug_options),
                )
            })
            .collect(),
        debug_path_spectator_snapshot: game
            .snapshot_for_spectator_with_options(&player_ids, full_debug_options),
    }
}

fn owner_debug_path_options() -> SnapshotOptions {
    SnapshotOptions {
        include_movement_paths: true,
        movement_paths_for_all_projected: false,
    }
}

fn all_projected_debug_path_options() -> SnapshotOptions {
    SnapshotOptions {
        include_movement_paths: true,
        movement_paths_for_all_projected: true,
    }
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
