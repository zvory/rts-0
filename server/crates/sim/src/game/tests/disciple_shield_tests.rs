use super::fixtures::*;
use super::*;
use crate::game::ability::AbilityKind;

fn setup() -> (Game, u32) {
    let player = PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::CULTIVATORS_FACTION_ID.into(),
        name: "Shield test".into(),
        color: "#fff".into(),
        is_ai: false,
    };
    let enemy = PlayerInit {
        id: 2,
        team_id: 2,
        ..player.clone()
    };
    let mut game = empty_flat_game(&[player, enemy]);
    game.state.players[0]
        .upgrades
        .insert(crate::game::upgrade::UpgradeKind::TechniqueShield);
    let caster = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    game.state
        .entities
        .get_mut(caster)
        .unwrap()
        .set_disciple_kind_for_test(EntityKind::MagicAdept);
    game.state.entities.get_mut(caster).unwrap().insight = config::DISCIPLE_INSIGHT_REQUIRED;
    (game, caster)
}

fn cast(caster: u32, x: f32, queued: bool) -> Command {
    Command::UseAbility {
        target_id: None,
        ability: AbilityKind::DiscipleShield,
        units: vec![caster],
        x: Some(x),
        y: Some(400.0),
        queued,
    }
}

#[test]
fn disciple_shield_keeps_five_tile_cast_range() {
    for (range_tiles, should_cast) in [(5.0, true), (5.1, false)] {
        let (mut game, caster) = setup();
        game.enqueue(
            1,
            cast(
                caster,
                400.0 + range_tiles * config::TILE_SIZE as f32,
                false,
            ),
        );
        game.tick();
        assert_eq!(
            game.state.entities.get(caster).unwrap().shield.is_some(),
            should_cast,
            "Shield must retain its own range independently of Magic Anchor"
        );
    }
}

#[test]
fn disciple_shield_in_range_preserves_movement_and_future_orders() {
    for attack_move in [false, true] {
        let (mut game, caster) = setup();
        let movement = if attack_move {
            Command::AttackMove {
                units: vec![caster],
                x: 800.0,
                y: 400.0,
                queued: false,
            }
        } else {
            Command::Move {
                units: vec![caster],
                x: 800.0,
                y: 400.0,
                queued: false,
            }
        };
        game.enqueue(1, movement);
        game.tick();
        game.enqueue(
            1,
            Command::Move {
                units: vec![caster],
                x: 900.0,
                y: 400.0,
                queued: true,
            },
        );
        game.tick();
        let order = game.state.entities.get(caster).unwrap().order();
        let queued = game
            .state
            .entities
            .get(caster)
            .unwrap()
            .queued_orders()
            .to_vec();
        let before = game.state.entities.get(caster).unwrap().pos_x();
        game.enqueue(1, cast(caster, 480.0, false));
        game.tick();
        let unit = game.state.entities.get(caster).unwrap();
        assert!(
            unit.shield.is_some(),
            "in-range cast must work while moving"
        );
        assert_eq!(unit.order(), order);
        assert_eq!(unit.queued_orders(), queued);
        assert!(unit.pos_x() > before, "casting must not pause movement");
        assert!(unit.ability_cooldown_ticks(AbilityKind::DiscipleShield) > 0);
    }
}

#[test]
fn disciple_shield_in_range_preserves_busy_attack_order() {
    let (mut game, caster) = setup();
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Warrior, 520.0, 400.0)
        .unwrap();
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
    game.enqueue(
        1,
        Command::Attack {
            units: vec![caster],
            target,
            queued: false,
        },
    );
    game.tick();
    let order = game.state.entities.get(caster).unwrap().order();
    assert!(matches!(order, Order::Attack(_)));
    game.enqueue(1, cast(caster, 480.0, false));
    game.tick();
    let unit = game.state.entities.get(caster).unwrap();
    assert!(unit.shield.is_some());
    assert_eq!(unit.order(), order);
}

#[test]
fn disciple_shield_out_of_range_replaces_move_and_casts_after_approach() {
    let (mut game, caster) = setup();
    game.enqueue(
        1,
        Command::Move {
            units: vec![caster],
            x: 400.0,
            y: 800.0,
            queued: false,
        },
    );
    game.tick();
    game.enqueue(1, cast(caster, 800.0, false));
    game.tick();
    let unit = game.state.entities.get(caster).unwrap();
    assert!(unit.shield.is_none());
    assert!(matches!(unit.order(), Order::Ability(_)));
    for _ in 0..400 {
        game.tick();
        if game.state.entities.get(caster).unwrap().shield.is_some() {
            break;
        }
    }
    let shield = game
        .state
        .entities
        .get(caster)
        .unwrap()
        .shield
        .as_ref()
        .unwrap();
    assert_eq!(shield.x.to_world(), 800.0);
}

#[test]
fn disciple_shield_shift_waits_for_move_and_cooldown_does_not_replace_field() {
    let (mut game, caster) = setup();
    game.enqueue(
        1,
        Command::Move {
            units: vec![caster],
            x: 450.0,
            y: 400.0,
            queued: false,
        },
    );
    game.tick();
    game.enqueue(1, cast(caster, 480.0, true));
    game.tick();
    assert!(game.state.entities.get(caster).unwrap().shield.is_none());
    for _ in 0..150 {
        game.tick();
        if game.state.entities.get(caster).unwrap().shield.is_some() {
            break;
        }
    }
    let id = game
        .state
        .entities
        .get(caster)
        .unwrap()
        .shield
        .as_ref()
        .unwrap()
        .id;
    game.enqueue(1, cast(caster, 500.0, false));
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(caster)
            .unwrap()
            .shield
            .as_ref()
            .unwrap()
            .id,
        id
    );
}

#[test]
fn disciple_shield_expires_at_10_seconds_after_checkpoint_restore() {
    let (mut game, caster) = setup();
    game.enqueue(1, cast(caster, 440.0, false));
    game.tick();
    assert_eq!(config::DISCIPLE_SHIELD_DURATION_TICKS, 10 * config::TICK_HZ);
    let expires = game.state.tick + config::DISCIPLE_SHIELD_DURATION_TICKS;
    assert_eq!(
        game.state
            .entities
            .get(caster)
            .unwrap()
            .shield
            .as_ref()
            .unwrap()
            .expires_tick,
        expires
    );
    for _ in 0..100 {
        game.tick();
    }
    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    for game in [&mut game, &mut restored] {
        while game.state.tick < expires - 1 {
            game.tick();
        }
        let view = game
            .snapshot_for(1)
            .ability_objects
            .into_iter()
            .find(|o| o.ability == "discipleShield")
            .unwrap();
        assert_eq!(view.expires_in, Some(1));
        assert_eq!(
            game.state.entities.shielded_damage(
                caster,
                2,
                20,
                20,
                0.0,
                (0.0, 0.0),
                &crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)])
            ),
            0
        );
        game.tick();
        assert!(game.state.entities.get(caster).unwrap().shield.is_none());
        assert!(!game
            .snapshot_for(1)
            .ability_objects
            .iter()
            .any(|o| o.ability == "discipleShield"));
        assert_eq!(
            game.state.entities.shielded_damage(
                caster,
                2,
                20,
                20,
                0.0,
                (0.0, 0.0),
                &crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)])
            ),
            20
        );
    }
}

#[test]
fn disciple_shield_recast_after_cooldown_gets_fresh_expiry() {
    let (mut game, caster) = setup();
    game.enqueue(1, cast(caster, 480.0, false));
    game.tick();
    let original = game
        .state
        .entities
        .get(caster)
        .unwrap()
        .shield
        .clone()
        .unwrap();
    while game
        .state
        .entities
        .get(caster)
        .unwrap()
        .ability_cooldown_ticks(AbilityKind::DiscipleShield)
        > 0
    {
        game.tick();
    }
    assert_eq!(game.state.tick + 1, original.expires_tick);
    assert!(game.state.entities.get(caster).unwrap().shield.is_some());
    game.enqueue(1, cast(caster, 500.0, false));
    game.tick();
    let replacement = game
        .state
        .entities
        .get(caster)
        .unwrap()
        .shield
        .as_ref()
        .unwrap();
    assert_ne!(replacement.id, original.id);
    assert_eq!(replacement.remaining, 150);
    assert_eq!(replacement.x.to_world(), 500.0);
    assert_eq!(
        replacement.expires_tick,
        game.state.tick + config::DISCIPLE_SHIELD_DURATION_TICKS
    );
    while game.state.tick <= original.expires_tick {
        game.tick();
    }
    assert!(game.state.entities.get(caster).unwrap().shield.is_some());
}

#[test]
fn disciple_shield_snapshots_track_full_hit_cost_and_remove_depleted_pool() {
    let (mut game, caster) = setup();
    game.enqueue(1, cast(caster, 440.0, false));
    game.tick();
    let teams = crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    for (damage, remaining) in [(0, 150), (50, 100), (50, 50)] {
        assert_eq!(
            game.state
                .entities
                .shielded_damage(caster, 2, damage, damage, 0.0, (0.0, 0.0), &teams),
            0
        );
        let shield = game
            .snapshot_full_for(1)
            .ability_objects
            .into_iter()
            .find(|object| object.ability == "discipleShield")
            .unwrap();
        assert_eq!(shield.owner_state.unwrap().hp, Some(remaining));
    }
    assert_eq!(
        game.state
            .entities
            .shielded_damage(caster, 2, 70, 70, 0.0, (0.0, 0.0), &teams),
        0
    );
    assert!(!game
        .snapshot_full_for(1)
        .ability_objects
        .iter()
        .any(|object| object.ability == "discipleShield"));
}

#[test]
fn disciple_shield_only_protects_inside_targets_and_piercing_still_drains_hp() {
    let (mut game, caster) = setup();
    game.enqueue(1, cast(caster, 480.0, false));
    game.tick();
    let teams = crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let radius = config::DISCIPLE_SHIELD_RADIUS_TILES * config::TILE_SIZE as f32;
    let inside = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, 480.0 + radius - 1.0, 400.0)
        .unwrap();
    let outside = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, 480.0 + radius + 1.0, 400.0)
        .unwrap();
    assert_eq!(
        game.state
            .entities
            .shielded_damage(outside, 2, 20, 20, 0.0, (0.0, 0.0), &teams),
        20
    );
    assert_eq!(
        game.state
            .entities
            .shielded_damage(inside, 2, 20, 20, 1.0, (0.0, 0.0), &teams),
        20
    );
    assert_eq!(
        game.state
            .entities
            .shielded_damage(inside, 2, 20, 20, 0.0, (0.0, 0.0), &teams),
        0
    );
    let shield = game
        .snapshot_full_for(1)
        .ability_objects
        .into_iter()
        .find(|o| o.ability == "discipleShield")
        .unwrap();
    let state = shield.owner_state.unwrap();
    assert_eq!(state.hp, Some(110));
    assert_eq!(state.radius, Some(radius));
}

#[test]
fn disciple_shield_armor_uses_normal_piercing_and_drains_full_hits_without_stacking() {
    let teams = crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    for (penetration, damage, expected_hit, expected_pool) in [
        (0.0, 100, 0, 50),
        (0.25, 100, 25, 50),
        (0.5, 100, 50, 50),
        (0.75, 100, 75, 50),
        (0.5, 5, 3, 145),
        (1.0, 100, 100, 50),
        (0.5, 400, 200, 0),
    ] {
        let (mut game, caster) = setup();
        game.enqueue(1, cast(caster, 440.0, false));
        game.tick();
        assert_eq!(
            game.state.entities.shielded_damage(
                caster,
                2,
                damage,
                damage,
                penetration,
                (0.0, 0.0),
                &teams
            ),
            expected_hit
        );
        assert_eq!(
            game.state
                .entities
                .get(caster)
                .unwrap()
                .shield
                .as_ref()
                .map_or(0, |s| s.remaining),
            expected_pool
        );
    }
    let (mut game, caster) = setup();
    game.enqueue(1, cast(caster, 440.0, false));
    game.tick();
    let second = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MagicAdept, 440.0, 420.0)
        .unwrap();
    game.state
        .entities
        .get_mut(second)
        .unwrap()
        .place_shield(99, 440.0, 400.0, 0, 150, true);
    assert_eq!(
        game.state
            .entities
            .shielded_damage(caster, 2, 400, 400, 0.5, (0.0, 0.0), &teams),
        200
    );
    assert!(game.state.entities.get(caster).unwrap().shield.is_none());
    assert_eq!(
        game.state
            .entities
            .get(second)
            .unwrap()
            .shield
            .as_ref()
            .unwrap()
            .remaining,
        150
    );
}

#[test]
fn disciple_shield_requires_outside_source_and_inside_target() {
    let teams = crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    for penetration in [0.0, 0.5, 1.0] {
        for source in [
            (440.0, 400.0),
            (
                440.0 + config::DISCIPLE_SHIELD_RADIUS_TILES * config::TILE_SIZE as f32 - 0.01,
                400.0,
            ),
        ] {
            let (mut game, caster) = setup();
            game.enqueue(1, cast(caster, 440.0, false));
            game.tick();
            assert_eq!(
                game.state
                    .entities
                    .shielded_damage(caster, 2, 40, 40, penetration, source, &teams),
                40
            );
            assert_eq!(
                game.state
                    .entities
                    .get(caster)
                    .unwrap()
                    .shield
                    .as_ref()
                    .unwrap()
                    .remaining,
                150
            );
        }
    }
}

#[test]
fn disciple_shield_hp_and_expiry_are_public_but_hidden_fields_and_casters_stay_hidden() {
    use crate::game::ObserverView;
    let (mut game, caster) = setup();
    game.enqueue(1, cast(caster, 440.0, false));
    game.tick();
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, 440.0, 400.0)
        .unwrap();
    // The field is visible even though its caster is far away in fog.
    game.state
        .entities
        .get_mut(caster)
        .unwrap()
        .set_position(1000.0, 1000.0);
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
    for snapshot in [
        game.snapshot_for(2),
        game.snapshot_for_observer(&ObserverView::Players(vec![2])),
        game.snapshot_for_observer(&ObserverView::Omniscient),
    ] {
        let shield = snapshot
            .ability_objects
            .iter()
            .find(|o| o.ability == "discipleShield")
            .unwrap();
        assert_eq!(shield.owner_state.as_ref().unwrap().hp, Some(150));
        assert_eq!(
            shield.expires_in,
            Some(config::DISCIPLE_SHIELD_DURATION_TICKS as u16)
        );
    }
    let shield = game
        .snapshot_for(2)
        .ability_objects
        .into_iter()
        .find(|o| o.ability == "discipleShield")
        .unwrap();
    assert!(shield.source_caster_id.is_none());
    game.state
        .entities
        .get_mut(enemy)
        .unwrap()
        .set_position(1000.0, 1000.0);
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
    assert!(!game
        .snapshot_for(2)
        .ability_objects
        .iter()
        .any(|o| o.ability == "discipleShield"));
}

#[test]
fn disciple_shield_full_attack_cost_ignores_existing_reductions_and_breaking_hit_is_armored() {
    let (mut game, caster) = setup();
    game.enqueue(1, cast(caster, 440.0, false));
    game.tick();
    let teams = crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    // Existing armor/cover reduced a 100-damage attack to 20 before shield armor.
    assert_eq!(
        game.state
            .entities
            .shielded_damage(caster, 2, 20, 100, 0.0, (0.0, 0.0), &teams),
        0
    );
    assert_eq!(
        game.state
            .entities
            .get(caster)
            .unwrap()
            .shield
            .as_ref()
            .unwrap()
            .remaining,
        50
    );
    assert_eq!(
        game.state
            .entities
            .shielded_damage(caster, 2, 100, 100, 0.0, (0.0, 0.0), &teams),
        0
    );
    assert!(game.state.entities.get(caster).unwrap().shield.is_none());
    assert_eq!(
        game.state
            .entities
            .shielded_damage(caster, 2, 100, 100, 0.0, (0.0, 0.0), &teams),
        100
    );
}

#[test]
fn magic_shield_grants_no_insight_across_checkpoint_consumption_and_expiry() {
    let (mut game, caster) = setup();
    game.tick();
    let unit = game.state.entities.get_mut(caster).unwrap();
    unit.insight = 0;
    let shield_id = game.state.ability_runtime.allocate_id().get();
    game.state
        .entities
        .get_mut(caster)
        .unwrap()
        .place_shield(shield_id, 400.0, 400.0, 0, 100, true);
    let teams = game.team_relations();
    for _ in 0..7 {
        game.state
            .entities
            .shielded_damage(caster, 2, 1, 1, 0.0, (0.0, 0.0), &teams);
    }
    assert_eq!(game.state.entities.get(caster).unwrap().insight, 0);
    let checkpoint = game.checkpoint_payload_text_for_test().unwrap();
    let mut game = Game::restore_checkpoint_payload_text_for_test(
        &checkpoint,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    game.state
        .entities
        .shielded_damage(caster, 2, 1000, 1000, 1.0, (0.0, 0.0), &teams);
    let unit = game.state.entities.get_mut(caster).unwrap();
    assert_eq!(
        unit.insight, 0,
        "shield consumption and overkill grant no insight"
    );
    assert!(unit.shield.is_none());
    unit.place_shield(2, 400.0, 400.0, 0, 200, true);
    unit.expire_shield(200);
    assert_eq!(unit.insight, 0, "expiration grants nothing");
    unit.insight = 199;
    unit.gain_magic_insight(20);
    assert_eq!(unit.insight, 200);
    unit.advance_dao_master();
    unit.gain_magic_insight(90);
    assert_eq!(unit.insight, 0, "Masters gain nothing");
}

#[test]
fn magic_support_insight_anchor_rewards_enemy_unit_deaths_once_per_caster() {
    use crate::game::ability_runtime::{
        AbilityObjectPayload, AbilityWorldObjectKind, AbilityWorldObjectSpec,
    };
    let (mut game, caster) = setup();
    game.state.entities.get_mut(caster).unwrap().insight = 0;
    game.state
        .ability_runtime
        .spawn_world_object(AbilityWorldObjectSpec {
            owner: 1,
            caster_id: caster,
            ability: AbilityKind::EkatMagicAnchor,
            kind: AbilityWorldObjectKind::MagicAnchor,
            x: 400.0,
            y: 400.0,
            created_tick: 0,
            expires_tick: 100,
            payload: AbilityObjectPayload::MagicAnchor { radius: 100.0 },
        })
        .unwrap();
    for (owner, x, kind) in [
        (2, 410.0, EntityKind::Rifleman),
        (2, 420.0, EntityKind::Tank),
        (1, 430.0, EntityKind::Rifleman),
        (2, 700.0, EntityKind::Rifleman),
    ] {
        let id = game
            .state
            .entities
            .spawn_unit(owner, kind, x, 400.0)
            .unwrap();
        game.state
            .entities
            .get_mut(id)
            .unwrap()
            .apply_damage(u32::MAX, None);
    }
    let building = game
        .state
        .entities
        .spawn_building(2, EntityKind::ResourceDepot, 440.0, 400.0, true)
        .unwrap();
    game.state
        .entities
        .get_mut(building)
        .unwrap()
        .apply_damage(u32::MAX, None);
    game.tick();
    assert_eq!(game.state.entities.get(caster).unwrap().insight, 40);
    game.tick();
    assert_eq!(
        game.state.entities.get(caster).unwrap().insight,
        40,
        "removed deaths cannot reward twice"
    );
}

#[test]
fn magic_master_shield_doubles_capacity_extends_lifetime_and_survives_checkpoint() {
    for master in [false, true] {
        let (mut game, caster) = setup();
        if master {
            let unit = game.state.entities.get_mut(caster).unwrap();
            unit.insight = config::DISCIPLE_EVOLVED_INSIGHT_CAP;
            unit.advance_dao_master();
        }
        game.enqueue(1, cast(caster, 400.0, false));
        game.tick();
        let unit = game.state.entities.get(caster).unwrap();
        let shield = unit.shield.as_ref().unwrap();
        let capacity = if master { 300 } else { 150 };
        let duration = config::TICK_HZ * if master { 15 } else { 10 };
        assert_eq!(shield.remaining, capacity);
        assert_eq!(shield.duration_ticks, duration);
        assert_eq!(shield.expires_tick - game.state.tick, duration);
        let snapshot = game.snapshot_full_for(1);
        let view = snapshot
            .ability_objects
            .iter()
            .find(|o| o.id == shield.id)
            .unwrap();
        let public = view.owner_state.as_ref().unwrap();
        assert_eq!(public.hp, Some(capacity as u16));
        assert_eq!(public.max_hp, Some(capacity as u16));
        assert_eq!(public.duration_ticks, Some(duration));
        assert_eq!(view.expires_in, Some(duration as u16));
        let checkpoint = game.checkpoint_payload_text_for_test().unwrap();
        let restored = Game::restore_checkpoint_payload_text_for_test(
            &checkpoint,
            game.state.map.clone(),
            game.state.map_metadata.clone(),
        )
        .unwrap();
        assert_eq!(
            restored
                .state
                .entities
                .get(caster)
                .unwrap()
                .shield
                .as_ref()
                .unwrap()
                .remaining,
            capacity
        );
        let expires = shield.expires_tick;
        let unit = game.state.entities.get_mut(caster).unwrap();
        unit.expire_shield(expires - 1);
        assert!(unit.shield.is_some());
        unit.expire_shield(expires);
        assert!(unit.shield.is_none());
    }
}

#[test]
fn promotion_preserves_existing_adept_shield_strength() {
    let (mut game, caster) = setup();
    game.enqueue(1, cast(caster, 400.0, false));
    game.tick();
    let unit = game.state.entities.get_mut(caster).unwrap();
    let expires = unit.shield.as_ref().unwrap().expires_tick;
    unit.insight = config::DISCIPLE_EVOLVED_INSIGHT_CAP;
    unit.advance_dao_master();
    let shield = unit.shield.as_ref().unwrap();
    assert_eq!(shield.capacity, 150);
    assert_eq!(shield.duration_ticks, config::TICK_HZ * 10);
    assert_eq!(shield.expires_tick, expires);
    let checkpoint = game.checkpoint_payload_text_for_test().unwrap();
    Game::restore_checkpoint_payload_text_for_test(
        &checkpoint,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
}

#[test]
fn disciple_shield_building_coverage_uses_half_the_field_area() {
    let teams = crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2), (3, 1)]);
    // Resource Depot footprint is [416, 544] on both axes. Edge placement can cover
    // enough of the field without containing the building; corner slivers cannot.
    for (owner, center, invulnerable, source, piercing, expected) in [
        (1, (480.0, 480.0), false, (0.0, 0.0), 0.0, 0),
        (1, (536.0, 480.0), false, (0.0, 0.0), 0.0, 0),
        (1, (552.0, 480.0), false, (0.0, 0.0), 0.0, 20),
        (1, (536.0, 536.0), false, (0.0, 0.0), 0.0, 20),
        (3, (480.0, 480.0), false, (0.0, 0.0), 0.0, 0),
        (2, (480.0, 480.0), false, (0.0, 0.0), 0.0, 20),
        (1, (480.0, 480.0), true, (0.0, 0.0), 0.0, 20),
        (1, (480.0, 480.0), false, (480.0, 480.0), 0.0, 20),
        (1, (480.0, 480.0), false, (0.0, 0.0), 1.0, 20),
    ] {
        let (mut game, caster) = setup();
        game.state
            .entities
            .get_mut(caster)
            .unwrap()
            .place_shield(1, center.0, center.1, 0, 150, true);
        let building = game
            .state
            .entities
            .spawn_building(owner, EntityKind::ResourceDepot, 480.0, 480.0, true)
            .unwrap();
        game.state
            .entities
            .get_mut(building)
            .unwrap()
            .set_invulnerable(invulnerable);
        assert_eq!(
            game.state.entities.shielded_damage(
                building,
                if owner == 2 { 1 } else { 2 },
                20,
                20,
                piercing,
                source,
                &teams,
            ),
            expected,
            "owner={owner}, center={center:?}, invulnerable={invulnerable}"
        );
        let drains = expected == 0 || (piercing == 1.0);
        assert_eq!(
            game.state
                .entities
                .get(caster)
                .unwrap()
                .shield
                .as_ref()
                .unwrap()
                .remaining,
            if drains { 130 } else { 150 }
        );
    }
}
