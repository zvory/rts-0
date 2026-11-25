use super::*;
use crate::game::ability::AbilityKind;
use crate::game::upgrade::UpgradeKind;

#[test]
fn novice_random_tao_waits_for_full_insight_and_preserves_explicit_choices() {
    let mut game = empty_flat_game(&[cultivator_player()]);
    let id = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.insight = config::DISCIPLE_INSIGHT_REQUIRED - 1;
    let draws = game.state.rng.draws_consumed();
    unit.advance_dao(&mut game.state.rng);
    assert_eq!(unit.chosen_dao, None);
    assert_eq!(unit.kind, EntityKind::Warrior);
    assert_eq!(game.state.rng.draws_consumed(), draws);

    unit.insight += 1;
    unit.choose_dao(AbilityKind::EvolveMagic);
    unit.advance_dao(&mut game.state.rng);
    assert_eq!(unit.dao(), Some(UpgradeKind::DaoOfMagic));
    assert_eq!(game.state.rng.draws_consumed(), draws);
    unit.advance_dao(&mut game.state.rng);
    assert_eq!(game.state.rng.draws_consumed(), draws);

    let dead = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 500.0, 400.0)
        .unwrap();
    let unit = game.state.entities.get_mut(dead).unwrap();
    unit.insight = config::DISCIPLE_INSIGHT_REQUIRED;
    unit.apply_damage(u32::MAX, None);
    unit.advance_dao(&mut game.state.rng);
    assert_eq!(unit.hp, 0);
    assert_eq!(unit.dao(), None);
    assert_eq!(game.state.rng.draws_consumed(), draws);
}

#[test]
fn novice_random_tao_evolves_all_paths_and_repeats_after_checkpoint() {
    let mut game = empty_flat_game(&[cultivator_player()]);
    let mut ids = Vec::new();
    for i in 0..48 {
        let id = game
            .state
            .entities
            .spawn_unit(
                1,
                EntityKind::Warrior,
                200.0 + (i % 8) as f32 * 100.0,
                200.0 + (i / 8) as f32 * 100.0,
            )
            .unwrap();
        let unit = game.state.entities.get_mut(id).unwrap();
        unit.insight = config::DISCIPLE_INSIGHT_REQUIRED - 1;
        unit.apply_damage(10, None);
        unit.set_order(Order::HoldPosition);
        ids.push(id);
    }
    // Rebuild supply and other tick-owned state before checkpointing the fixture.
    game.tick();
    for &id in &ids {
        game.state.entities.get_mut(id).unwrap().insight = config::DISCIPLE_INSIGHT_REQUIRED;
    }
    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    game.tick();
    restored.tick();
    assert_eq!(
        game.replay_end_state_hash().unwrap(),
        restored.replay_end_state_hash().unwrap()
    );
    let mut paths = std::collections::HashSet::new();
    for id in ids {
        let unit = game.state.entities.get(id).unwrap();
        paths.insert(
            unit.dao()
                .expect("full insight automatically chooses a Tao"),
        );
        assert_eq!(unit.insight, 0);
        assert_eq!(unit.hp, unit.max_hp);
        assert_eq!(unit.chosen_dao, None);
        assert!(matches!(unit.order(), Order::HoldPosition));
    }
    assert_eq!(paths.len(), 3, "the seeded sample reaches every Tao");
}

#[test]
fn novice_random_tao_advances_on_combat_insight_without_interrupting_attack() {
    let mut game = empty_flat_game(&[
        cultivator_player(),
        PlayerInit {
            id: 2,
            team_id: 2,
            ..cultivator_player()
        },
    ]);
    let id = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Warrior, 455.0, 400.0)
        .unwrap();
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.insight = config::DISCIPLE_INSIGHT_REQUIRED - 1;
    unit.set_order(Order::attack(target));
    for _ in 0..60 {
        game.tick();
        if game.state.entities.get(id).unwrap().dao().is_some() {
            break;
        }
    }
    let unit = game.state.entities.get(id).unwrap();
    assert!(unit.dao().is_some());
    assert_eq!(unit.insight, 0);
    assert!(matches!(unit.order(), Order::Attack(_)));
    let victim = game.state.entities.get(target).unwrap();
    assert!(victim.hp < victim.max_hp);
}
