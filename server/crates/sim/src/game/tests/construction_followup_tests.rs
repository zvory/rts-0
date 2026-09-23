use super::fixtures::*;
use super::*;
use crate::game::entity::BuildPhase;

fn fixture() -> (Game, u32, u32) {
    let mut game = empty_flat_game(&[PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".into(),
        name: "Builder".into(),
        color: "#fff".into(),
        is_ai: false,
    }]);
    game.state.players[0].set_resources(10_000, 10_000);
    let (x, y) =
        services::occupancy::footprint_center(&game.state.map, EntityKind::ResourceDepot, 3, 3);
    game.state
        .entities
        .spawn_building(1, EntityKind::ResourceDepot, x, y, true)
        .unwrap();
    let (x, y) = game.state.map.tile_center(9, 10);
    let worker = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, x, y)
        .unwrap();
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1], &game.state.entities, &game.state.map);
    game.enqueue(
        1,
        Command::Build {
            units: vec![worker],
            building: EntityKind::Barracks,
            tile_x: 10,
            tile_y: 10,
            queued: false,
        },
    );
    for _ in 0..120 {
        game.tick();
        if let Some(BuildPhase::Constructing { site }) =
            game.state.entities.get(worker).unwrap().build_phase()
        {
            return (game, worker, site);
        }
    }
    panic!("worker must start the barracks");
}

fn finish(game: &mut Game, site: u32) {
    let building = game.state.entities.get_mut(site).unwrap();
    let total = building.construction.as_ref().unwrap().total;
    building.set_construction_progress(total - 1);
    game.tick();
    assert!(!game.state.entities.get(site).unwrap().under_construction());
    game.tick();
}

fn move_command(worker: u32, x: f32, queued: bool) -> Command {
    Command::Move {
        units: vec![worker],
        x,
        y: 320.0,
        queued,
    }
}

#[test]
fn construction_followup_latest_click_replaces_queue_and_walks_after_completion() {
    let (mut game, worker, site) = fixture();
    let entity = game.state.entities.get(worker).unwrap();
    let before = (entity.pos_x, entity.pos_y);
    game.enqueue(1, move_command(worker, 640.0, false));
    game.enqueue(1, move_command(worker, 704.0, true));
    game.enqueue(1, move_command(worker, 768.0, false));
    game.tick();
    let entity = game.state.entities.get(worker).unwrap();
    assert_eq!((entity.pos_x, entity.pos_y), before);
    assert_eq!(
        entity.build_phase(),
        Some(BuildPhase::Constructing { site })
    );
    assert_eq!(
        entity.queued_orders(),
        &[OrderIntent::move_to(768.0, 320.0)]
    );
    let view = game
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|e| e.id == worker)
        .unwrap();
    assert_eq!(
        view.order_plan
            .iter()
            .map(|m| m.kind.as_str())
            .collect::<Vec<_>>(),
        ["build", "move"]
    );
    assert_eq!(view.order_plan[1].x, 768.0);
    finish(&mut game, site);
    assert!(matches!(
        game.state.entities.get(worker).unwrap().order(),
        Order::Move(_)
    ));
    for _ in 0..600 {
        game.tick();
    }
    let after = game.state.entities.get(worker).unwrap();
    assert!((after.pos_x - 768.0).abs() < 32.0 && (after.pos_y - 320.0).abs() < 32.0);
}

#[test]
fn construction_followup_training_centre_starts_after_barracks() {
    let (mut game, worker, site) = fixture();
    game.enqueue(
        1,
        Command::Build {
            units: vec![worker],
            building: EntityKind::TrainingCentre,
            tile_x: 16,
            tile_y: 10,
            queued: false,
        },
    );
    game.tick();
    assert_eq!(
        game.state.entities.get(worker).unwrap().queued_orders(),
        &[OrderIntent::build(EntityKind::TrainingCentre, 16, 10)]
    );
    finish(&mut game, site);
    for _ in 0..600 {
        game.tick();
        if game
            .state
            .entities
            .iter()
            .any(|e| e.kind == EntityKind::TrainingCentre)
        {
            return;
        }
    }
    panic!("follow-up training centre should start without Shift");
}

#[test]
fn construction_followup_hold_replaces_full_queue_and_shift_still_appends() {
    let (mut game, worker, site) = fixture();
    for index in 0..8 {
        game.enqueue(1, move_command(worker, 640.0 + index as f32 * 32.0, true));
    }
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(worker)
            .unwrap()
            .queued_orders()
            .len(),
        8
    );
    game.enqueue(
        1,
        Command::HoldPosition {
            units: vec![worker],
            queued: false,
        },
    );
    game.tick();
    assert_eq!(
        game.state.entities.get(worker).unwrap().queued_orders(),
        &[OrderIntent::hold_position()]
    );
    game.enqueue(1, move_command(worker, 640.0, false));
    game.enqueue(1, move_command(worker, 704.0, true));
    game.tick();
    assert_eq!(
        game.state.entities.get(worker).unwrap().queued_orders(),
        &[
            OrderIntent::move_to(640.0, 320.0),
            OrderIntent::move_to(704.0, 320.0)
        ]
    );
    assert_eq!(
        game.state.entities.get(worker).unwrap().build_phase(),
        Some(BuildPhase::Constructing { site })
    );
    game.enqueue(
        1,
        Command::Stop {
            units: vec![worker],
        },
    );
    game.tick();
    assert!(matches!(
        game.state.entities.get(worker).unwrap().order(),
        Order::Idle
    ));
    assert!(game
        .state
        .entities
        .get(worker)
        .unwrap()
        .queued_orders()
        .is_empty());
    assert!(game.state.entities.get(site).unwrap().under_construction());
}

#[test]
fn construction_followup_formation_moves_other_units_immediately() {
    let (mut game, worker, site) = fixture();
    let soldier = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, 200.0, 320.0)
        .unwrap();
    game.rebuild_final_spatial();
    game.enqueue(
        1,
        Command::FormationMove {
            units: vec![worker, soldier],
            points: vec![(640.0, 320.0), (704.0, 320.0)],
            attack_move: false,
            queued: false,
        },
    );
    game.tick();
    assert_eq!(
        game.state.entities.get(worker).unwrap().build_phase(),
        Some(BuildPhase::Constructing { site })
    );
    assert_eq!(
        game.state
            .entities
            .get(worker)
            .unwrap()
            .queued_orders()
            .len(),
        1
    );
    assert!(matches!(
        game.state.entities.get(soldier).unwrap().order(),
        Order::Move(_)
    ));
    finish(&mut game, site);
    assert!(matches!(
        game.state.entities.get(worker).unwrap().order(),
        Order::Move(_)
    ));
}

#[test]
fn construction_followup_other_worker_orders_preserve_build_and_promote() {
    for kind in ["attackMove", "attack", "hold", "deconstruct"] {
        let (mut game, worker, site) = fixture();
        let target = match kind {
            "deconstruct" => game
                .state
                .entities
                .spawn_building(1, EntityKind::TankTrap, 416.0, 240.0, true)
                .unwrap(),
            _ => game
                .state
                .entities
                .spawn_unit(1, EntityKind::Rifleman, 640.0, 320.0)
                .unwrap(),
        };
        game.state
            .entities
            .spawn_unit(1, EntityKind::Rifleman, 448.0, 240.0)
            .unwrap();
        game.rebuild_final_spatial();
        game.state
            .fog
            .recompute(&[1], &game.state.entities, &game.state.map);
        let command = match kind {
            "attackMove" => Command::AttackMove {
                units: vec![worker],
                x: 640.0,
                y: 320.0,
                queued: false,
            },
            "attack" => Command::Attack {
                units: vec![worker],
                target,
                queued: false,
            },
            "hold" => Command::HoldPosition {
                units: vec![worker],
                queued: false,
            },
            _ => Command::Deconstruct {
                units: vec![worker],
                target,
                queued: false,
            },
        };
        game.enqueue(1, command);
        game.tick();
        let entity = game.state.entities.get(worker).unwrap();
        assert_eq!(
            entity.build_phase(),
            Some(BuildPhase::Constructing { site }),
            "{kind}"
        );
        assert_eq!(entity.queued_orders().len(), 1, "{kind}");
        finish(&mut game, site);
        let order = game.state.entities.get(worker).unwrap().order();
        assert!(
            match kind {
                "attackMove" => matches!(order, Order::AttackMove(_)),
                "attack" => matches!(order, Order::Attack(_)),
                "hold" => matches!(order, Order::HoldPosition),
                _ => matches!(order, Order::Deconstruct(_)),
            },
            "{kind}: {order:?}"
        );
    }
}
