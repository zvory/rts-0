use super::fixtures::*;
use super::*;

fn cultivator_player() -> PlayerInit {
    PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::CULTIVATORS_FACTION_ID.to_string(),
        name: "Cultivator".into(),
        color: "#fff".into(),
        is_ai: false,
    }
}

#[test]
fn cultivator_portal_trains_warrior_and_reserves_two_supply() {
    let mut game = empty_flat_game(&[cultivator_player()]);
    game.state.players[0].set_resources(100, 0);
    let position = game.state.map.tile_center(12, 12);
    let portal = game
        .state
        .entities
        .spawn_building(1, EntityKind::Portal, position.0, position.1, true)
        .expect("Portal should spawn");

    game.enqueue(
        1,
        Command::Train {
            building: portal,
            unit: EntityKind::Warrior,
        },
    );
    game.tick();

    assert_eq!(game.state.players[0].steel, 0);
    assert_eq!(game.state.players[0].supply_used, 2);
    assert_eq!(
        game.state
            .entities
            .get(portal)
            .map(|producer| producer.prod_queue().len()),
        Some(1)
    );

    for _ in 0..config::unit_stats(EntityKind::Warrior)
        .expect("Warrior stats should exist")
        .build_ticks
    {
        game.tick();
    }

    let warrior = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Warrior)
        .expect("Portal should produce a Warrior");
    assert_eq!((warrior.hp, warrior.max_hp), (135, 135));
    assert_eq!(warrior.radius(), 13.5);
}

#[test]
fn cultivator_pavilion_research_spends_resources_completes_and_leaves_disciple_unchanged() {
    use crate::game::upgrade::{self, UpgradeKind};

    let mut game = empty_flat_game(&[cultivator_player()]);
    game.state.players[0].set_resources(600, 300);
    let pavilion_pos = game.state.map.tile_center(12, 12);
    let pavilion = game
        .state
        .entities
        .spawn_building(
            1,
            EntityKind::ThreeDaoPavilion,
            pavilion_pos.0,
            pavilion_pos.1,
            true,
        )
        .expect("Pavilion should spawn");
    let disciple_pos = game.state.map.tile_center(20, 20);
    let disciple = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, disciple_pos.0, disciple_pos.1)
        .expect("Disciple should spawn");
    let before = game.state.entities.get(disciple).unwrap().clone();
    let daos = [
        UpgradeKind::DaoOfLightness,
        UpgradeKind::DaoOfBarriers,
        UpgradeKind::DaoOfHeaviness,
    ];
    for dao in daos {
        game.enqueue(
            1,
            Command::Research {
                building: pavilion,
                upgrade: dao,
            },
        );
    }
    // Repeated requests must not create a second research or spend resources twice.
    game.enqueue(
        1,
        Command::Research {
            building: pavilion,
            upgrade: daos[0],
        },
    );
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(pavilion)
            .unwrap()
            .research_queue()
            .len(),
        3
    );
    for dao in daos {
        assert_eq!(upgrade::definition(dao).research_ticks, 600);
    }
    // The shared queue pays each item when it becomes active.
    for _ in 0..1800 {
        game.tick();
    }
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (0, 0)
    );
    for dao in daos {
        assert!(game.state.players[0].upgrades.contains(&dao));
    }
    assert!(game
        .state
        .entities
        .get(pavilion)
        .unwrap()
        .research_queue()
        .is_empty());
    let after = game.state.entities.get(disciple).unwrap();
    assert_eq!(
        (after.hp, after.max_hp, after.pos_x(), after.pos_y()),
        (before.hp, before.max_hp, before.pos_x(), before.pos_y())
    );
    game.state.entities.remove(pavilion);
    game.tick();
    for dao in daos {
        assert!(game.state.players[0].upgrades.contains(&dao));
    }
}

#[test]
fn pavilion_prerequisites_and_research_are_faction_gated() {
    use crate::rules::{
        economy,
        faction::{self, UpgradeKind},
    };
    let pavilion = EntityKind::ThreeDaoPavilion;
    let cultivators = faction::CULTIVATORS_FACTION_ID;
    assert!(economy::can_build_for_faction(
        cultivators,
        EntityKind::Worker,
        pavilion
    ));
    assert!(!economy::can_build_for_faction(
        faction::DEFAULT_FACTION_ID,
        EntityKind::Worker,
        pavilion
    ));
    assert!(!economy::build_requirement_met(
        pavilion,
        &[EntityKind::ResourceDepot]
    ));
    assert!(!economy::build_requirement_met(
        pavilion,
        &[EntityKind::Portal]
    ));
    assert!(economy::build_requirement_met(
        pavilion,
        &[EntityKind::ResourceDepot, EntityKind::Portal]
    ));
    for dao in [
        UpgradeKind::DaoOfLightness,
        UpgradeKind::DaoOfBarriers,
        UpgradeKind::DaoOfHeaviness,
    ] {
        assert!(economy::can_research_for_faction(
            cultivators,
            dao,
            pavilion
        ));
        assert!(!economy::can_research_for_faction(
            cultivators,
            dao,
            EntityKind::Portal
        ));
        assert!(!economy::can_research_for_faction(
            faction::DEFAULT_FACTION_ID,
            dao,
            pavilion
        ));
    }
}

#[test]
fn disciple_evolution_requires_insight_research_ownership_and_is_permanent() {
    use crate::game::ability::AbilityKind;
    use crate::game::upgrade::UpgradeKind;
    for (ability, upgrade, hp) in [
        (
            AbilityKind::EvolveLightness,
            UpgradeKind::DaoOfLightness,
            135,
        ),
        (AbilityKind::EvolveWar, UpgradeKind::DaoOfHeaviness, 155),
        (AbilityKind::EvolveReality, UpgradeKind::DaoOfBarriers, 135),
    ] {
        let mut game = empty_flat_game(&[
            cultivator_player(),
            PlayerInit {
                id: 2,
                team_id: 2,
                ..cultivator_player()
            },
        ]);
        let unit = game
            .state
            .entities
            .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
            .unwrap();
        let command = Command::UseAbility {
            ability,
            units: vec![unit, unit],
            x: None,
            y: None,
            queued: false,
        };
        game.enqueue(1, command.clone());
        game.tick();
        assert_eq!(game.state.entities.get(unit).unwrap().dao, None);
        game.state.entities.get_mut(unit).unwrap().insight = 60;
        game.enqueue(1, command.clone());
        game.tick();
        assert_eq!(
            game.state.entities.get(unit).unwrap().dao,
            None,
            "research is required"
        );
        game.state.players[0].upgrades.insert(upgrade);
        game.state.entities.get_mut(unit).unwrap().insight = 59;
        game.enqueue(1, command.clone());
        game.tick();
        assert_eq!(
            game.state.entities.get(unit).unwrap().dao,
            None,
            "60 insight is required"
        );
        game.state.entities.get_mut(unit).unwrap().insight = 60;
        game.enqueue(2, command.clone());
        game.tick();
        assert_eq!(
            game.state.entities.get(unit).unwrap().dao,
            None,
            "ownership is required"
        );
        game.enqueue(1, command.clone());
        game.tick();
        let evolved = game.state.entities.get(unit).unwrap();
        assert_eq!(evolved.dao, Some(upgrade));
        assert_eq!((evolved.hp, evolved.max_hp), (hp, hp));
        game.enqueue(1, command);
        game.tick();
        assert_eq!(
            game.state.entities.get(unit).unwrap().max_hp,
            hp,
            "no repeated HP grants"
        );
        let payload = game.checkpoint_payload_text_for_test().unwrap();
        assert!(payload.contains("\"insight\":60"));
        let restored = Game::restore_checkpoint_payload_text_for_test(
            &payload,
            game.state.map.clone(),
            game.state.map_metadata.clone(),
        )
        .unwrap();
        assert_eq!(
            restored.state.entities.get(unit).unwrap().dao,
            Some(upgrade)
        );
    }
}

#[test]
fn disciple_dao_gates_blink_return_and_magic_anchor() {
    use crate::game::ability::AbilityKind;
    use crate::game::upgrade::UpgradeKind;
    let mut game = empty_flat_game(&[cultivator_player()]);
    let unit = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    let cast = |ability| Command::UseAbility {
        ability,
        units: vec![unit],
        x: Some(480.0),
        y: Some(400.0),
        queued: false,
    };
    game.enqueue(1, cast(AbilityKind::EkatTeleport));
    game.tick();
    game.enqueue(1, cast(AbilityKind::EkatMagicAnchor));
    game.tick();
    assert_eq!(game.state.entities.get(unit).unwrap().pos_x(), 400.0);
    assert_eq!(game.state.ability_runtime.world_objects().count(), 0);
    game.state.entities.get_mut(unit).unwrap().insight = 60;
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfLightness);
    game.enqueue(1, cast(AbilityKind::EvolveLightness));
    game.tick();
    game.enqueue(1, cast(AbilityKind::EkatTeleport));
    game.tick();
    assert_eq!(game.state.entities.get(unit).unwrap().pos_x(), 480.0);
    game.enqueue(
        1,
        Command::RecastAbility {
            ability: AbilityKind::EkatTeleport,
            units: vec![unit],
            target_object_id: None,
            queued: false,
        },
    );
    game.tick();
    assert_eq!(game.state.entities.get(unit).unwrap().pos_x(), 400.0);
    assert_eq!(game.state.ability_runtime.world_objects().count(), 0);
    game.enqueue(1, cast(AbilityKind::EkatMagicAnchor));
    game.tick();
    assert_eq!(game.state.ability_runtime.world_objects().count(), 0);

    let support = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 600.0, 400.0)
        .unwrap();
    game.state.entities.get_mut(support).unwrap().insight = 60;
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfBarriers);
    game.enqueue(
        1,
        Command::UseAbility {
            ability: AbilityKind::EvolveReality,
            units: vec![support],
            x: None,
            y: None,
            queued: false,
        },
    );
    game.tick();
    game.enqueue(
        1,
        Command::UseAbility {
            ability: AbilityKind::EkatMagicAnchor,
            units: vec![support],
            x: Some(640.0),
            y: Some(400.0),
            queued: false,
        },
    );
    game.tick();
    assert_eq!(game.state.ability_runtime.world_objects().count(), 1);
    game.enqueue(
        1,
        Command::UseAbility {
            ability: AbilityKind::EkatTeleport,
            units: vec![support],
            x: Some(640.0),
            y: Some(400.0),
            queued: false,
        },
    );
    game.tick();
    // The Anchor can pull its own caster slightly; a forbidden blink must not cross 40px.
    assert!((game.state.entities.get(support).unwrap().pos_x() - 600.0).abs() < 2.0);
}

#[test]
fn disciple_shield_shared_pool_overflow_teams_and_checkpoint() {
    use crate::game::ability::AbilityKind;
    use crate::game::teams::TeamRelations;
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
    let caster = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    let friend = game
        .state
        .entities
        .spawn_unit(3, EntityKind::Rifleman, 480.0, 400.0)
        .unwrap();
    let outside = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, 700.0, 400.0)
        .unwrap();
    game.state.entities.get_mut(caster).unwrap().insight = 60;
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfBarriers);
    let cast = |ability| Command::UseAbility {
        ability,
        units: vec![caster],
        x: Some(480.0),
        y: Some(400.0),
        queued: false,
    };
    game.enqueue(1, cast(AbilityKind::DiscipleShield));
    game.tick();
    assert!(game.state.entities.get(caster).unwrap().shield.is_none());
    game.enqueue(1, cast(AbilityKind::EvolveReality));
    game.tick();
    game.enqueue(1, cast(AbilityKind::DiscipleShield));
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(caster)
            .unwrap()
            .shield
            .as_ref()
            .unwrap()
            .remaining,
        300
    );
    let foe = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, 480.0, 400.0)
        .unwrap();
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2), (3, 1)]);
    assert_eq!(
        game.state.entities.shielded_damage(outside, 2, 50, &teams),
        50
    );
    assert_eq!(game.state.entities.shielded_damage(foe, 1, 50, &teams), 50);
    assert_eq!(
        game.state.entities.shielded_damage(friend, 1, 50, &teams),
        50
    );
    assert_eq!(
        game.state.entities.shielded_damage(friend, 2, 120, &teams),
        0
    );
    assert_eq!(
        game.state.entities.shielded_damage(caster, 2, 80, &teams),
        0
    );
    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    assert_eq!(
        restored
            .state
            .entities
            .shielded_damage(friend, 2, 150, &teams),
        50
    );
    assert!(restored
        .state
        .entities
        .get(caster)
        .unwrap()
        .shield
        .is_none());
    assert_eq!(
        restored
            .state
            .entities
            .shielded_damage(friend, 2, 20, &teams),
        20
    );
    game.state
        .entities
        .get_mut(caster)
        .unwrap()
        .apply_damage(u32::MAX, None);
    assert_eq!(
        game.state.entities.shielded_damage(friend, 2, 50, &teams),
        50
    );
}

#[test]
fn disciple_insight_is_team_private_but_readiness_and_advance_are_public() {
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
    game.state
        .fog
        .recompute(&[1, 2, 3], &game.state.entities, &game.state.map);
    for (viewer, insight) in [(1, Some(60)), (2, None), (3, Some(60))] {
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
        assert_eq!(view.insight, insight);
        assert_eq!(view.dao_ready, Some(true));
    }
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfLightness);
    game.enqueue(
        1,
        Command::UseAbility {
            ability: AbilityKind::EvolveLightness,
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
    assert_eq!(enemy_view.insight, None);
    assert_eq!(enemy_view.dao_ready, Some(false));
    assert!(enemy_view
        .dao_advance_ticks
        .is_some_and(|n| n > 0 && n <= 15));
    for _ in 0..15 {
        game.tick();
    }
    assert_eq!(game.state.entities.get(unit).unwrap().dao_advance_ticks, 0);
}
