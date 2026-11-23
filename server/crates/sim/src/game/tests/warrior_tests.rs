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

mod master_rank;

#[test]
fn cultivator_pavilion_research_spends_resources_completes_and_leaves_disciple_unchanged() {
    use crate::game::upgrade::{self, UpgradeKind};

    let mut game = empty_flat_game(&[cultivator_player()]);
    game.state.players[0].set_resources(350, 700);
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
        UpgradeKind::DaoOfMagic,
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
    for _ in 0..2100 {
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
        UpgradeKind::DaoOfMagic,
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
fn disciple_evolution_requires_insight_ownership_but_no_research_and_is_permanent() {
    use crate::game::ability::AbilityKind;
    use crate::game::upgrade::UpgradeKind;
    for (ability, upgrade, hp, speed) in [
        (
            AbilityKind::EvolveLightness,
            UpgradeKind::DaoOfLightness,
            115,
            2.7025,
        ),
        (
            AbilityKind::EvolveWar,
            UpgradeKind::DaoOfHeaviness,
            135,
            2.1,
        ),
        (AbilityKind::EvolveMagic, UpgradeKind::DaoOfMagic, 115, 2.1),
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
            target_id: None,
            ability,
            units: vec![unit, unit],
            x: None,
            y: None,
            queued: false,
        };
        let entity = game.state.entities.get_mut(unit).unwrap();
        entity.apply_damage(entity.hp - 1, None);
        game.enqueue(1, command.clone());
        game.tick();
        assert_eq!(game.state.entities.get(unit).unwrap().dao(), None);
        assert_eq!(game.state.entities.get(unit).unwrap().hp, 1);
        game.state.entities.get_mut(unit).unwrap().insight = 59;
        game.enqueue(1, command.clone());
        game.tick();
        assert_eq!(
            game.state.entities.get(unit).unwrap().dao(),
            None,
            "60 insight is required"
        );
        game.state.entities.get_mut(unit).unwrap().insight = 60;
        game.state.entities.get_mut(unit).unwrap().chosen_dao = None;
        game.enqueue(2, command.clone());
        game.tick();
        assert_eq!(
            game.state.entities.get(unit).unwrap().dao(),
            None,
            "ownership is required"
        );
        game.enqueue(1, command.clone());
        game.tick();
        let evolved = game.state.entities.get(unit).unwrap();
        assert_eq!(evolved.dao(), Some(upgrade));
        assert_eq!(Some(evolved.kind), EntityKind::evolved_disciple(upgrade));
        assert_ne!(evolved.kind, EntityKind::Warrior);
        let view = game
            .snapshot_for(1)
            .entities
            .into_iter()
            .find(|e| e.id == unit)
            .unwrap();
        assert_eq!(
            view.kind,
            EntityKind::evolved_disciple(upgrade).unwrap().stable_id()
        );
        assert_eq!(evolved.insight, 0);
        assert_eq!(evolved.insight_cap(), 120);
        assert_eq!((evolved.hp, evolved.max_hp), (hp, hp));
        assert!((evolved.movement_speed() - speed).abs() < 0.0001);
        let entity = game.state.entities.get_mut(unit).unwrap();
        entity.apply_damage(entity.hp - 1, None);
        game.enqueue(1, command);
        game.tick();
        assert_eq!(
            game.state.entities.get(unit).unwrap().max_hp,
            hp,
            "no repeated HP grants"
        );
        assert_eq!(
            game.state.entities.get(unit).unwrap().hp,
            1,
            "evolved units cannot heal by choosing a Dao again"
        );
        let payload = game.checkpoint_payload_text_for_test().unwrap();
        assert_eq!(game.state.entities.get(unit).unwrap().insight, 0);
        let restored = Game::restore_checkpoint_payload_text_for_test(
            &payload,
            game.state.map.clone(),
            game.state.map_metadata.clone(),
        )
        .unwrap();
        assert_eq!(
            restored.state.entities.get(unit).unwrap().dao(),
            Some(upgrade)
        );
        assert_eq!(
            Some(restored.state.entities.get(unit).unwrap().kind),
            EntityKind::evolved_disciple(upgrade)
        );
        assert!(
            !payload.contains("\"dao\":"),
            "checkpoint must not store a second Dao identity"
        );
        for insight in [0, 61, 120, 121] {
            game.state.entities.get_mut(unit).unwrap().insight = insight;
            let payload = game.checkpoint_payload_text_for_test();
            if insight > 120 {
                assert!(payload.is_err(), "over-cap insight must be rejected");
                continue;
            }
            let restored = Game::restore_checkpoint_payload_text_for_test(
                &payload.unwrap(),
                game.state.map.clone(),
                game.state.map_metadata.clone(),
            )
            .unwrap();
            assert_eq!(restored.state.entities.get(unit).unwrap().insight, insight);
        }
    }
}

#[test]
fn disciple_dash_casts_for_every_selected_ready_lightness_unit() {
    use crate::game::ability::AbilityKind;
    for queued in [false, true] {
        let enemy = PlayerInit {
            id: 2,
            team_id: 2,
            ..cultivator_player()
        };
        let mut game = empty_flat_game(&[cultivator_player(), enemy]);
        let mut units = Vec::new();
        let mut origins = Vec::new();
        for (index, owner) in [1, 1, 1, 1, 2].into_iter().enumerate() {
            let origin = game.state.map.tile_center(10, 10 + index as u32 * 2);
            let id = game
                .state
                .entities
                .spawn_unit(owner, EntityKind::Warrior, origin.0, origin.1)
                .unwrap();
            let entity = game.state.entities.get_mut(id).unwrap();
            if index != 3 {
                entity.set_disciple_kind_for_test(EntityKind::LightnessAdept);
            }
            if index == 2 {
                entity.start_ability_cooldown(AbilityKind::EkatTeleport, 100);
            }
            units.push(id);
            origins.push(origin);
        }
        let target = game.state.map.tile_center(14, 11);
        let mut selection = units.clone();
        selection.extend([units[0], u32::MAX]);
        game.enqueue(
            1,
            Command::UseAbility {
                target_id: None,
                ability: AbilityKind::EkatTeleport,
                units: selection,
                x: Some(target.0),
                y: Some(target.1),
                queued,
            },
        );
        // Queued orders may activate on the following tick.
        game.tick();
        game.tick();
        for (index, id) in units.iter().copied().enumerate() {
            let entity = game.state.entities.get(id).unwrap();
            if index < 2 {
                let dx = entity.pos_x() - target.0;
                let dy = entity.pos_y() - target.1;
                assert!(
                    dx * dx + dy * dy <= (config::TILE_SIZE * config::TILE_SIZE) as f32,
                    "each caster should reach the target area, allowing collision separation; queued={queued}",
                );
                assert!(entity.ability_cooldown_ticks(AbilityKind::EkatTeleport) > 0);
                let marker = game
                    .state
                    .ability_runtime
                    .active_return_marker(
                        1,
                        id,
                        AbilityKind::EkatTeleport,
                        None,
                        game.current_tick(),
                    )
                    .expect("each ready caster should leave its own return marker");
                assert_eq!((marker.x.to_world(), marker.y.to_world()), origins[index]);
            } else {
                assert_eq!((entity.pos_x(), entity.pos_y()), origins[index]);
            }
        }
        assert_eq!(game.state.ability_runtime.world_objects().count(), 2);
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
        target_id: None,
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
        .insert(UpgradeKind::DaoOfMagic);
    game.enqueue(
        1,
        Command::UseAbility {
            target_id: None,
            ability: AbilityKind::EvolveMagic,
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
            target_id: None,
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
            target_id: None,
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
        .insert(UpgradeKind::DaoOfMagic);
    let cast = |ability| Command::UseAbility {
        target_id: None,
        ability,
        units: vec![caster],
        x: Some(480.0),
        y: Some(400.0),
        queued: false,
    };
    game.enqueue(1, cast(AbilityKind::DiscipleShield));
    game.tick();
    assert!(game.state.entities.get(caster).unwrap().shield.is_none());
    game.enqueue(1, cast(AbilityKind::EvolveMagic));
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
    game.state.entities.get_mut(unit).unwrap().chosen_dao = Some(UpgradeKind::DaoOfMagic);
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
        assert_eq!(view.chosen_dao.as_deref(), insight.map(|_| "magic"));
        assert_eq!(view.dao_ready, Some(true));
    }
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfLightness);
    game.enqueue(
        1,
        Command::UseAbility {
            target_id: None,
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

#[test]
fn disciple_shield_does_not_identify_a_smoke_hidden_caster() {
    let enemy = PlayerInit {
        id: 2,
        team_id: 2,
        ..cultivator_player()
    };
    let mut game = empty_flat_game(&[cultivator_player(), enemy]);
    let caster = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    game.state
        .entities
        .spawn_unit(2, EntityKind::Worker, 520.0, 400.0)
        .unwrap();
    game.tick();
    let shield_id = game.state.ability_runtime.allocate_id().get();
    let unit = game.state.entities.get_mut(caster).unwrap();
    unit.insight = 60;
    unit.set_disciple_kind_for_test(EntityKind::MagicAdept);
    unit.place_shield(
        shield_id,
        480.0,
        400.0,
        300,
        config::DISCIPLE_SHIELD_DURATION_TICKS,
        true,
    );
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
    game.state
        .smokes
        .spawn(400.0, 400.0, 0.75, 100, game.state.tick)
        .unwrap();

    // The caster's tile is visible, but smoke excludes its entity from the enemy snapshot.
    assert!(game.state.fog.is_visible_world(2, 400.0, 400.0));
    let enemy_view = game.snapshot_for(2);
    assert!(!enemy_view.entities.iter().any(|e| e.id == caster));
    let shield = enemy_view
        .ability_objects
        .iter()
        .find(|o| o.id == shield_id)
        .unwrap();
    assert_eq!(shield.source_caster_id, None);
    assert_eq!(shield.owner_state, None);
    for snapshot in [game.snapshot_for(1), game.snapshot_full_for(2)] {
        let shield = snapshot
            .ability_objects
            .iter()
            .find(|o| o.id == shield_id)
            .unwrap();
        assert_eq!(shield.source_caster_id, Some(caster));
        assert_eq!(shield.owner_state.as_ref().unwrap().hp, Some(300));
    }
    game.state.smokes = crate::game::smoke::SmokeCloudStore::new();
    let visible = game.snapshot_for(2);
    assert!(visible.entities.iter().any(|e| e.id == caster));
    assert_eq!(
        visible
            .ability_objects
            .iter()
            .find(|o| o.id == shield_id)
            .unwrap()
            .source_caster_id,
        Some(caster)
    );
}

#[test]
fn disciple_pending_dao_replaces_for_selection_survives_checkpoint_and_waits_for_insight() {
    use crate::game::ability::AbilityKind;
    use crate::game::upgrade::UpgradeKind;
    let mut game = empty_flat_game(&[
        cultivator_player(),
        PlayerInit {
            id: 2,
            team_id: 2,
            ..cultivator_player()
        },
    ]);
    let first = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    let second = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 460.0, 400.0)
        .unwrap();
    let foreign = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Warrior, 900.0, 900.0)
        .unwrap();
    let untouched = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 520.0, 400.0)
        .unwrap();
    let cast = |ability| Command::UseAbility {
        target_id: None,
        ability,
        units: vec![first, second, first, foreign, u32::MAX],
        x: None,
        y: None,
        queued: false,
    };
    game.enqueue(
        1,
        Command::Move {
            units: vec![first, second],
            x: 650.0,
            y: 400.0,
            queued: false,
        },
    );
    game.enqueue(1, cast(AbilityKind::EvolveWar));
    game.tick();
    for id in [first, second] {
        let entity = game.state.entities.get(id).unwrap();
        assert_eq!(entity.chosen_dao, Some(UpgradeKind::DaoOfHeaviness));
        assert!(
            matches!(entity.order(), Order::Move(_)),
            "choosing a Dao preserves movement"
        );
        assert_eq!(entity.dao(), None);
    }
    assert_eq!(game.state.entities.get(foreign).unwrap().chosen_dao, None);
    assert_eq!(game.state.entities.get(untouched).unwrap().chosen_dao, None);
    game.enqueue(1, cast(AbilityKind::EvolveMagic));
    game.tick();
    let hash = game.replay_end_state_hash().unwrap();
    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let mut game = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    assert_eq!(game.replay_end_state_hash().unwrap(), hash);
    for id in [first, second] {
        assert_eq!(
            game.state.entities.get(id).unwrap().chosen_dao,
            Some(UpgradeKind::DaoOfMagic)
        );
    }
    game.state.entities.get_mut(first).unwrap().insight = 60;
    game.tick();
    assert_eq!(
        game.state.entities.get(first).unwrap().dao(),
        Some(UpgradeKind::DaoOfMagic),
        "combat insight evolves the pending path without any Pavilion or research"
    );
    assert!(game.state.players[0].upgrades.is_empty());
    assert_eq!(game.state.entities.get(first).unwrap().chosen_dao, None);
    assert_eq!(
        game.state.entities.get(second).unwrap().dao(),
        None,
        "research alone does not evolve"
    );
    game.state.entities.get_mut(second).unwrap().insight = 60;
    game.tick();
    assert_eq!(
        game.state.entities.get(second).unwrap().dao(),
        Some(UpgradeKind::DaoOfMagic),
        "insight triggers without another click"
    );
    assert_eq!(
        game.state.entities.get(untouched).unwrap().dao(),
        None,
        "research never assigns an unselected Disciple"
    );
    game.enqueue(1, cast(AbilityKind::EvolveWar));
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfHeaviness);
    game.tick();
    assert_eq!(
        game.state.entities.get(first).unwrap().dao(),
        Some(UpgradeKind::DaoOfMagic),
        "evolved path is permanent"
    );
}

#[test]
fn dead_disciple_cannot_advance_pending_war_and_regain_hp() {
    use crate::game::ability::AbilityKind;
    use crate::game::upgrade::UpgradeKind;
    let mut game = empty_flat_game(&[cultivator_player()]);
    let unit = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    let entity = game.state.entities.get_mut(unit).unwrap();
    assert!(entity.choose_dao(AbilityKind::EvolveWar));
    entity.insight = 60;
    entity.apply_damage(u32::MAX, None);
    entity.advance_chosen_dao();
    assert_eq!(entity.hp, 0);
    assert_eq!(entity.dao(), None);
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfHeaviness);
    game.tick();
    assert!(game.state.entities.get(unit).is_none());
}

#[test]
fn disciple_pending_dao_advances_on_combat_insight_without_interrupting_attack() {
    use crate::game::ability::AbilityKind;
    use crate::game::upgrade::UpgradeKind;
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
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Warrior, 425.0, 400.0)
        .unwrap();
    let entity = game.state.entities.get_mut(unit).unwrap();
    entity.insight = 59;
    entity.choose_dao(AbilityKind::EvolveWar);
    entity.set_order(Order::attack(target));
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfHeaviness);
    for _ in 0..60 {
        game.tick();
        if game.state.entities.get(unit).unwrap().dao().is_some() {
            break;
        }
    }
    let entity = game.state.entities.get(unit).unwrap();
    assert_eq!(entity.insight, 0);
    assert_eq!(entity.dao(), Some(UpgradeKind::DaoOfHeaviness));
    assert!(matches!(entity.order(), Order::Attack(_)));
    assert!(game.state.entities.get(target).unwrap().hp < 115);
}

#[test]
fn war_throw_sword_is_dao_gated_and_hits_harder_on_return_after_checkpoint() {
    use crate::game::ability::AbilityKind;
    use crate::game::upgrade::UpgradeKind;
    let mut game = empty_flat_game(&[cultivator_player(), human_vs_ai_players()[1].clone()]);
    let caster = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    game.state
        .entities
        .get_mut(caster)
        .unwrap()
        .replace_active_order(Order::HoldPosition);
    let victim = game
        .state
        .entities
        .spawn_building(2, EntityKind::ResourceDepot, 560.0, 476.0, true)
        .unwrap();
    let cast = Command::UseAbility {
        target_id: None,
        ability: AbilityKind::ThrowSword,
        units: vec![caster],
        x: Some(1000.0),
        y: Some(400.0),
        queued: false,
    };
    for dao in [
        None,
        Some(UpgradeKind::DaoOfLightness),
        Some(UpgradeKind::DaoOfMagic),
    ] {
        let unit = game.state.entities.get_mut(caster).unwrap();
        unit.set_disciple_kind_for_test(
            dao.and_then(EntityKind::evolved_disciple)
                .unwrap_or(EntityKind::Warrior),
        );
        // Isolate ability gating from the new ordinary ranged attacks.
        unit.set_attack_cd(1000);
        game.enqueue(1, cast.clone());
        game.tick();
        assert_eq!(game.state.ability_runtime.world_objects().count(), 0);
    }
    game.state.entities.get_mut(caster).unwrap().insight = 0;
    game.state
        .entities
        .get_mut(caster)
        .unwrap()
        .set_disciple_kind_for_test(EntityKind::WarAdept);
    let fragile = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, 440.0, 400.0)
        .unwrap();
    game.state
        .entities
        .get_mut(fragile)
        .unwrap()
        .set_spawn_health(7);
    let protected = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, 460.0, 400.0)
        .unwrap();
    game.state
        .entities
        .get_mut(protected)
        .unwrap()
        .set_invulnerable(true);
    for id in [fragile, protected] {
        let target = game.state.entities.get_mut(id).unwrap();
        target.set_attack_cd(1000);
        target.replace_active_order(Order::HoldPosition);
    }
    game.enqueue(1, cast);
    game.tick();
    // Throw Sword clears the active order. Keep ordinary melee out of this projectile test.
    game.state
        .entities
        .get_mut(caster)
        .unwrap()
        .replace_active_order(Order::HoldPosition);
    let object = game.state.ability_runtime.world_objects().next().unwrap();
    assert_eq!(object.ability, AbilityKind::ThrowSword);
    assert!((object.x.to_world() - (400.0 + 40.0 / 7.0)).abs() < 0.01);
    assert_eq!(object.expires_tick - object.created_tick, 114);
    assert_eq!(
        game.state
            .entities
            .get(caster)
            .unwrap()
            .ability_cooldown_ticks(AbilityKind::ThrowSword),
        config::EKAT_LINE_SHOT_COOLDOWN_TICKS - 1
    );
    for _ in 0..30 {
        game.tick();
    }
    assert_eq!(
        game.state.entities.get(victim).unwrap().hp,
        140,
        "exactly one 40 damage outbound hit"
    );
    assert_eq!(
        game.state.entities.get(caster).unwrap().insight,
        27,
        "projectile insight halves building HP loss, excluding overkill and invulnerability"
    );
    // The depot is 76 px off the path: outside the former 64 + 9.6 px hit radius.
    // The throw reaches 7.5 tiles after 42 ticks: a 2.8-second nominal round trip.
    for _ in 0..11 {
        game.tick();
    }
    let object = game.state.ability_runtime.world_objects().next().unwrap();
    assert!((object.x.to_world() - 640.0).abs() < 0.01);
    // A round trip through a checkpoint retains the projectile identity, leg and hit sets.
    let checkpoint = game.checkpoint_payload_text_for_test().unwrap();
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &checkpoint,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    for _ in 0..41 {
        game.tick();
        restored.tick();
    }
    assert_eq!(game.state.ability_runtime.world_objects().count(), 1);
    assert_eq!(restored.state.ability_runtime.world_objects().count(), 1);
    game.tick();
    restored.tick();
    assert_eq!(
        game.state.entities.get(victim).unwrap().hp,
        60,
        "exactly one 80 damage return hit"
    );
    assert_eq!(restored.state.entities.get(victim).unwrap().hp, 60);
    assert_eq!(game.state.entities.get(caster).unwrap().insight, 67);
    assert_eq!(restored.state.entities.get(caster).unwrap().insight, 67);
    assert_eq!(game.state.ability_runtime.world_objects().count(), 0);
}

#[test]
fn dao_parallel_research_reserves_tiers_and_refunds_committed_costs() {
    use crate::game::upgrade::UpgradeKind;
    let mut game = empty_flat_game(&[cultivator_player()]);
    game.state.players[0].set_resources(1000, 1000);
    let daos = [
        UpgradeKind::DaoOfMagic,
        UpgradeKind::DaoOfHeaviness,
        UpgradeKind::DaoOfLightness,
    ];
    let mut buildings = Vec::new();
    for (index, dao) in daos.into_iter().enumerate() {
        let (x, y) = game.state.map.tile_center(8 + index as u32 * 5, 12);
        let id = game
            .state
            .entities
            .spawn_building(1, EntityKind::ThreeDaoPavilion, x, y, true)
            .unwrap();
        buildings.push(id);
        game.enqueue(
            1,
            Command::Research {
                building: id,
                upgrade: dao,
            },
        );
    }
    game.tick();
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (650, 300)
    );
    for (id, total) in buildings.iter().zip([300, 600, 1200]) {
        assert_eq!(
            game.state.entities.get(*id).unwrap().research_queue()[0].total,
            total
        );
    }
    // Removing the cheap research must not lower the refunds for the other paid tiers.
    game.enqueue(
        1,
        Command::Cancel {
            building: buildings[0],
            construction: false,
        },
    );
    game.tick();
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (700, 400)
    );
    game.state
        .entities
        .get_mut(buildings[2])
        .unwrap()
        .apply_damage(u32::MAX, None);
    game.tick();
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (900, 800)
    );
    game.enqueue(
        1,
        Command::Cancel {
            building: buildings[1],
            construction: false,
        },
    );
    game.tick();
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (1000, 1000)
    );
}

#[test]
fn dao_waiting_research_reprices_when_payment_starts() {
    use crate::game::upgrade::UpgradeKind;

    let mut game = empty_flat_game(&[cultivator_player()]);
    game.state.players[0].set_resources(0, 0);
    let mut buildings = Vec::new();
    for (index, dao) in [UpgradeKind::DaoOfMagic, UpgradeKind::DaoOfLightness]
        .into_iter()
        .enumerate()
    {
        let (x, y) = game.state.map.tile_center(8 + index as u32 * 5, 12);
        let id = game
            .state
            .entities
            .spawn_building(1, EntityKind::ThreeDaoPavilion, x, y, true)
            .unwrap();
        buildings.push(id);
        game.enqueue(
            1,
            Command::Research {
                building: id,
                upgrade: dao,
            },
        );
    }
    game.tick();
    for id in &buildings {
        let item = &game.state.entities.get(*id).unwrap().research_queue()[0];
        assert!(!item.paid);
        assert_eq!((item.total, item.progress), (300, 0));
    }

    game.state.players[0].set_resources(50, 100);
    game.tick();
    assert!(
        game.state
            .entities
            .get(buildings[0])
            .unwrap()
            .research_queue()[0]
            .paid
    );
    let waiting = &game
        .state
        .entities
        .get(buildings[1])
        .unwrap()
        .research_queue()[0];
    assert!(!waiting.paid);
    assert_eq!(waiting.progress, 0);

    // The second queue was admitted at the cheap tier, but must pay the current tier.
    game.state.players[0].set_resources(100, 200);
    game.tick();
    let started = &game
        .state
        .entities
        .get(buildings[1])
        .unwrap()
        .research_queue()[0];
    assert!(started.paid);
    assert_eq!((started.total, started.progress), (600, 1));
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (0, 0)
    );
}

#[test]
fn disciple_dash_doubles_range_for_immediate_and_queued_casts() {
    use crate::game::ability::AbilityKind;
    for queued in [false, true] {
        for requested_tiles in [8.0, 20.0] {
            let mut game = empty_flat_game(&[cultivator_player()]);
            let origin = game.state.map.tile_center(10, 10);
            let unit = game
                .state
                .entities
                .spawn_unit(1, EntityKind::Warrior, origin.0, origin.1)
                .unwrap();
            game.state
                .entities
                .get_mut(unit)
                .unwrap()
                .set_disciple_kind_for_test(EntityKind::LightnessAdept);
            game.enqueue(
                1,
                Command::UseAbility {
                    target_id: None,
                    ability: AbilityKind::EkatTeleport,
                    units: vec![unit],
                    x: Some(origin.0 + requested_tiles * config::TILE_SIZE as f32),
                    y: Some(origin.1),
                    queued,
                },
            );
            game.tick();
            game.tick();
            let entity = game.state.entities.get(unit).unwrap();
            let expected = origin.0 + requested_tiles.min(10.0) * config::TILE_SIZE as f32;
            assert!((entity.pos_x() - expected).abs() < 0.001);
            assert!((entity.pos_y() - origin.1).abs() < 0.001);
            assert!(game
                .state
                .ability_runtime
                .active_return_marker(
                    1,
                    unit,
                    AbilityKind::EkatTeleport,
                    None,
                    game.current_tick()
                )
                .is_some());
        }
    }
}
