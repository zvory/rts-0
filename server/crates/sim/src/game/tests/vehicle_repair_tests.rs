use super::checkpoint_helpers::{restore_checkpoint_and_assert_equivalent, tick_pair_for};
use super::fixtures::*;
use super::*;

fn fixture(kind: EntityKind) -> (Game, u32, u32) {
    let mut game = empty_flat_game(&[PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".into(),
        name: "Mechanic".into(),
        color: "#fff".into(),
        is_ai: false,
    }]);
    game.state.players[0].set_resources(10_000, 10_000);
    game.state
        .entities
        .spawn_building(1, EntityKind::ResourceDepot, 320.0, 320.0, true)
        .unwrap();
    let worker = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, 560.0, 600.0)
        .unwrap();
    let target = game
        .state
        .entities
        .spawn_unit(1, kind, 600.0, 600.0)
        .unwrap();
    let t = game.state.entities.get_mut(target).unwrap();
    t.apply_damage(20, None);
    t.set_order(Order::HoldPosition);
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1], &game.state.entities, &game.state.map);
    (game, worker, target)
}

fn issue(game: &mut Game, worker: u32, target: u32, queued: bool) {
    game.enqueue(
        1,
        Command::Repair {
            units: vec![worker],
            target,
            queued,
        },
    );
}

#[test]
fn vehicle_repair_all_four_targets_share_rate_price_and_queue_completion() {
    for kind in crate::rules::target::REPAIRABLE_UNIT_KINDS {
        let (mut game, worker, target) = fixture(kind);
        let before = game.state.entities.get(target).unwrap().hp;
        issue(&mut game, worker, target, true);
        game.enqueue(
            1,
            Command::Move {
                units: vec![worker],
                x: 440.0,
                y: 600.0,
                queued: true,
            },
        );
        for _ in 0..30 {
            game.tick();
        }
        assert_eq!(
            game.state.entities.get(target).unwrap().hp,
            before + 2,
            "{kind:?}"
        );
        for _ in 0..271 {
            game.tick();
        }
        let t = game.state.entities.get(target).unwrap();
        assert_eq!(t.hp, t.max_hp, "{kind:?}");
        let price = crate::rules::economy::resource_cost(kind);
        let charge = |amount: u32| (20 * amount).div_ceil(4 * t.max_hp);
        assert_eq!(
            (game.state.players[0].steel, game.state.players[0].oil),
            (10_000 - charge(price.steel), 10_000 - charge(price.oil)),
            "{kind:?}"
        );
        assert!(!matches!(
            game.state.entities.get(worker).unwrap().order(),
            Order::Repair(_)
        ));
    }
}

#[test]
fn vehicle_repair_follows_moving_target_and_preserves_billing_through_checkpoint() {
    let (mut game, worker, target) = fixture(EntityKind::Tank);
    issue(&mut game, worker, target, false);
    for _ in 0..20 {
        game.tick();
    }
    let hp = game.state.entities.get(target).unwrap().hp;
    game.enqueue(
        1,
        Command::Move {
            units: vec![target],
            x: 950.0,
            y: 600.0,
            queued: false,
        },
    );
    let start_worker_x = game.state.entities.get(worker).unwrap().pos_x();
    for _ in 0..180 {
        game.tick();
    }
    assert!(game.state.entities.get(worker).unwrap().pos_x() > start_worker_x + 100.0);
    let t = game.state.entities.get(target).unwrap();
    assert!((t.pos_x() - 950.0).abs() < 40.0);
    assert!(
        t.hp < t.max_hp,
        "escaping the worker must pause repair progress"
    );
    for _ in 0..180 {
        game.tick();
    }
    assert!(
        game.state.entities.get(target).unwrap().hp > hp,
        "repairs resume after catching up"
    );
    let mut restored = restore_checkpoint_and_assert_equivalent(&game, "mobile repair");
    tick_pair_for(&mut game, &mut restored, 30, "mobile repair checkpoint");
}

#[test]
fn vehicle_repair_rejects_all_unlisted_mobile_kinds() {
    for kind in EntityKind::ALL
        .into_iter()
        .filter(|k| k.is_unit() && !crate::rules::target::is_repairable_unit(*k))
    {
        let Some(mut t) = Entity::new_unit(1, kind, 600.0, 600.0) else {
            continue;
        };
        t.apply_damage(1, None);
        assert!(!t.can_be_repaired_by(1), "{kind:?}");
    }
    for kind in crate::rules::target::REPAIRABLE_UNIT_KINDS {
        let mut t = Entity::new_unit(2, kind, 600.0, 600.0).unwrap();
        t.apply_damage(10, None);
        assert!(!t.can_be_repaired_by(1));
    }
}

#[test]
fn vehicle_repair_stacks_workers_and_waits_for_funds() {
    let (mut game, worker, target) = fixture(EntityKind::Tank);
    let second = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, 600.0, 560.0)
        .unwrap();
    let hp = game.state.entities.get(target).unwrap().hp;
    game.state.players[0].set_resources(0, 0);
    game.enqueue(
        1,
        Command::Repair {
            units: vec![worker, second],
            target,
            queued: false,
        },
    );
    for _ in 0..30 {
        game.tick();
    }
    assert_eq!(game.state.entities.get(target).unwrap().hp, hp);
    game.state.players[0].set_resources(100, 100);
    game.tick();
    assert_eq!(game.state.entities.get(target).unwrap().hp, hp + 2);
    for _ in 0..29 {
        game.tick();
    }
    assert_eq!(game.state.entities.get(target).unwrap().hp, hp + 4);
    let mut restored =
        restore_checkpoint_and_assert_equivalent(&game, "active vehicle repair billing");
    tick_pair_for(
        &mut game,
        &mut restored,
        20,
        "active vehicle repair billing",
    );
}
