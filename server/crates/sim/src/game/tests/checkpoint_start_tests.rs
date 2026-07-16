use super::checkpoint_helpers::{
    assert_equivalent_games, tick_pair_and_assert_equivalent, tick_pair_for,
};
use super::*;
use crate::game::checkpoint::CheckpointPayloadError;
use crate::game::replay::ReplayStartComposition;
use crate::protocol::terrain;

fn default_map_metadata() -> MapMetadata {
    Map::metadata_for_name("Chokes").expect("Chokes map metadata should load")
}

fn team_faction_ai_players() -> [PlayerInit; 3] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Alpha".to_string(),
            color: "#cc1111".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "ekat".to_string(),
            name: "Bravo".to_string(),
            color: "#1133bb".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Computer".to_string(),
            color: "#22aa55".to_string(),
            is_ai: true,
        },
    ]
}

fn two_human_players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".to_string(),
            color: "#cc1111".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".to_string(),
            color: "#1133bb".to_string(),
            is_ai: false,
        },
    ]
}

fn load_default_map_for(players: &[PlayerInit], seed: u32) -> (Map, MapMetadata) {
    let start_players: Vec<_> = players
        .iter()
        .map(|player| (player.id, player.team_id))
        .collect();
    let map = Map::load_for_players("Chokes", &start_players, seed)
        .expect("Chokes map should load for test players");
    (map, default_map_metadata())
}

fn assert_checkpoint_start_matches_direct(
    mut direct: Game,
    mut checkpoint_backed: Game,
    label: &str,
) -> (Game, Game) {
    assert_equivalent_games(&direct, &checkpoint_backed, label);
    tick_pair_for(&mut direct, &mut checkpoint_backed, 3, label);
    (direct, checkpoint_backed)
}

fn owned_kind_ids(game: &Game, owner: u32, kind: EntityKind) -> Vec<u32> {
    game.state
        .entities
        .iter()
        .filter(|entity| entity.owner == owner && entity.kind == kind)
        .map(|entity| entity.id)
        .collect()
}

#[test]
fn checkpoint_start_matches_direct_generated_team_faction_ai_setup() {
    let players = team_faction_ai_players();
    let seed = 0x5150_3001;
    let direct =
        Game::new_direct_start_for_test(&players, None, seed, None, None, default_map_metadata());
    let checkpoint_backed = Game::new(&players, seed);

    assert_checkpoint_start_matches_direct(
        direct,
        checkpoint_backed,
        "generated team/faction/AI checkpoint start",
    );
}

#[test]
fn checkpoint_start_matches_direct_authored_map_setup() {
    let players = two_human_players();
    let seed = 0x5150_3002;
    let (map, metadata) = load_default_map_for(&players, seed);
    let direct = Game::new_direct_start_for_test(
        &players,
        None,
        seed,
        None,
        Some(map.clone()),
        metadata.clone(),
    );
    let checkpoint_backed =
        Game::new_with_random_ai_profiles_and_map_metadata(&players, seed, map, metadata);

    assert_checkpoint_start_matches_direct(
        direct,
        checkpoint_backed,
        "authored map checkpoint start",
    );
}

#[test]
fn checkpoint_start_preserves_replay_loadouts_and_artifact_metadata() {
    let players = two_human_players();
    let seed = 0x5150_3003;
    let loadouts = [
        PlayerStartingLoadout {
            player_id: 1,
            faction_id: "kriegsia".to_string(),
            loadout_id: "kriegsia.standard".to_string(),
            starting_steel: 777,
            starting_oil: 333,
        },
        PlayerStartingLoadout {
            player_id: 2,
            faction_id: "kriegsia".to_string(),
            loadout_id: "kriegsia.standard".to_string(),
            starting_steel: 888,
            starting_oil: 444,
        },
    ];
    let direct = Game::new_direct_start_for_test(
        &players,
        None,
        seed,
        Some(&loadouts),
        None,
        default_map_metadata(),
    );
    let checkpoint_backed = Game::new_for_replay_with_starting_loadouts(&players, &loadouts, seed);
    let (mut direct, mut checkpoint_backed) = assert_checkpoint_start_matches_direct(
        direct,
        checkpoint_backed,
        "replay loadout checkpoint start",
    );
    let direct_replay_start =
        ReplayStartComposition::capture(&direct, "test-sha").expect("direct replay start");
    let checkpoint_replay_start = ReplayStartComposition::capture(&checkpoint_backed, "test-sha")
        .expect("checkpoint-backed replay start");

    let worker_ids = owned_kind_ids(&direct, 1, EntityKind::Worker);
    direct.enqueue(
        1,
        SimCommand::Stop {
            units: worker_ids.clone(),
        },
    );
    checkpoint_backed.enqueue(1, SimCommand::Stop { units: worker_ids });
    tick_pair_and_assert_equivalent(
        &mut direct,
        &mut checkpoint_backed,
        "replay command-log continuation after checkpoint start",
    );

    let direct_artifact = direct_replay_start.finalize(&direct, Some(1), direct.scores());
    let checkpoint_artifact =
        checkpoint_replay_start.finalize(&checkpoint_backed, Some(1), checkpoint_backed.scores());
    assert_eq!(
        direct_artifact, checkpoint_artifact,
        "checkpoint-backed starts should not change replay artifact start metadata or command log"
    );
}

#[test]
fn checkpoint_start_matches_direct_blank_lab_setup() {
    let players = two_human_players();
    let seed = 0x5150_3004;
    let (map, metadata) = load_default_map_for(&players, seed);
    let direct = Game::new_direct_start_for_test(
        &players,
        None,
        seed,
        None,
        Some(map.clone()),
        metadata.clone(),
    );
    let checkpoint_backed = Game::new_lab(&players, seed, map, metadata);

    assert_checkpoint_start_matches_direct(direct, checkpoint_backed, "blank lab checkpoint start");
}

#[test]
fn checkpoint_start_rejects_mismatched_map_before_restore() {
    let players = two_human_players();
    let game = Game::new(&players, 0x5150_3005);
    let text = game
        .checkpoint_payload_text_for_test()
        .expect("checkpoint-backed start should export");
    let mut wrong_map = game.state.map.clone();
    wrong_map.terrain[0] = if wrong_map.terrain[0] == terrain::ROCK {
        terrain::GRASS
    } else {
        terrain::ROCK
    };

    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &text,
            wrong_map,
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::MapBindingMismatch {
            field: "materializedMapHash"
        })
    ));
}
