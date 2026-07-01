use super::checkpoint_helpers::{
    checkpoint_payload_text_for, restore_checkpoint_and_assert_equivalent,
    tick_pair_and_assert_equivalent,
};
use super::fixtures::human_vs_ai_players;
use super::*;
use crate::game::checkpoint::CheckpointPayloadError;

#[test]
fn checkpoint_payload_round_trips_through_text_and_normalizes_output() {
    let mut baseline = Game::new_for_replay(&human_vs_ai_players(), 0x5150_2001);
    baseline.tick();

    let mut restored =
        restore_checkpoint_and_assert_equivalent(&baseline, "basic payload text round trip");
    tick_pair_and_assert_equivalent(
        &mut baseline,
        &mut restored,
        "basic payload continuation after text import",
    );
}

#[test]
fn checkpoint_payload_rejects_corrupt_oversized_and_unsupported_text() {
    let game = Game::new_for_replay(&human_vs_ai_players(), 0x5150_2002);
    let text = checkpoint_payload_text_for(&game, "negative payload fixture");

    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            "{",
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::MalformedJson(_))
    ));
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &" ".repeat(5 * 1024 * 1024),
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::PayloadTooLarge { .. })
    ));

    let unsupported_version = mutate_payload(&text, |value| {
        value["version"] = serde_json::json!(99);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &unsupported_version,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::UnsupportedVersion { found: 99 })
    ));

    let unsupported_feature = mutate_payload(&text, |value| {
        value["compatibility"]["requiredFeatures"] =
            serde_json::json!(["future-authoritative-field"]);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &unsupported_feature,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::UnsupportedRequiredFeature { .. })
    ));
}

#[test]
fn checkpoint_payload_rejects_wrong_map_and_invalid_state_references() {
    let game = Game::new_for_replay(&human_vs_ai_players(), 0x5150_2003);
    let text = checkpoint_payload_text_for(&game, "invalid state payload fixture");

    let mut wrong_map = game.state.map.clone();
    wrong_map.terrain[0] = terrain::ROCK;
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

    let duplicate_entity = mutate_payload(&text, |value| {
        let first = value["entities"]["entities"][0].clone();
        value["entities"]["entities"]
            .as_array_mut()
            .expect("entities array")
            .push(first);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &duplicate_entity,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::DuplicateId {
            field: "entities",
            ..
        })
    ));

    let stale_owner = mutate_payload(&text, |value| {
        value["entities"]["entities"][0]["owner"] = serde_json::json!(999);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &stale_owner,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidReference {
            field: "entities.owner",
            id: 999
        })
    ));
}

#[test]
fn checkpoint_payload_rejects_invalid_coordinates_and_queue_caps() {
    let game = Game::new_for_replay(&human_vs_ai_players(), 0x5150_2004);
    let text = checkpoint_payload_text_for(&game, "invalid coordinate payload fixture");

    let out_of_bounds = mutate_payload(&text, |value| {
        value["entities"]["entities"][0]["pos_x"] = serde_json::json!(-1.0);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &out_of_bounds,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidValue {
            field: "entities.position"
        })
    ));

    let oversized_pending = mutate_payload(&text, |value| {
        let command = serde_json::json!({
            "player": 1,
            "command": { "Rejected": { "reason": "Unit" } },
            "admission": "Normal"
        });
        value["pendingCommands"] = serde_json::Value::Array(vec![command; 1_025]);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &oversized_pending,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::CountCapExceeded {
            field: "pendingCommands",
            ..
        })
    ));
}

fn mutate_payload(text: &str, mutate: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut value: serde_json::Value = serde_json::from_str(text).expect("valid checkpoint JSON");
    mutate(&mut value);
    serde_json::to_string(&value).expect("mutated checkpoint JSON")
}
