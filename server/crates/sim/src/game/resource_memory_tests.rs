use super::*;

fn remaining(snapshot: &Snapshot, id: u32) -> u32 {
    snapshot
        .resource_deltas
        .iter()
        .find(|row| row.id == id)
        .expect("known patch")
        .remaining
}

#[test]
fn resource_knowledge_survives_hidden_depletion_checkpoint_and_observer_switches() {
    for kind in [EntityKind::Steel, EntityKind::Oil] {
        let mut game = empty_flat_game_with_players(&three_players());
        let near = game.state.map.tile_center(22, 20);
        let far = game.state.map.tile_center(70, 70);
        let node = game
            .state
            .entities
            .spawn_node(kind, near.0, near.1)
            .unwrap();
        let initial = game.state.entities.get(node).unwrap().remaining().unwrap();
        let scout1 = game
            .state
            .entities
            .spawn_unit(1, EntityKind::Rifleman, near.0, near.1)
            .unwrap();
        let scout2 = game
            .state
            .entities
            .spawn_unit(2, EntityKind::Rifleman, far.0, far.1)
            .unwrap();
        advance_to_next_fog_refresh(&mut game);
        assert_eq!(remaining(&game.snapshot_for(1), node), initial);
        let before = game.clone_for_replay_keyframe();
        game.state
            .entities
            .get_mut(scout1)
            .unwrap()
            .set_position(far.0, far.1);
        game.state
            .entities
            .get_mut(scout2)
            .unwrap()
            .set_position(near.0, near.1);
        advance_to_next_fog_refresh(&mut game);
        game.state
            .entities
            .get_mut(node)
            .unwrap()
            .harvest_resources(initial);
        advance_to_next_fog_refresh(&mut game);
        assert!(game.state.entities.get(node).is_none());
        assert_eq!(
            remaining(&game.snapshot_for(1), node),
            initial,
            "hidden depletion stays unknown"
        );
        assert_eq!(remaining(&game.snapshot_for(2), node), 0);
        for view in [
            ObserverView::Omniscient,
            ObserverView::Players(vec![1, 2]),
            ObserverView::Players(vec![2, 1]),
        ] {
            assert_eq!(remaining(&game.snapshot_for_observer(&view), node), 0);
        }
        assert_eq!(
            remaining(
                &game.snapshot_for_observer(&ObserverView::Players(vec![1])),
                node
            ),
            initial,
            "viewing other knowledge cannot mutate player one's memory"
        );
        let restored = Game::restore_checkpoint_payload_text_for_test(
            &game.checkpoint_payload_text_for_test().unwrap(),
            game.state.map.clone(),
            game.map_metadata().clone(),
        )
        .unwrap();
        assert_eq!(
            game.snapshot_for(1).resource_deltas,
            restored.snapshot_for(1).resource_deltas
        );
        assert_eq!(
            game.snapshot_for(2).resource_deltas,
            restored.snapshot_for(2).resource_deltas
        );
        game.state
            .entities
            .get_mut(scout1)
            .unwrap()
            .set_position(near.0, near.1);
        advance_to_next_fog_refresh(&mut game);
        game.state
            .entities
            .get_mut(scout1)
            .unwrap()
            .set_position(far.0, far.1);
        advance_to_next_fog_refresh(&mut game);
        assert_eq!(
            remaining(&game.snapshot_for(1), node),
            0,
            "known depletion persists outside sight"
        );
        assert_eq!(
            remaining(&before.snapshot_for(1), node),
            initial,
            "backward keyframe restores earlier knowledge"
        );
    }
}

#[test]
fn resource_knowledge_legacy_launch_seed_and_nonzero_missing_memory_validation() {
    let game = Game::new_for_replay(&players(), 42);
    let mut payload: serde_json::Value =
        serde_json::from_str(&game.checkpoint_payload_text_for_test().unwrap()).unwrap();
    payload.as_object_mut().unwrap().remove("resourceMemory");
    let restored = Game::restore_checkpoint_payload_text_for_test(
        &payload.to_string(),
        game.state.map.clone(),
        game.map_metadata().clone(),
    )
    .unwrap();
    assert_eq!(
        game.snapshot_for(1).resource_deltas,
        restored.snapshot_for(1).resource_deltas
    );
    payload["tick"] = serde_json::json!(1);
    assert!(Game::restore_checkpoint_payload_text_for_test(
        &payload.to_string(),
        game.state.map.clone(),
        game.map_metadata().clone()
    )
    .is_err());
}

#[test]
fn resource_knowledge_rejects_future_or_foreign_observations() {
    let game = Game::new_for_replay(&players(), 42);
    let original: serde_json::Value =
        serde_json::from_str(&game.checkpoint_payload_text_for_test().unwrap()).unwrap();
    for observation in [
        serde_json::json!({"999":{"remaining":0,"tick":0}}),
        serde_json::json!({"1":{"remaining":0,"tick":1}}),
    ] {
        let mut payload = original.clone();
        let node = payload["resourceMemory"]["nodes"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        node["observations"] = observation;
        assert!(Game::restore_checkpoint_payload_text_for_test(
            &payload.to_string(),
            game.state.map.clone(),
            game.map_metadata().clone()
        )
        .is_err());
    }
}

#[test]
fn resource_knowledge_storage_is_bounded_by_patches_and_players() {
    let mut game = Game::new_for_replay(&three_players(), 42);
    let patches = game.state.resource_memory.nodes.len();
    let ids = vec![1, 2, 3];
    let started = std::time::Instant::now();
    for _ in 0..1_000 {
        game.state.tick += 1;
        game.refresh_fog_memories(&ids);
    }
    eprintln!(
        "Resource/fog memory refresh: {} patches, {} players, {:.1} us/refresh (1000 samples)",
        patches,
        ids.len(),
        started.elapsed().as_micros() as f64 / 1_000.0
    );
    assert_eq!(game.state.resource_memory.nodes.len(), patches);
    assert!(game
        .state
        .resource_memory
        .nodes
        .values()
        .all(|node| node.observations.len() <= ids.len()));
}
