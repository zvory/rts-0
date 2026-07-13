use super::checkpoint_helpers::{
    assert_equivalent_games, checkpoint_payload_text_for, restore_checkpoint_and_assert_equivalent,
    tick_pair_and_assert_equivalent, tick_pair_for,
};
use super::fixtures::human_vs_ai_players;
use super::panzerfaust_tests::{enqueue_attack, panzerfaust_fixture, player_events};
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
fn checkpoint_payload_round_trips_panzerfaust_shot_in_flight() {
    let (mut baseline, panzerfaust, tank) = panzerfaust_fixture();
    enqueue_attack(&mut baseline, panzerfaust, tank, false);
    tick_until_panzerfaust_launch(&mut baseline);

    let mut restored =
        restore_checkpoint_and_assert_equivalent(&baseline, "panzerfaust in-flight round trip");
    tick_pair_for(
        &mut baseline,
        &mut restored,
        crate::config::PANZERFAUST_TRAVEL_TICKS + 5,
        "panzerfaust in-flight continuation",
    );
}

#[test]
fn checkpoint_payload_backfills_legacy_panzerfaust_shot_in_flight() {
    let (mut baseline, panzerfaust, tank) = panzerfaust_fixture();
    enqueue_attack(&mut baseline, panzerfaust, tank, false);
    tick_until_panzerfaust_launch(&mut baseline);

    let text = checkpoint_payload_text_for(&baseline, "legacy panzerfaust in-flight fixture");
    let legacy_text = mutate_payload(&text, |value| {
        value
            .as_object_mut()
            .expect("checkpoint payload should be an object")
            .remove("panzerfaustShots");
    });
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &legacy_text,
        baseline.state.map.clone(),
        baseline.map_metadata().clone(),
    )
    .expect("legacy payload without panzerfaustShots should restore");

    assert_equivalent_games(&baseline, &restored, "legacy panzerfaust backfill");
    tick_pair_for(
        &mut baseline,
        &mut restored,
        crate::config::PANZERFAUST_TRAVEL_TICKS + 5,
        "legacy panzerfaust in-flight continuation",
    );
}

#[test]
fn checkpoint_payload_rejects_invalid_panzerfaust_shot_state() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    enqueue_attack(&mut game, panzerfaust, tank, false);
    tick_until_panzerfaust_launch(&mut game);
    let text = checkpoint_payload_text_for(&game, "invalid panzerfaust shot payload fixture");

    let stale_owner = mutate_payload(&text, |value| {
        value["panzerfaustShots"]["shots"][0]["owner"] = serde_json::json!(999);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &stale_owner,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidReference {
            field: "panzerfaustShots.owner",
            id: 999
        })
    ));

    let stale_target = mutate_payload(&text, |value| {
        value["panzerfaustShots"]["shots"][0]["target"] = serde_json::json!(999);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &stale_target,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidReference {
            field: "panzerfaustShots.target",
            id: 999
        })
    ));

    let out_of_world_impact = mutate_payload(&text, |value| {
        value["panzerfaustShots"]["shots"][0]["impact_x"] = serde_json::json!(-1.0);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &out_of_world_impact,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidValue {
            field: "panzerfaustShots.impact",
        })
    ));

    let already_due = mutate_payload(&text, |value| {
        value["panzerfaustShots"]["shots"][0]["impact_tick"] = value["tick"].clone();
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &already_due,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidValue {
            field: "panzerfaustShots.impactTick",
        })
    ));
}

#[test]
fn checkpoint_payload_rejects_invalid_tank_armor_reaction_lock() {
    let (mut game, attacker, tank) = panzerfaust_fixture();
    let source = game
        .state
        .entities
        .get(attacker)
        .map(|attacker| (attacker.pos_x, attacker.pos_y))
        .expect("attacker should exist");
    let reaction_tick = game.tick_count();
    game.state
        .entities
        .get_mut(tank)
        .expect("tank should exist")
        .lock_tank_armor_reaction_source(source, reaction_tick);
    let text = checkpoint_payload_text_for(&game, "invalid armor reaction lock fixture");

    let out_of_world_source = mutate_payload(&text, |value| {
        tank_armor_reaction_lock_mut(value, tank)["source_x"] = serde_json::json!(-1.0);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &out_of_world_source,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidValue {
            field: "entities.combat.tankArmorReactionLock.source",
        })
    ));

    let future_lock = mutate_payload(&text, |value| {
        tank_armor_reaction_lock_mut(value, tank)["acquired_tick"] =
            serde_json::json!(game.tick_count().saturating_add(1));
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &future_lock,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidValue {
            field: "entities.combat.tankArmorReactionLock.acquiredTick",
        })
    ));
}

#[test]
fn checkpoint_payload_serializes_entities_in_stable_id_order() {
    let game = Game::new_for_replay(&human_vs_ai_players(), 0x5150_2006);
    let text = checkpoint_payload_text_for(&game, "stable entity ordering fixture");
    let value: serde_json::Value = serde_json::from_str(&text).expect("valid checkpoint JSON");
    let ids = value["entities"]["entities"]
        .as_array()
        .expect("entity array")
        .iter()
        .map(|entity| entity["id"].as_u64().expect("entity id"))
        .collect::<Vec<_>>();
    let mut sorted = ids.clone();
    sorted.sort_unstable();

    assert_eq!(
        ids, sorted,
        "checkpoint payload should not depend on EntityStore HashMap iteration order"
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

    let mut oversized_export = game.clone();
    oversized_export.state.players[0].name = "x".repeat(5 * 1024 * 1024);
    assert!(matches!(
        oversized_export.checkpoint_payload_text_for_test(),
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

    let unsupported_protocol = mutate_payload(&text, |value| {
        value["compatibility"]["protocolVersion"] = serde_json::json!(99);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &unsupported_protocol,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidValue {
            field: "compatibility.protocolVersion",
        })
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

#[test]
fn checkpoint_payload_accepts_legacy_score_rows_without_resource_income_fields() {
    let game = Game::new_for_replay(&human_vs_ai_players(), 0x5150_2007);
    let text = checkpoint_payload_text_for(&game, "legacy score resource income fixture");
    let legacy_text = mutate_payload(&text, |value| {
        for player in value["players"].as_array_mut().expect("players array") {
            let score = player["score"].as_object_mut().expect("score object");
            score.remove("resourcesMined");
            score.remove("resourceIncomeHistory");
        }
    });

    let restored = Game::restore_checkpoint_payload_text_for_test(
        &legacy_text,
        game.state.map.clone(),
        game.map_metadata().clone(),
    )
    .expect("legacy score payload should default new resource-income fields");

    assert_equivalent_games(&game, &restored, "legacy score resource-income defaults");
    assert!(restored
        .observer_analysis()
        .players
        .iter()
        .all(|player| player.resources.lifetime.steel == 0
            && player.resources.lifetime.oil == 0
            && player.resources.last_5s.steel == 0
            && player.resources.last_5s.oil == 0
            && player.resources.last_minute.steel == 0
            && player.resources.last_minute.oil == 0));
}

#[test]
fn checkpoint_payload_rejects_invalid_resource_income_history() {
    let game = Game::new_for_replay(&human_vs_ai_players(), 0x5150_2008);
    let text = checkpoint_payload_text_for(&game, "invalid resource income history fixture");

    let future_income = mutate_payload(&text, |value| {
        value["players"][0]["score"]["resourceIncomeHistory"] = serde_json::json!([{
            "tick": game.tick_count().saturating_add(1),
            "steel": 1,
            "oil": 0
        }]);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &future_income,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidValue {
            field: "players.score.resourceIncomeHistory.tick",
        })
    ));

    let duplicate_income_tick = mutate_payload(&text, |value| {
        value["players"][0]["score"]["resourceIncomeHistory"] = serde_json::json!([
            { "tick": 0, "steel": 1, "oil": 0 },
            { "tick": 0, "steel": 0, "oil": 1 }
        ]);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &duplicate_income_tick,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::DuplicateId {
            field: "players.score.resourceIncomeHistory",
            id: 0
        })
    ));
}

#[test]
fn checkpoint_payload_rejects_inconsistent_metadata_rng_and_supply() {
    let game = Game::new_for_replay(&human_vs_ai_players(), 0x5150_2005);
    let text = checkpoint_payload_text_for(&game, "inconsistent metadata payload fixture");

    let mismatched_rng_seed = mutate_payload(&text, |value| {
        value["rng"]["seed"] = serde_json::json!(123);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &mismatched_rng_seed,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidValue { field: "rng.seed" })
    ));

    let inconsistent_command_log_metadata = mutate_payload(&text, |value| {
        value["commandLogMetadata"]["firstTick"] = serde_json::json!(0);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &inconsistent_command_log_metadata,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidValue {
            field: "commandLogMetadata.firstTick",
        })
    ));

    let inconsistent_supply = mutate_payload(&text, |value| {
        value["players"][0]["supplyUsed"] = serde_json::json!(999);
    });
    assert!(matches!(
        Game::restore_checkpoint_payload_text_for_test(
            &inconsistent_supply,
            game.state.map.clone(),
            game.map_metadata().clone(),
        ),
        Err(CheckpointPayloadError::InvalidValue {
            field: "players.supplyUsed",
        })
    ));
}

fn mutate_payload(text: &str, mutate: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut value: serde_json::Value = serde_json::from_str(text).expect("valid checkpoint JSON");
    mutate(&mut value);
    serde_json::to_string(&value).expect("mutated checkpoint JSON")
}

fn tank_armor_reaction_lock_mut(
    value: &mut serde_json::Value,
    tank: u32,
) -> &mut serde_json::Value {
    let tank = value["entities"]["entities"]
        .as_array_mut()
        .expect("entities array")
        .iter_mut()
        .find(|entity| entity["id"].as_u64() == Some(tank as u64))
        .expect("tank entity should exist");
    &mut tank["combat"]["tank_armor_reaction_lock"]
}

fn tick_until_panzerfaust_launch(game: &mut Game) {
    for _ in 0..30 {
        let events = game.tick();
        if player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustLaunch { .. }))
        {
            return;
        }
    }
    panic!("test setup should reach Panzerfaust launch before checkpointing");
}
