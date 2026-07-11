use super::checkpoint_helpers::{assert_equivalent_games, tick_pair_for};
use super::*;
use crate::game::lab::{
    LabEntityIdRemap, LabError, LabOp, LabSetCompletedResearch, LabSetPlayerResources,
    LabSpawnEntity, LAB_CHECKPOINT_SCENARIO_V1_SCHEMA_VERSION,
};
use crate::game::upgrade::UpgradeKind;

const TEST_BUILD_SHA: &str = "checkpoint-lab-test";

fn lab_players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Lab Alpha".to_string(),
            color: "#cc1111".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Lab Bravo".to_string(),
            color: "#1133bb".to_string(),
            is_ai: false,
        },
    ]
}

fn default_lab_game(seed: u32) -> Game {
    let players = lab_players();
    let start_players: Vec<_> = players
        .iter()
        .map(|player| (player.id, player.team_id))
        .collect();
    let map = Map::load_for_players("Default", &start_players, seed)
        .expect("Default map should load for lab checkpoint tests");
    let metadata = Map::metadata_for_name("Default").expect("Default metadata should load");
    Game::new_lab(&players, seed, map, metadata)
}

fn free_spawn_position(game: &Game, owner: u32, kind: EntityKind) -> (f32, f32) {
    for y in 2..game.state.map.size.saturating_sub(2) {
        for x in 2..game.state.map.size.saturating_sub(2) {
            if !game.state.map.is_passable(x as i32, y as i32) {
                continue;
            }
            let (px, py) = game.state.map.tile_center(x, y);
            let mut trial = game.clone_for_replay_keyframe();
            if trial
                .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                    owner,
                    kind,
                    x: px,
                    y: py,
                    completed: true,
                }))
                .is_ok()
            {
                return (px, py);
            }
        }
    }
    panic!("no free spawn position for {kind:?}");
}

fn assert_restore_invalid_scenario(
    scenario: crate::game::lab::LabCheckpointScenarioV1,
    expected: &str,
) {
    match Game::restore_lab_checkpoint_scenario(scenario) {
        Ok(_) => panic!("checkpoint lab scenario restore should reject {expected}"),
        Err(LabError::InvalidScenario { reason }) => {
            assert!(reason.contains(expected), "unexpected error: {reason}");
        }
        Err(err) => panic!("unexpected error for {expected}: {err:?}"),
    }
}

fn assert_restore_invalid_map(scenario: crate::game::lab::LabCheckpointScenarioV1, expected: &str) {
    match Game::restore_lab_checkpoint_scenario(scenario) {
        Ok(_) => panic!("checkpoint lab scenario restore should reject {expected}"),
        Err(LabError::InvalidMap { reason, .. }) => {
            assert!(reason.contains(expected), "unexpected error: {reason}");
        }
        Err(err) => panic!("unexpected error for {expected}: {err:?}"),
    }
}

#[test]
fn checkpoint_lab_scenario_export_matches_direct_state() {
    let mut authored = default_lab_game(0x5150_5001);
    authored
        .apply_lab_op(LabOp::SetPlayerResources(LabSetPlayerResources {
            player_id: 1,
            steel: 777,
            oil: 66,
        }))
        .expect("resources should update");
    authored
        .apply_lab_op(LabOp::SetCompletedResearch(LabSetCompletedResearch {
            player_id: 1,
            upgrade: UpgradeKind::TankUnlock,
            completed: true,
        }))
        .expect("research should update");
    let (x, y) = free_spawn_position(&authored, 1, EntityKind::Tank);
    authored
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Tank,
            x,
            y,
            completed: true,
        }))
        .expect("tank should spawn");
    authored.tick();

    let checkpoint = authored
        .export_lab_checkpoint_scenario("Checkpoint Export Proof".to_string(), TEST_BUILD_SHA)
        .expect("checkpoint setup should export");
    assert_eq!(
        checkpoint.schema_version,
        LAB_CHECKPOINT_SCENARIO_V1_SCHEMA_VERSION
    );
    assert_eq!(checkpoint.kind, "labCheckpointScenario");
    assert_eq!(checkpoint.name, "Checkpoint Export Proof");
    assert_eq!(checkpoint.seed, 0x5150_5001);
    assert_eq!(checkpoint.metadata.exported_tick, authored.tick_count());
    assert_eq!(checkpoint.metadata.source_scenario, None);
    assert!(checkpoint
        .metadata
        .source_entity_id_map
        .iter()
        .all(|entry| entry.old_id == entry.new_id));
    assert_eq!(
        checkpoint.map.data.terrain.len(),
        (checkpoint.map.data.size * checkpoint.map.data.size) as usize
    );
    assert!(
        !checkpoint.checkpoint_payload.contains("\"terrain\""),
        "embedded GameCheckpointV1 must not duplicate map terrain"
    );
    let payload_json: serde_json::Value =
        serde_json::from_str(&checkpoint.checkpoint_payload).expect("checkpoint JSON");
    assert_eq!(payload_json["schema"], "rts.gameCheckpoint");
    assert_eq!(payload_json["compatibility"]["createdBy"], "lab");
    assert_eq!(
        payload_json["compatibility"]["serverBuildSha"],
        TEST_BUILD_SHA
    );

    let mut restored = Game::restore_lab_checkpoint_scenario(checkpoint)
        .expect("checkpoint scenario restore should succeed");
    assert_equivalent_games(
        &authored,
        &restored,
        "checkpoint-backed lab scenario restore",
    );
    let mut direct = authored;
    tick_pair_for(
        &mut direct,
        &mut restored,
        3,
        "checkpoint-backed lab scenario continuation",
    );
}

#[test]
fn lab_checkpoint_scenario_export_preserves_god_mode_and_rejects_map_mismatches() {
    let mut game = default_lab_game(0x5150_5003);
    game.apply_lab_op(LabOp::SetPlayerGodMode {
        player_id: 1,
        enabled: true,
    })
    .expect("god mode should update");
    game.tick();

    let checkpoint = game
        .export_lab_checkpoint_scenario("Untitled lab scenario".to_string(), TEST_BUILD_SHA)
        .expect("live lab checkpoint scenario should export");
    assert_eq!(checkpoint.metadata.exported_tick, game.tick_count());
    assert_eq!(checkpoint.metadata.source_scenario, None);

    let restored = Game::restore_lab_checkpoint_scenario(checkpoint.clone())
        .expect("live lab checkpoint scenario should restore");
    assert_eq!(restored.lab_god_mode_players(), vec![1]);
    assert_equivalent_games(&game, &restored, "live lab checkpoint scenario restore");

    let mut wrong_hash = checkpoint.clone();
    wrong_hash.map.content_hash = "wrong-content-hash".to_string();
    assert_restore_invalid_scenario(wrong_hash, "contentHash");

    let mut wrong_tick = checkpoint.clone();
    wrong_tick.metadata.exported_tick = wrong_tick.metadata.exported_tick.saturating_add(1);
    assert_restore_invalid_scenario(wrong_tick, "exportedTick");

    let mut wrong_map_data = checkpoint;
    wrong_map_data.map.data.terrain[0] = if wrong_map_data.map.data.terrain[0] == terrain::GRASS {
        terrain::ROCK
    } else {
        terrain::GRASS
    };
    match Game::restore_lab_checkpoint_scenario(wrong_map_data) {
        Ok(_) => panic!("checkpoint lab scenario restore should reject wrong materialized map"),
        Err(LabError::InvalidMap { reason, .. }) => {
            assert!(
                reason.contains("materialized hash"),
                "unexpected error: {reason}"
            );
        }
        Err(err) => panic!("unexpected error for wrong materialized map: {err:?}"),
    }
}

#[test]
fn lab_checkpoint_scenario_rejects_player_starts_that_disagree_with_its_map() {
    let game = default_lab_game(0x5150_5006);
    let mut checkpoint = game
        .export_lab_checkpoint_scenario("Untitled lab scenario".to_string(), TEST_BUILD_SHA)
        .expect("live lab checkpoint scenario should export");
    checkpoint.map.data.starts.swap(0, 1);
    let map = Map {
        size: checkpoint.map.data.size,
        terrain: checkpoint.map.data.terrain.clone(),
        starts: checkpoint
            .map
            .data
            .starts
            .iter()
            .map(|tile| (tile.x, tile.y))
            .collect(),
        base_sites: checkpoint
            .map
            .data
            .base_sites
            .iter()
            .map(|tile| (tile.x, tile.y))
            .collect(),
    };
    let materialized_hash = map.materialized_hash();
    checkpoint.map.materialized_hash = materialized_hash.clone();
    let mut payload: serde_json::Value = serde_json::from_str(&checkpoint.checkpoint_payload)
        .expect("checkpoint payload should be JSON");
    payload["mapBinding"]["materializedMapHash"] = serde_json::Value::String(materialized_hash);
    checkpoint.checkpoint_payload =
        serde_json::to_string(&payload).expect("checkpoint payload should serialize");

    assert_restore_invalid_scenario(
        checkpoint,
        "player start tiles do not match scenario map starts",
    );
}

#[test]
fn lab_checkpoint_scenario_restore_rejects_untrusted_source_entity_id_maps() {
    let game = default_lab_game(0x5150_5005);
    let checkpoint = game
        .export_lab_checkpoint_scenario("Untitled lab scenario".to_string(), TEST_BUILD_SHA)
        .expect("live lab checkpoint scenario should export");

    let mut missing_new_id = checkpoint.clone();
    missing_new_id.metadata.source_entity_id_map = vec![LabEntityIdRemap {
        old_id: 1,
        new_id: u32::MAX,
    }];
    assert_restore_invalid_scenario(
        missing_new_id,
        "sourceEntityIdMap newId must reference a restored entity",
    );

    let mut duplicate = checkpoint;
    let old_id = duplicate.metadata.source_entity_id_map[0].old_id;
    duplicate.metadata.source_entity_id_map[1].old_id = old_id;
    assert_restore_invalid_scenario(duplicate, "sourceEntityIdMap contains duplicate oldId");
}

#[test]
fn lab_checkpoint_scenario_restore_bounds_map_site_lists() {
    let game = default_lab_game(0x5150_5004);
    let checkpoint = game
        .export_lab_checkpoint_scenario("Untitled lab scenario".to_string(), TEST_BUILD_SHA)
        .expect("live lab checkpoint scenario should export");

    let mut no_starts = checkpoint.clone();
    no_starts.map.data.starts.clear();
    assert_restore_invalid_map(no_starts, "start site count");

    let mut too_many_starts = checkpoint.clone();
    let start = too_many_starts.map.data.starts[0];
    while too_many_starts.map.data.starts.len() <= 8 {
        too_many_starts.map.data.starts.push(start);
    }
    assert_restore_invalid_map(too_many_starts, "start site count");

    let mut too_many_base_sites = checkpoint;
    let base_site = too_many_base_sites.map.data.starts[0];
    while too_many_base_sites.map.data.base_sites.len() <= 64 {
        too_many_base_sites.map.data.base_sites.push(base_site);
    }
    assert_restore_invalid_map(too_many_base_sites, "base site count");
}
