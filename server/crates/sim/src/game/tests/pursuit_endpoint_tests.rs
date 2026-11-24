use super::fixtures::*;
use super::*;
use crate::game::lab::LabOp;

fn fixture(kind: EntityKind, target_kind: EntityKind, gap: f32) -> (Game, u32, u32) {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let attacker = game
        .state
        .entities
        .spawn_unit(1, kind, 400.0, 800.0)
        .unwrap();
    let target = game
        .state
        .entities
        .spawn_unit(2, target_kind, 400.0 + gap, 800.0)
        .unwrap();
    for player_id in [1, 2] {
        game.apply_lab_op(LabOp::SetPlayerGodMode {
            player_id,
            enabled: true,
        })
        .unwrap();
    }
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
    game.enqueue(
        2,
        Command::Move {
            units: vec![target],
            x: 1800.0,
            y: 800.0,
            queued: false,
        },
    );
    game.tick();
    game.enqueue(
        1,
        Command::Attack {
            units: vec![attacker],
            target,
            queued: false,
        },
    );
    (game, attacker, target)
}

fn attacks(game: &mut Game, attacker: u32) -> usize {
    game.tick()
        .into_iter()
        .flat_map(|(_, events)| events)
        .filter(|event| matches!(event, Event::Attack { from, .. } if *from == attacker))
        .count()
}

#[test]
fn faster_foot_pursuers_reach_and_attack_retreating_targets() {
    for (kind, target_kind, gap) in [
        (EntityKind::WindAdept, EntityKind::ScoutCar, 160.0),
        (EntityKind::Worker, EntityKind::Panzerfaust, 100.0),
        (EntityKind::Golem, EntityKind::Rifleman, 100.0),
        (EntityKind::MagicAdept, EntityKind::Worker, 220.0),
    ] {
        let (mut game, attacker, target) = fixture(kind, target_kind, gap);
        let count: usize = (0..450).map(|_| attacks(&mut game, attacker)).sum();
        assert!(
            count > 0,
            "{kind:?} never attacked retreating {target_kind:?}"
        );
        assert!(
            matches!(
                game.state.entities.get(target).unwrap().order(),
                Order::Move(_)
            ),
            "the target must still be retreating, not stopped at its destination"
        );
    }
}

#[test]
fn tracked_pursuit_handles_stopping_reversing_and_crossing_targets() {
    for next in [None, Some((200.0, 800.0)), Some((900.0, 1400.0))] {
        let (mut game, attacker, target) =
            fixture(EntityKind::WindAdept, EntityKind::ScoutCar, 90.0);
        for _ in 0..90 {
            attacks(&mut game, attacker);
        }
        let command = match next {
            Some((x, y)) => Command::Move {
                units: vec![target],
                x,
                y,
                queued: false,
            },
            None => Command::HoldPosition {
                units: vec![target],
                queued: false,
            },
        };
        game.enqueue(2, command);
        let mut count = 0;
        for _ in 0..240 {
            count += attacks(&mut game, attacker);
        }
        assert!(count > 0, "failed to attack after target change {next:?}");
        if next.is_none() {
            let defender = game.state.entities.get(target).unwrap();
            assert!(
                (defender.pos_x() - (490.0 + 91.0 * 2.35)).abs() < 3.0,
                "stopped target was displaced: {}",
                defender.pos_x()
            );
            let a = game.state.entities.get(attacker).unwrap();
            let a_body = crate::game::services::geometry::unit_body_for_entity(a).unwrap();
            let b_body = crate::game::services::geometry::unit_body_for_entity(defender).unwrap();
            assert!(!crate::game::services::geometry::unit_bodies_intersect(
                a_body, b_body
            ));
        }
    }
}

#[test]
fn master_pursuit_keeps_its_scaled_body_outside_a_stopped_hull() {
    let (mut game, attacker, target) = fixture(EntityKind::WindAdept, EntityKind::ScoutCar, 100.0);
    game.state.entities.get_mut(attacker).unwrap().dao_master = true;
    let mut count = 0;
    for _ in 0..90 {
        count += attacks(&mut game, attacker);
    }
    assert!(count > 0, "master must catch the retreating car");
    game.enqueue(
        2,
        Command::HoldPosition {
            units: vec![target],
            queued: false,
        },
    );
    attacks(&mut game, attacker);
    let stopped = game.state.entities.get(target).unwrap().pos_x();
    for _ in 0..120 {
        attacks(&mut game, attacker);
    }
    let defender = game.state.entities.get(target).unwrap();
    assert_eq!(
        defender.pos_x(),
        stopped,
        "master must not push the stopped car"
    );
    let body = crate::game::services::geometry::unit_body_for_entity(
        game.state.entities.get(attacker).unwrap(),
    )
    .unwrap();
    let hull = crate::game::services::geometry::unit_body_for_entity(defender).unwrap();
    assert!(!crate::game::services::geometry::unit_bodies_intersect(
        body, hull
    ));
}

#[test]
fn grouped_pursuit_attacks_without_pushing_a_stopped_target() {
    let (mut game, attacker, target) = fixture(EntityKind::WindAdept, EntityKind::ScoutCar, 160.0);
    let mut pursuers = vec![attacker];
    for y in [620.0, 680.0, 740.0, 860.0, 920.0, 980.0, 1040.0] {
        let id = game
            .state
            .entities
            .spawn_unit(1, EntityKind::WindAdept, 400.0, y)
            .unwrap();
        pursuers.push(id);
    }
    game.apply_lab_op(LabOp::SetPlayerGodMode {
        player_id: 1,
        enabled: true,
    })
    .unwrap();
    game.rebuild_final_spatial();
    game.enqueue(
        1,
        Command::Attack {
            units: pursuers.clone(),
            target,
            queued: false,
        },
    );
    let mut count = 0;
    for _ in 0..450 {
        count += game.tick().into_iter().flat_map(|(_, es)| es)
            .filter(|e| matches!(e, Event::Attack { from, to, .. } if pursuers.contains(from) && *to == target)).count();
    }
    assert!(count > 0, "grouped pursuers must reach attack range");
    game.enqueue(
        2,
        Command::HoldPosition {
            units: vec![target],
            queued: false,
        },
    );
    game.tick();
    let target_pos = {
        let e = game.state.entities.get(target).unwrap();
        (e.pos_x(), e.pos_y())
    };
    for _ in 0..120 {
        game.tick();
    }
    let defender = game.state.entities.get(target).unwrap();
    assert!(
        (defender.pos_x() - target_pos.0).abs() < 1.0
            && (defender.pos_y() - target_pos.1).abs() < 1.0,
        "crowded pursuers must not push the stopped car"
    );
}
