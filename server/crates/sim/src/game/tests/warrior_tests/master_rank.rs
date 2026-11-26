use super::*;

#[test]
fn dao_master_advances_once_keeps_reload_and_persists_rank() {
    use crate::game::ability::AbilityKind;
    for (ability, hp) in [
        (AbilityKind::EvolveWind, 210),
        (AbilityKind::EvolveWar, 246),
        (AbilityKind::EvolveMagic, 210),
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
fn dao_master_combat_threshold_and_damage_apply_to_combat_paths() {
    use crate::game::ability::AbilityKind;
    for (ability, adept_damage, master_damage, cooldown) in [
        (AbilityKind::EvolveWind, 23, 28, 61),
        (AbilityKind::EvolveWar, 35, 84, 91),
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
            .spawn_unit(2, EntityKind::WindAdept, 455.0, 400.0)
            .unwrap();
        let victim = game.state.entities.get_mut(target).unwrap();
        let target_hp = victim.hp;
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
            target_hp - adept_damage
        );
        game.state.entities.get_mut(id).unwrap().set_attack_cd(0);
        game.tick();
        assert_eq!(
            game.state.entities.get(target).unwrap().hp,
            target_hp - adept_damage - master_damage
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
        assert_eq!(enemy.insight, Some(0));
    }
}

#[test]
fn disciple_flat_unit_insight_carries_building_fractions_through_checkpoints() {
    for kind in [
        EntityKind::Warrior,
        EntityKind::WindAdept,
        EntityKind::WarAdept,
        EntityKind::MagicAdept,
    ] {
        let mut game = empty_flat_game(&[cultivator_player()]);
        let id = game
            .state
            .entities
            .spawn_unit(1, kind, 400.0, 400.0)
            .unwrap();
        game.tick();
        let unit = game.state.entities.get_mut(id).unwrap();
        for target in [
            EntityKind::Rifleman,
            EntityKind::ScoutCar,
            EntityKind::Golem,
            EntityKind::Tank,
        ] {
            unit.gain_insight(3, target);
        }
        let expected = if kind == EntityKind::WarAdept { 15 } else { 12 };
        assert_eq!(
            (unit.insight, unit.insight_fraction),
            (expected, 0),
            "{kind:?}"
        );
        unit.gain_insight(3, EntityKind::ResourceDepot);
        assert_eq!((unit.insight, unit.insight_fraction), (expected, 3));
        let checkpoint = game.checkpoint_payload_text_for_test().unwrap();
        let mut restored = Game::restore_checkpoint_payload_text_for_test(
            &checkpoint,
            game.state.map.clone(),
            game.state.map_metadata.clone(),
        )
        .unwrap();
        let unit = restored.state.entities.get_mut(id).unwrap();
        unit.gain_insight(1, EntityKind::Rifleman);
        assert_eq!((unit.insight, unit.insight_fraction), (expected + 1, 3));
        unit.gain_insight(1, EntityKind::ResourceDepot);
        assert_eq!((unit.insight, unit.insight_fraction), (expected + 2, 0));
        unit.gain_insight(u32::MAX, EntityKind::Tank);
        assert_eq!(
            (unit.insight, unit.insight_fraction),
            (unit.insight_cap(), 0)
        );
        if kind != EntityKind::Warrior {
            unit.advance_dao_master();
            unit.gain_insight(1, EntityKind::Rifleman);
            assert_eq!((unit.insight, unit.insight_fraction), (0, 0));
        }
    }
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
    unit.gain_insight(1, EntityKind::ResourceDepot);
    assert!(unit.insight_fraction > 0);
    unit.insight_fraction = 4; // Building fractions use a fixed denominator of four.
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

#[test]
fn wind_insight_extractors_grant_full_actual_damage_other_buildings_quarter() {
    for kind in [
        EntityKind::WindAdept,
        EntityKind::WarAdept,
        EntityKind::MagicAdept,
        EntityKind::Warrior,
    ] {
        let mut game = empty_flat_game(&[cultivator_player()]);
        let id = game
            .state
            .entities
            .spawn_unit(1, kind, 400.0, 400.0)
            .unwrap();
        for target in [
            EntityKind::PumpJack,
            EntityKind::SteelMine,
            EntityKind::ResourceDepot,
        ] {
            let unit = game.state.entities.get_mut(id).unwrap();
            unit.insight = 0;
            unit.insight_fraction = 0;
            unit.gain_insight(12, target);
            let expected = if kind == EntityKind::WindAdept && target.is_resource_extractor() {
                12
            } else {
                3
            };
            assert_eq!(unit.insight, expected, "{kind:?} against {target:?}");
            if kind == EntityKind::WindAdept && target.is_resource_extractor() {
                // Full-rate extractor hits retain no fractional remainder.
                unit.gain_insight(8, target);
                assert_eq!((unit.insight, unit.insight_fraction), (20, 0));
            }
        }
    }
}

#[test]
fn novice_evolution_clears_the_larger_body_from_adjacent_buildings() {
    let mut game = empty_flat_game(&[cultivator_player()]);
    game.state
        .entities
        .spawn_building(1, EntityKind::ProvenGrounds, 624.0, 624.0, true)
        .unwrap();
    let id = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 692.0, 624.0)
        .unwrap();
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.insight = config::DISCIPLE_INSIGHT_REQUIRED;
    unit.set_order(Order::HoldPosition);
    game.enqueue(
        1,
        Command::UseAbility {
            target_id: None,
            ability: crate::game::ability::AbilityKind::EvolveMagic,
            units: vec![id],
            x: None,
            y: None,
            queued: false,
        },
    );
    game.tick();
    let unit = game.state.entities.get(id).unwrap();
    assert_eq!(unit.kind, EntityKind::MagicAdept);
    assert!(
        unit.pos_x() >= 698.0,
        "the 26px Adept must clear the building's right edge at x=672"
    );
    assert!(game.checkpoint_payload_text_for_test().is_ok());
}

#[test]
fn novice_evolution_waits_for_room_without_healing_or_losing_insight() {
    let mut game = empty_flat_game(&[cultivator_player(), human_vs_ai_players()[1].clone()]);
    let blockers = [
        (430.0, 400.0),
        (400.0, 370.0),
        (370.0, 400.0),
        (400.0, 430.0),
    ]
    .map(|(x, y)| {
        game.state
            .entities
            .spawn_unit(2, EntityKind::Worker, x, y)
            .unwrap()
    });
    let id = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.insight = config::DISCIPLE_INSIGHT_REQUIRED;
    unit.choose_dao(crate::game::ability::AbilityKind::EvolveMagic);
    unit.apply_damage(1, None);
    game.tick();
    let unit = game.state.entities.get(id).unwrap();
    assert_eq!(
        (unit.kind, unit.hp, unit.insight),
        (EntityKind::Warrior, 57, 60)
    );
    assert!(unit.chosen_dao.is_some());
    assert!(game.checkpoint_payload_text_for_test().is_ok());
    game.state.entities.remove(blockers[0]);
    game.state.entities.remove(blockers[1]);
    game.tick();
    let unit = game.state.entities.get(id).unwrap();
    assert_eq!(
        (unit.kind, unit.hp, unit.insight),
        (EntityKind::MagicAdept, 150, 0)
    );
    assert!(unit.chosen_dao.is_none());
}
