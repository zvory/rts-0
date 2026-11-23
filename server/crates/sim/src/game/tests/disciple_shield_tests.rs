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
fn disciple_shield_expires_at_15_seconds_after_checkpoint_restore() {
    let (mut game, caster) = setup();
    game.enqueue(1, cast(caster, 480.0, false));
    game.tick();
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
                &crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)])
            ),
            20
        );
    }
}

#[test]
fn disciple_shield_replacement_refreshes_expiry() {
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
    assert_eq!(replacement.remaining, 300);
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
fn disciple_shield_snapshots_track_absorption_and_remove_depleted_pool() {
    let (mut game, caster) = setup();
    game.enqueue(1, cast(caster, 480.0, false));
    game.tick();
    let teams = crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    for (damage, remaining) in [(0, 300), (100, 200), (150, 50)] {
        assert_eq!(game.state.entities.shielded_damage(caster, 2, damage, &teams), 0);
        let shield = game.snapshot_full_for(1).ability_objects.into_iter()
            .find(|object| object.ability == "discipleShield").unwrap();
        assert_eq!(shield.owner_state.unwrap().hp, Some(remaining));
    }
    assert_eq!(game.state.entities.shielded_damage(caster, 2, 70, &teams), 20);
    assert!(!game.snapshot_full_for(1).ability_objects.iter()
        .any(|object| object.ability == "discipleShield"));
}
