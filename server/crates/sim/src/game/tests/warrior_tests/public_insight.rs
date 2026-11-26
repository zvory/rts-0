use super::*;

#[test]
fn disciple_insight_is_public_but_chosen_dao_stays_team_private() {
    use crate::game::ability::AbilityKind;
    use crate::game::upgrade::UpgradeKind;
    let enemy = PlayerInit {
        id: 2,
        team_id: 2,
        ..cultivator_player()
    };
    let ally = PlayerInit {
        id: 3,
        ..cultivator_player()
    };
    let mut game = empty_flat_game(&[cultivator_player(), enemy, ally]);
    let unit = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    game.state
        .entities
        .spawn_unit(2, EntityKind::Worker, 450.0, 400.0)
        .unwrap();
    game.state
        .entities
        .spawn_unit(3, EntityKind::Worker, 480.0, 400.0)
        .unwrap();
    game.tick();
    game.state.entities.get_mut(unit).unwrap().insight = 60;
    game.state.entities.get_mut(unit).unwrap().chosen_dao = Some(UpgradeKind::DaoOfMagic);
    game.state
        .fog
        .recompute(&[1, 2, 3], &game.state.entities, &game.state.map);
    for viewer in [1, 2, 3] {
        let snapshot = game.snapshot_for(viewer);
        let view = snapshot
            .entities
            .iter()
            .find(|e| e.id == unit)
            .unwrap_or_else(|| {
                panic!(
                    "missing disciple for viewer {viewer}; players {:?}",
                    game.state
                        .players
                        .iter()
                        .map(|p| (p.id, p.team_id))
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(view.insight, Some(60));
        assert_eq!(view.chosen_dao.as_deref(), (viewer != 2).then_some("magic"));
        assert_eq!(view.dao_ready, Some(true));
    }
    game.state.players[0].upgrades.insert(UpgradeKind::DaoWind);
    game.enqueue(
        1,
        Command::UseAbility {
            target_id: None,
            ability: AbilityKind::EvolveWind,
            units: vec![unit],
            x: None,
            y: None,
            queued: false,
        },
    );
    game.tick();
    let enemy_view = game
        .snapshot_for(2)
        .entities
        .into_iter()
        .find(|e| e.id == unit)
        .unwrap();
    assert_eq!(enemy_view.insight, Some(0));
    assert_eq!(enemy_view.dao_ready, Some(false));
    assert!(enemy_view
        .dao_advance_ticks
        .is_some_and(|n| n > 0 && n <= 15));
    for _ in 0..15 {
        game.tick();
    }
    assert_eq!(game.state.entities.get(unit).unwrap().dao_advance_ticks, 0);

    // Partial Adept XP is public too, while fog still hides the whole unit.
    game.state.entities.get_mut(unit).unwrap().insight = 37;
    assert_eq!(
        game.snapshot_for(2)
            .entities
            .iter()
            .find(|e| e.id == unit)
            .unwrap()
            .insight,
        Some(37)
    );
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .set_position(1000.0, 1000.0);
    game.state
        .fog
        .recompute(&[1, 2, 3], &game.state.entities, &game.state.map);
    assert!(!game.snapshot_for(2).entities.iter().any(|e| e.id == unit));
}

#[test]
fn passive_insight_ticks_in_game_and_triggers_automatic_evolution() {
    let mut game = empty_flat_game(&[cultivator_player()]);
    let novice = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    let serf = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, 600.0, 400.0)
        .unwrap();
    let unit = game.state.entities.get_mut(novice).unwrap();
    unit.insight = 59;
    unit.chosen_dao = Some(crate::game::upgrade::UpgradeKind::DaoWind);
    for _ in 1..config::DISCIPLE_PASSIVE_INSIGHT_TICKS {
        game.tick();
    }
    assert_eq!(
        game.state.entities.get(novice).unwrap().kind,
        EntityKind::Warrior
    );
    // A later spawn must start its own cadence, including across a real game checkpoint.
    let late = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 800.0, 400.0)
        .unwrap();
    // Refresh derived state and supply before exporting the newly spawned fixture.
    game.tick();
    let evolved = game.state.entities.get(novice).unwrap();
    assert_eq!(evolved.kind, EntityKind::WindAdept);
    assert_eq!(evolved.insight, 0);
    assert_eq!(game.state.entities.get(serf).unwrap().insight, 0);
    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    for _ in 1..config::DISCIPLE_PASSIVE_INSIGHT_TICKS {
        game.tick();
        restored.tick();
    }
    for continued in [&game, &restored] {
        assert_eq!(continued.state.entities.get(novice).unwrap().insight, 0);
        assert_eq!(continued.state.entities.get(late).unwrap().insight, 1);
    }
    game.tick();
    restored.tick();
    for continued in [&game, &restored] {
        assert_eq!(continued.state.entities.get(novice).unwrap().insight, 1);
        assert_eq!(continued.state.entities.get(late).unwrap().insight, 1);
    }
}
