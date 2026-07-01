use super::*;

#[derive(Debug, PartialEq)]
struct SemanticGameView {
    tick: u32,
    seed: u32,
    map_size: u32,
    map_terrain: Vec<u8>,
    map_metadata: MapMetadata,
    starting_loadouts: Vec<PlayerStartingLoadout>,
    next_entity_id: u32,
    players: Vec<SemanticPlayerView>,
    entities: Vec<(u32, String)>,
    command_log: Vec<super::replay::CommandLogEntry>,
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
    tick_pair_and_assert_equivalent(&mut baseline, &mut wiped, "warm path cache tick");

    assert!(
        baseline.pathing.cache_len() > 0,
        "pathing-heavy setup should warm the baseline path cache before the wipe"
    );
    assert_eq!(
        baseline.pathing.cache_len(),
        wiped.pathing.cache_len(),
        "paired games should warm the same cache entries before the wipe"
    );
    assert!(
        !baseline
            .entities
            .get(tank)
            .expect("tank should survive")
            .path_is_empty(),
        "the selected movement path must live on the entity, not only in the pathing cache"
    );

    wiped.clear_and_rebuild_derived_state_for_test();
    assert_eq!(
        wiped.pathing.cache_len(),
        0,
        "the derived-state wipe should clear the persistent pathing cache"
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
    tick_pair_and_assert_equivalent(&mut baseline, &mut wiped, "post-wipe repath tick");
    assert!(
        wiped.pathing.cache_len() > 0,
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
    for tile in &mut game.map.terrain {
        *tile = terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }

    for (tx, ty) in pathing_obstacle_tiles() {
        let index = game.map.index(tx, ty);
        game.map.terrain[index] = terrain::ROCK;
    }

    let start = game.map.tile_center(3, 12);
    let goal = game.map.tile_center(20, 12);
    let tank = game
        .entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank should spawn");
    let enemy_pos = game.map.tile_center(20, 15);
    game.entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    let resource_pos = game.map.tile_center(8, 18);
    game.entities
        .spawn_node(EntityKind::Steel, resource_pos.0, resource_pos.1)
        .expect("resource node should spawn");

    systems::recompute_supply(&mut game.players, &game.entities);
    game.clear_and_rebuild_derived_state_for_test();
    let player_ids = player_ids(&game);
    game.recompute_live_fog(&player_ids);
    game.refresh_building_memory(&player_ids);
    game.refresh_trench_memory(&player_ids);
    game.assert_invariants();

    (game, tank, goal, start)
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

fn semantic_game_view(game: &Game) -> SemanticGameView {
    let players = game
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
        .entities
        .iter()
        .map(|entity| (entity.id, format!("{entity:?}")))
        .collect();
    let building_memory = player_ids(game)
        .into_iter()
        .map(|player| {
            let mut entries = game
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
        map_size: game.map.size,
        map_terrain: game.map.terrain.clone(),
        map_metadata: game.map_metadata().clone(),
        starting_loadouts: game.starting_loadouts().to_vec(),
        next_entity_id: game.entities.next_id_for_test(),
        players,
        entities,
        command_log: game.command_log().to_vec(),
        scores: game.scores(),
        active_construction_sites: game.active_construction_sites.iter().copied().collect(),
        lab_god_mode_players: game.lab_god_mode_players.iter().copied().collect(),
        building_memory,
        lingering_sight: format!("{:?}", game.lingering_sight),
        firing_reveals: format!("{:?}", game.firing_reveals),
        smokes: format!("{:?}", game.smokes),
        trenches: format!("{:?}", game.trenches),
        ability_runtime: format!("{:?}", game.ability_runtime),
        mortar_shells: format!("{:?}", game.mortar_shells),
        artillery_shells: format!("{:?}", game.artillery_shells),
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
    game.players.iter().map(|player| player.id).collect()
}
