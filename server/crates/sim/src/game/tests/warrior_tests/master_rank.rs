use super::*;

#[test]
fn dao_master_advances_once_keeps_reload_and_persists_rank() {
    use crate::game::ability::AbilityKind;
    for (ability, hp) in [
        (AbilityKind::EvolveWind, 161),
        (AbilityKind::EvolveWar, 189),
        (AbilityKind::EvolveMagic, 161),
    ] {
        let mut game = empty_flat_game(&[cultivator_player()]);
        let id = game
            .state
            .entities
            .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
            .unwrap();
        let unit = game.state.entities.get_mut(id).unwrap();
        unit.insight = 60;
        assert!(unit.evolve_dao(ability));
        let speed = unit.movement_speed();
        let radius = unit.radius();
        unit.apply_damage(unit.hp - 1, None);
        unit.insight = 199;
        unit.set_attack_cd(30);
        unit.set_order(Order::HoldPosition);
        game.tick();
        assert!(!game.state.entities.get(id).unwrap().dao_master);
        let unit = game.state.entities.get_mut(id).unwrap();
        unit.insight = 200;
        game.tick();
        let unit = game.state.entities.get(id).unwrap();
        assert!(unit.dao_master);
        assert_eq!((unit.hp, unit.max_hp, unit.insight), (hp, hp, 0));
        assert_eq!(unit.attack_cd(), 28, "promotion preserves remaining reload");
        assert!(matches!(unit.order(), Order::HoldPosition));
        assert!((unit.movement_speed() - speed * 1.1).abs() < 0.00001);
        assert!((unit.radius() - radius * 1.2).abs() < 0.00001);
        let checkpoint = game.checkpoint_payload_text_for_test().unwrap();
        let restored = Game::restore_checkpoint_payload_text_for_test(
            &checkpoint,
            game.state.map.clone(),
            game.state.map_metadata.clone(),
        )
        .unwrap();
        let unit = restored.state.entities.get(id).unwrap();
        assert!(unit.dao_master);
        assert_eq!((unit.hp, unit.max_hp), (hp, hp));
        let view = game
            .snapshot_full_for(1)
            .entities
            .into_iter()
            .find(|e| e.id == id)
            .unwrap();
        assert_eq!(view.dao_master, Some(true));
        let unit = game.state.entities.get_mut(id).unwrap();
        unit.apply_damage(unit.hp - 1, None);
        unit.gain_insight(1000, EntityKind::Rifleman);
        unit.advance_dao_master();
        assert_eq!((unit.hp, unit.max_hp, unit.insight), (1, hp, 0));
    }
}

#[test]
fn dao_master_dead_unit_cannot_heal_and_invalid_rank_is_rejected() {
    use crate::game::ability::AbilityKind;
    let mut game = empty_flat_game(&[cultivator_player()]);
    let id = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.dao_master = true;
    assert!(
        game.checkpoint_payload_text_for_test().is_err(),
        "a Novice cannot be a Master"
    );
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.dao_master = false;
    unit.insight = 60;
    assert!(unit.evolve_dao(AbilityKind::EvolveWar));
    unit.insight = 200;
    unit.apply_damage(u32::MAX, None);
    unit.advance_dao_master();
    assert_eq!(unit.hp, 0);
    assert!(!unit.dao_master);
}

#[test]
fn dao_master_combat_threshold_and_damage_apply_to_all_three_paths() {
    use crate::game::ability::AbilityKind;
    for (ability, adept_damage, master_damage, cooldown) in [
        (AbilityKind::EvolveWind, 23, 28, 61),
        (AbilityKind::EvolveWar, 35, 42, 91),
        (AbilityKind::EvolveMagic, 23, 28, 91),
    ] {
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
            .spawn_unit(2, EntityKind::Warrior, 425.0, 400.0)
            .unwrap();
        let victim = game.state.entities.get_mut(target).unwrap();
        victim.set_attack_cd(1000);
        let unit = game.state.entities.get_mut(id).unwrap();
        unit.insight = 60;
        unit.evolve_dao(ability);
        unit.insight = 199;
        unit.apply_damage(unit.hp - 1, None);
        unit.set_order(Order::attack(target));
        for _ in 0..30 {
            game.tick();
            if game.state.entities.get(id).unwrap().dao_master {
                break;
            }
        }
        let unit = game.state.entities.get(id).unwrap();
        assert!(unit.dao_master, "combat insight advances in the same tick");
        assert_eq!(unit.hp, unit.max_hp);
        assert!(matches!(unit.order(), Order::Attack(_)));
        assert_eq!(
            game.state.entities.get(target).unwrap().hp,
            115 - adept_damage
        );
        game.state.entities.get_mut(id).unwrap().set_attack_cd(0);
        game.tick();
        assert_eq!(
            game.state.entities.get(target).unwrap().hp,
            115 - adept_damage - master_damage
        );
        assert_eq!(game.state.entities.get(id).unwrap().insight, 0);
        assert_eq!(game.state.entities.get(id).unwrap().attack_cd(), cooldown);
        let enemy = game
            .snapshot_for(2)
            .entities
            .into_iter()
            .find(|e| e.id == id)
            .unwrap();
        assert_eq!(enemy.dao_master, Some(true));
        assert_eq!(enemy.insight, None);
    }
}

#[test]
fn disciple_weighted_insight_carries_exact_fractions_through_targets_and_checkpoints() {
    let mut game = empty_flat_game(&[cultivator_player()]);
    let id = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MagicAdept, 400.0, 400.0)
        .unwrap();
    game.tick();
    let unit = game.state.entities.get_mut(id).unwrap();
    // Adept value 475, Scout Car value 550: 19 HP earns exactly 22.
    for _ in 0..18 {
        unit.gain_insight(1, EntityKind::ScoutCar);
    }
    assert_eq!(unit.insight, 20);
    let checkpoint = game.checkpoint_payload_text_for_test().unwrap();
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &checkpoint,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    let unit = restored.state.entities.get_mut(id).unwrap();
    unit.gain_insight(1, EntityKind::ScoutCar);
    assert_eq!((unit.insight, unit.insight_fraction), (22, 0));
    // Unit hits retain halves at the floor; buildings still retain quarters.
    for target in [
        EntityKind::Rifleman,
        EntityKind::ResourceDepot,
        EntityKind::Golem,
        EntityKind::Rifleman,
        EntityKind::ResourceDepot,
    ] {
        unit.gain_insight(1, target);
    }
    assert_eq!((unit.insight, unit.insight_fraction), (24, 0));
    unit.gain_insight(1, EntityKind::Tank);
    assert_eq!(unit.insight, 26, "expensive targets cap at double insight");
    unit.gain_insight(u32::MAX, EntityKind::Tank);
    assert_eq!((unit.insight, unit.insight_fraction), (200, 0));
    unit.advance_dao_master();
    unit.gain_insight(1, EntityKind::Rifleman);
    assert_eq!((unit.insight, unit.insight_fraction), (0, 0));
}

#[test]
fn disciple_insight_fraction_checkpoint_validation_and_evolution_reset() {
    let mut game = empty_flat_game(&[cultivator_player()]);
    let id = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    game.tick();
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.gain_insight(1, EntityKind::Rifleman);
    assert!(unit.insight_fraction > 0);
    unit.insight_fraction = 800; // Novice denominator is 4 * (100 steel * 2).
    assert!(game.checkpoint_payload_text_for_test().is_err());
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.insight_fraction = 1;
    unit.insight = 60;
    assert!(
        game.checkpoint_payload_text_for_test().is_err(),
        "capped units cannot retain fractions"
    );
    let unit = game.state.entities.get_mut(id).unwrap();
    assert!(unit.evolve_dao(crate::game::ability::AbilityKind::EvolveMagic));
    assert_eq!((unit.insight, unit.insight_fraction), (0, 0));
    game.tick();
    assert!(game.checkpoint_payload_text_for_test().is_ok());
}
