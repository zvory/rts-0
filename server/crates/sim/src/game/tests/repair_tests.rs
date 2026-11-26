use super::checkpoint_helpers::{restore_checkpoint_and_assert_equivalent, tick_pair_for};
use super::fixtures::*;
use super::*;

fn fixture(faction: &str) -> (Game, u32, u32) {
    let mut game = empty_flat_game(&[PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: faction.into(),
        name: "Repairer".into(),
        color: "#fff".into(),
        is_ai: false,
    }]);
    game.state.players[0].set_resources(10_000, 10_000);
    let (x, y) =
        services::occupancy::footprint_center(&game.state.map, EntityKind::ResourceDepot, 8, 8);
    let building = game
        .state
        .entities
        .spawn_building(1, EntityKind::ResourceDepot, x, y, true)
        .unwrap();
    game.state.entities.get_mut(building).unwrap().hp -= 100;
    let worker = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, x - 96.0, y)
        .unwrap();
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1], &game.state.entities, &game.state.map);
    (game, worker, building)
}

fn repair(worker: u32, target: u32, queued: bool) -> Command {
    Command::Repair {
        units: vec![worker],
        target,
        queued,
    }
}

#[test]
fn repair_both_factions_restore_two_hp_per_second_and_resume_checkpoint() {
    for faction in ["kriegsia", "cultivators"] {
        let (mut game, worker, building) = fixture(faction);
        let before = game.state.entities.get(building).unwrap().hp;
        game.enqueue(1, repair(worker, building, false));
        for _ in 0..30 {
            game.tick();
        }
        assert_eq!(
            game.state.entities.get(building).unwrap().hp,
            before + 2,
            "{faction}"
        );
        let marker = &game
            .snapshot_for(1)
            .entities
            .into_iter()
            .find(|e| e.id == worker)
            .unwrap()
            .order_plan[0];
        assert_eq!(marker.kind, "repair");
        for _ in 0..7 {
            game.tick();
        }
        let mut restored =
            restore_checkpoint_and_assert_equivalent(&game, "repair midway through HP");
        tick_pair_for(&mut game, &mut restored, 30, "repair checkpoint");
    }
}

#[test]
fn repair_multiple_workers_stack_and_queue_finishes_at_full_health() {
    let (mut game, worker, building) = fixture("kriegsia");
    let second = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, 240.0, 350.0)
        .unwrap();
    game.state.entities.get_mut(building).unwrap().hp =
        game.state.entities.get(building).unwrap().max_hp - 4;
    game.enqueue(
        1,
        Command::Repair {
            units: vec![worker, second],
            target: building,
            queued: false,
        },
    );
    game.enqueue(
        1,
        Command::Move {
            units: vec![worker],
            x: 200.0,
            y: 350.0,
            queued: true,
        },
    );
    for _ in 0..30 {
        game.tick();
    }
    let b = game.state.entities.get(building).unwrap();
    assert_eq!(b.hp, b.max_hp);
    game.tick();
    assert!(matches!(
        game.state.entities.get(worker).unwrap().order(),
        Order::Move(_)
    ));
    assert!(matches!(
        game.state.entities.get(second).unwrap().order(),
        Order::Idle
    ));
}

#[test]
fn repair_walks_to_building_and_skips_invalid_queued_targets() {
    let (mut game, worker, building) = fixture("kriegsia");
    game.state
        .entities
        .get_mut(worker)
        .unwrap()
        .set_position(100.0, 100.0);
    game.rebuild_final_spatial();
    let before = game.state.entities.get(building).unwrap().hp;
    game.enqueue(1, repair(worker, building, true));
    for _ in 0..600 {
        game.tick();
    }
    assert!(game.state.entities.get(building).unwrap().hp > before);
    game.state.entities.remove(building);
    game.tick();
    assert!(!matches!(
        game.state.entities.get(worker).unwrap().order(),
        Order::Repair(_)
    ));
}

#[test]
fn repair_waits_for_resources_without_losing_order_or_charging_failed_work() {
    let (mut game, worker, building) = fixture("kriegsia");
    game.state.players[0].set_resources(0, 0);
    let before = game.state.entities.get(building).unwrap().hp;
    game.enqueue(1, repair(worker, building, false));
    for _ in 0..60 {
        game.tick();
    }
    assert_eq!(game.state.entities.get(building).unwrap().hp, before);
    assert!(matches!(
        game.state.entities.get(worker).unwrap().order(),
        Order::Repair(_)
    ));
    assert_eq!(
        game.state.entities.get(building).unwrap().repair_credit,
        None
    );
    game.state.players[0].set_resources(10, 10);
    game.tick();
    assert_eq!(game.state.entities.get(building).unwrap().hp, before + 1);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (9, 9)
    );
}

#[test]
fn repair_rejects_enemy_scaffolds_units_and_full_health_and_can_be_interrupted() {
    for invalid in ["enemy", "scaffold", "unit", "full", "dead"] {
        let (mut game, worker, building) = fixture("kriegsia");
        let target = if invalid == "unit" { worker } else { building };
        let b = game.state.entities.get_mut(building).unwrap();
        match invalid {
            "enemy" => b.owner = 2,
            "scaffold" => {
                // Spawn a real scaffold rather than manufacturing inconsistent entity groups.
            }
            "full" => b.hp = b.max_hp,
            "dead" => b.hp = 0,
            _ => {}
        }
        let target = if invalid == "scaffold" {
            game.state
                .entities
                .spawn_building(1, EntityKind::Barracks, 600.0, 600.0, false)
                .unwrap()
        } else {
            target
        };
        game.enqueue(1, repair(worker, target, false));
        game.tick();
        assert!(
            !matches!(
                game.state.entities.get(worker).unwrap().order(),
                Order::Repair(_)
            ),
            "{invalid}"
        );
    }
    let (mut game, worker, building) = fixture("kriegsia");
    game.enqueue(1, repair(worker, building, false));
    game.tick();
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
}

#[test]
fn repair_rejects_consumable_healing_capacity_without_spending_resources() {
    let (mut game, worker, _) = fixture("cultivators");
    let seal = game
        .state
        .entities
        .spawn_building(1, EntityKind::HealingSeal, 300.0, 300.0, true)
        .unwrap();
    // Seals spend HP as healing capacity; it must never be refilled by paid repairs.
    let target = game.state.entities.get_mut(seal).unwrap();
    target.hp -= 100;
    let before = target.hp;
    for queued in [false, true] {
        game.enqueue(1, repair(worker, seal, queued));
        game.tick();
        let w = game.state.entities.get(worker).unwrap();
        assert!(!matches!(w.order(), Order::Repair(_)));
        assert!(w.queued_orders().is_empty());
    }
    // Tick validation must also terminate an invalid active order from restored state.
    game.state
        .entities
        .get_mut(worker)
        .unwrap()
        .set_order(Order::repair(seal));
    for _ in 0..30 {
        game.tick();
    }
    assert!(matches!(
        game.state.entities.get(worker).unwrap().order(),
        Order::Idle
    ));
    assert_eq!(game.state.entities.get(seal).unwrap().hp, before);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (10_000, 10_000)
    );
}
