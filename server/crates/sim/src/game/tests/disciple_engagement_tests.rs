use super::fixtures::*;
use super::*;
use crate::game::entity::{MovePhase, OrderIntent};

fn fixture(kind: EntityKind, distance: f32) -> (Game, u32, u32) {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let attacker = game
        .state
        .entities
        .spawn_unit(1, kind, 500.0, 500.0)
        .unwrap();
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Worker, 500.0 + distance, 500.0)
        .unwrap();
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
    (game, attacker, target)
}

fn attack_move(game: &mut Game, attacker: u32) {
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![attacker],
            x: 500.0,
            y: 800.0,
            queued: false,
        },
    );
}

#[test]
fn disciple_attack_move_approaches_visible_enemy_without_extending_sword_reach() {
    let (mut game, attacker, target) = fixture(EntityKind::Warrior, 128.0);
    attack_move(&mut game, attacker);
    game.tick();
    let unit = game.state.entities.get(attacker).unwrap();
    assert_eq!(unit.target_id(), Some(target));
    assert!(matches!(unit.order(), Order::AttackMove(_)));
    let goal = unit
        .move_intent()
        .expect("attack-move destination survives acquisition");
    assert!((goal.0 - 500.0).abs() <= 16.0 && (goal.1 - 800.0).abs() <= 16.0);
    assert!(!unit.path_is_empty());
    assert_eq!(
        game.state.entities.get(target).unwrap().hp,
        40,
        "must approach before damage"
    );
    for _ in 0..150 {
        game.tick();
    }
    assert!(game
        .state
        .entities
        .get(target)
        .is_none_or(|unit| unit.hp < 40));
    for _ in 0..350 {
        game.tick();
    }
    let unit = game.state.entities.get(attacker).unwrap();
    assert!(
        (unit.pos_y() - goal.1).abs() < 32.0,
        "must resume original route: {}",
        unit.pos_y()
    );
}

#[test]
fn disciple_outside_engagement_range_is_ignored() {
    let (mut game, attacker, target) = fixture(EntityKind::Warrior, 192.0);
    attack_move(&mut game, attacker);
    game.tick();
    assert_eq!(game.state.entities.get(attacker).unwrap().target_id(), None);
    assert_eq!(game.state.entities.get(target).unwrap().hp, 40);
}

#[test]
fn disciple_idle_approaches_but_hold_point_and_move_do_not() {
    for order in [
        Order::Idle,
        Order::HoldPosition,
        Order::Point { facing: 0.0 },
        Order::move_to(500.0, 800.0),
    ] {
        let (mut game, attacker, target) = fixture(EntityKind::Warrior, 128.0);
        game.state
            .entities
            .get_mut(attacker)
            .unwrap()
            .set_order(order.clone());
        game.tick();
        let unit = game.state.entities.get(attacker).unwrap();
        if matches!(order, Order::Idle) {
            assert_eq!(unit.target_id(), Some(target));
            assert!(!unit.path_is_empty());
        } else {
            assert_eq!(unit.target_id(), None, "{order:?}");
        }
    }
}

#[test]
fn disciple_lost_target_cancels_approach_and_resumes_route_during_cooldown() {
    for idle in [false, true] {
        let (mut game, attacker, target) = fixture(EntityKind::Warrior, 128.0);
        if !idle {
            attack_move(&mut game, attacker);
        }
        game.tick();
        game.state
            .entities
            .get_mut(attacker)
            .unwrap()
            .set_weapon_cooldown(combat::WeaponKind::WarriorSword, 50);
        game.state
            .entities
            .get_mut(target)
            .unwrap()
            .set_position(1000.0, 1000.0);
        for _ in 0..20 {
            game.tick();
        }
        let unit = game.state.entities.get(attacker).unwrap();
        assert_eq!(unit.target_id(), None);
        if idle {
            assert!(unit.path_is_empty());
            assert_eq!(unit.path_goal(), None);
        } else {
            assert_eq!(unit.path_goal(), unit.move_intent());
        }
    }
}

#[test]
fn disciple_cooldown_retains_approach_and_acquires_replacement() {
    let (mut game, attacker, target) = fixture(EntityKind::Warrior, 128.0);
    game.state
        .entities
        .get_mut(attacker)
        .unwrap()
        .set_weapon_cooldown(combat::WeaponKind::WarriorSword, 50);
    attack_move(&mut game, attacker);
    game.tick();
    assert_eq!(
        game.state.entities.get(attacker).unwrap().target_id(),
        Some(target)
    );
    let replacement = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Worker, 620.0, 530.0)
        .unwrap();
    game.state.entities.remove(target);
    game.tick();
    assert_eq!(
        game.state.entities.get(attacker).unwrap().target_id(),
        Some(replacement)
    );
    game.tick();
    assert_eq!(
        game.state.entities.get(attacker).unwrap().target_id(),
        Some(replacement)
    );
}

#[test]
fn disciple_engagement_does_not_finish_queued_attack_move_at_approach_goal() {
    let (mut game, attacker, target) = fixture(EntityKind::Warrior, 128.0);
    attack_move(&mut game, attacker);
    game.tick();
    let unit = game.state.entities.get_mut(attacker).unwrap();
    unit.append_queued_order(OrderIntent::move_to(900.0, 900.0));
    unit.clear_path();
    unit.mark_move_phase(MovePhase::Arrived);
    game.tick();
    let unit = game.state.entities.get(attacker).unwrap();
    assert_eq!(unit.target_id(), Some(target));
    assert!(matches!(unit.order(), Order::AttackMove(_)));
    assert_eq!(unit.queued_orders().len(), 1);
}

#[test]
fn disciple_queued_order_advances_at_original_goal_even_with_a_target() {
    let (mut game, attacker, target) = fixture(EntityKind::Warrior, 32.0);
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![attacker],
            x: 500.0,
            y: 500.0,
            queued: false,
        },
    );
    game.tick();
    let unit = game.state.entities.get_mut(attacker).unwrap();
    assert_eq!(unit.target_id(), Some(target));
    assert_eq!(unit.move_phase(), Some(MovePhase::Arrived));
    assert_eq!(unit.path_goal(), unit.move_intent());
    unit.append_queued_order(OrderIntent::move_to(900.0, 900.0));
    game.tick();
    let unit = game.state.entities.get(attacker).unwrap();
    assert!(matches!(unit.order(), Order::Move(_)));
    assert!(unit.queued_orders().is_empty());
}

#[test]
fn engagement_range_defaults_to_current_attack_range_for_every_other_kind() {
    for kind in EntityKind::ALL {
        for range in [0.0, 0.5, 5.0, 14.0, 35.0] {
            let actual = combat::engagement_range_tiles(kind, range);
            assert_eq!(
                actual,
                if kind.is_disciple() {
                    range.max(5.0)
                } else {
                    range
                },
                "{kind:?}"
            );
        }
    }
    for kind in [
        EntityKind::Worker,
        EntityKind::Golem,
        EntityKind::Rifleman,
        EntityKind::MachineGunner,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
        let range = combat::attack_profile(kind).range_tiles * config::TILE_SIZE as f32;
        let (mut game, attacker, _) = fixture(kind, range + 64.0);
        attack_move(&mut game, attacker);
        game.tick();
        assert_eq!(
            game.state.entities.get(attacker).unwrap().target_id(),
            None,
            "{kind:?} must not start acquiring beyond weapon range"
        );
    }
}

#[test]
fn disciple_engagement_respects_fog_and_ownership() {
    for allied in [false, true] {
        let (mut game, attacker, target) = fixture(EntityKind::Warrior, 128.0);
        if allied {
            game.state.entities.remove(target);
            game.state
                .entities
                .spawn_unit(1, EntityKind::Worker, 628.0, 500.0)
                .unwrap();
        } else {
            for y in 0..game.state.map.height {
                let i = game.state.map.index(18, y);
                game.state.map.terrain[i] = crate::protocol::terrain::ROCK;
            }
            game.state
                .fog
                .recompute(&[1, 2], &game.state.entities, &game.state.map);
            assert!(!game.state.fog.is_visible_world(1, 628.0, 500.0));
        }
        attack_move(&mut game, attacker);
        game.tick();
        assert_eq!(game.state.entities.get(attacker).unwrap().target_id(), None);
    }
}

#[test]
fn disciple_explicit_attack_can_pursue_beyond_automatic_engagement_range() {
    let (mut game, attacker, target) = fixture(EntityKind::Warrior, 224.0);
    game.enqueue(
        1,
        Command::Attack {
            units: vec![attacker],
            target,
            queued: false,
        },
    );
    game.tick();
    let unit = game.state.entities.get(attacker).unwrap();
    assert_eq!(unit.target_id(), Some(target));
    assert!(!unit.path_is_empty());
    assert_eq!(unit.order().attack_target(), Some(target));
}

#[test]
fn disciple_queued_order_waits_until_original_goal_after_target_dies() {
    let (mut game, attacker, target) = fixture(EntityKind::Warrior, 64.0);
    attack_move(&mut game, attacker);
    game.tick();
    game.state
        .entities
        .get_mut(attacker)
        .unwrap()
        .append_queued_order(OrderIntent::move_to(900.0, 900.0));
    let mut resumed = false;
    for _ in 0..240 {
        game.tick();
        let unit = game.state.entities.get(attacker).unwrap();
        if game.state.entities.get(target).is_none() {
            resumed = true;
            if unit.pos_y() < 760.0 {
                assert!(
                    matches!(unit.order(), Order::AttackMove(_)),
                    "must reach original goal before queue promotion"
                );
                assert_eq!(unit.queued_orders().len(), 1);
            }
        }
    }
    assert!(resumed, "must kill target and resume");
    for _ in 0..300 {
        game.tick();
    }
    let unit = game.state.entities.get(attacker).unwrap();
    assert!(unit.queued_orders().is_empty());
    assert!(unit.pos_x() > 800.0 && unit.pos_y() > 800.0);
}

#[test]
fn war_spear_approaches_then_stops_at_ready_tip_reach() {
    // Start outside spear reach but inside automatic engagement. Ordinary infantry
    // retains center targeting; the smaller ready tip is about 66.5 px from the crown.
    let (mut game, attacker, target) = fixture(EntityKind::Warrior, 160.0);
    game.state
        .entities
        .get_mut(attacker)
        .unwrap()
        .set_disciple_kind_for_test(EntityKind::WarAdept);
    game.state
        .entities
        .get_mut(target)
        .unwrap()
        .set_order(Order::HoldPosition);
    let mut struck = false;
    for _ in 0..150 {
        game.tick();
        let victim = game.state.entities.get(target).unwrap();
        if victim.hp < 40 {
            let spear = game.state.entities.get(attacker).unwrap();
            let distance =
                (victim.pos_x() - spear.pos_x()).det_hypot(victim.pos_y() - spear.pos_y());
            assert!(
                (66.0..=70.0).contains(&distance),
                "spear stand-off: {distance}"
            );
            let stopped = (spear.pos_x(), spear.pos_y());
            for _ in 0..10 {
                game.tick();
            }
            let spear = game.state.entities.get(attacker).unwrap();
            assert!((spear.pos_x() - stopped.0).abs() < 0.01);
            assert!((spear.pos_y() - stopped.1).abs() < 0.01);
            struck = true;
            break;
        }
    }
    assert!(struck, "War must approach and strike from spear reach");
}
