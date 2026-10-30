use super::checkpoint_helpers::restore_checkpoint_and_assert_equivalent;
use super::fixtures::{empty_flat_game, human_vs_ai_players};
use super::*;
use crate::game::services::occupancy::footprint_center;
use crate::game::upgrade::UpgradeKind;

fn fixture() -> (Game, Vec<u32>) {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let mut depots = Vec::new();
    for (owner, tile) in [(1, (5, 5)), (1, (12, 5)), (1, (19, 5)), (2, (50, 50))] {
        let (x, y) = footprint_center(&game.state.map, EntityKind::ResourceDepot, tile.0, tile.1);
        depots.push(
            game.state
                .entities
                .spawn_building(owner, EntityKind::ResourceDepot, x, y, true)
                .unwrap(),
        );
    }
    game.state.players[0].set_resources(10_000, 10_000);
    (game, depots)
}

fn upgrade(game: &mut Game, depot: u32) {
    game.enqueue(
        1,
        Command::Research {
            building: depot,
            upgrade: UpgradeKind::IndustrialCenter,
        },
    );
    game.tick();
}

#[test]
fn industrial_center_instant_cost_limit_loss_and_replacement() {
    let (mut game, depots) = fixture();
    upgrade(&mut game, depots[0]);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (9_200, 9_700)
    );
    let center = game.state.entities.get(depots[0]).unwrap();
    assert!(center.is_industrial_center());
    assert!(center.research_queue().is_empty());
    assert_eq!((center.hp, center.max_hp), (540, 540));
    assert_eq!(game.snapshot_for(1).supply_cap, 200);
    upgrade(&mut game, depots[0]);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (9_200, 9_700)
    );
    upgrade(&mut game, depots[1]);
    upgrade(&mut game, depots[2]);
    assert!(!game
        .state
        .entities
        .get(depots[2])
        .unwrap()
        .is_industrial_center());
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (8_400, 9_400)
    );
    assert_eq!(game.snapshot_for(1).supply_cap, 300);
    assert!(!game.state.players[0]
        .upgrades
        .contains(&UpgradeKind::IndustrialCenter));
    game.state
        .entities
        .get_mut(depots[0])
        .unwrap()
        .apply_damage(u32::MAX, None);
    assert_eq!(game.snapshot_for(1).supply_cap, 200);
    upgrade(&mut game, depots[2]);
    assert!(game
        .state
        .entities
        .get(depots[2])
        .unwrap()
        .is_industrial_center());
    restore_checkpoint_and_assert_equivalent(&game, "instant industrial centers");
}

#[test]
fn industrial_center_debt_is_projected_saved_and_repaid_by_income() {
    let (mut game, depots) = fixture();
    game.state.players[0].set_resources(0, 0);
    upgrade(&mut game, depots[0]);
    assert_eq!(
        (game.snapshot_for(1).steel, game.snapshot_for(1).oil),
        (-800, -300)
    );
    assert_eq!(game.snapshot_for(1).supply_cap, 200);
    let observer = game.snapshot_for_spectator(&[1, 2]);
    let row = observer
        .player_resources
        .iter()
        .find(|p| p.id == 1)
        .unwrap();
    assert_eq!((row.steel, row.oil), (-800, -300));
    restore_checkpoint_and_assert_equivalent(&game, "industrial center debt");
    // Cancellation cannot refund an instantaneous, completed upgrade.
    game.enqueue(
        1,
        Command::Cancel {
            building: depots[0],
            construction: false,
        },
    );
    game.tick();
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (-800, -300)
    );
    let tick = game.tick_count();
    let player = &mut game.state.players[0];
    player.add_gathered_resources(EntityKind::Steel, 900, tick);
    assert_eq!((player.steel, player.oil), (100, -300));
    assert!(
        !player.can_afford(0, 0),
        "even free jobs wait while either balance is negative"
    );
    player.add_gathered_resources(EntityKind::Oil, 300, tick);
    assert!(player.can_afford(100, 0));
    assert!(player.spend_resources(100, 0));
    assert_eq!((player.steel, player.oil), (0, 0));
    assert!(player.can_afford(0, 0));
    // A second eligible depot may incur additional debt as well.
    upgrade(&mut game, depots[1]);
    assert_eq!(
        (game.snapshot_for(1).steel, game.snapshot_for(1).oil),
        (-800, -300)
    );
    assert_eq!(game.snapshot_for(1).supply_cap, 300);
}

#[test]
fn industrial_center_rejects_enemy_unfinished_and_other_faction_depots() {
    let (mut game, depots) = fixture();
    upgrade(&mut game, depots[3]);
    assert!(!game
        .state
        .entities
        .get(depots[3])
        .unwrap()
        .is_industrial_center());
    let unfinished = game
        .state
        .entities
        .spawn_building(1, EntityKind::ResourceDepot, 800.0, 800.0, false)
        .unwrap();
    upgrade(&mut game, unfinished);
    assert!(!game
        .state
        .entities
        .get(unfinished)
        .unwrap()
        .is_industrial_center());
    for faction in ["ekat", "cultivators"] {
        game.state.players[0].faction_id = faction.into();
        upgrade(&mut game, depots[0]);
        assert!(!game
            .state
            .entities
            .get(depots[0])
            .unwrap()
            .is_industrial_center());
        assert_eq!(game.snapshot_for(1).supply_cap, 300);
    }
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (10_000, 10_000)
    );
}

#[test]
fn industrial_center_debt_keeps_paid_training_running_and_unpaid_training_waiting() {
    let (mut game, depots) = fixture();
    game.enqueue(
        1,
        Command::Train {
            building: depots[0],
            unit: EntityKind::Worker,
        },
    );
    game.tick();
    let progress = game.state.entities.get(depots[0]).unwrap().prod_queue()[0].progress;
    game.state.players[0].set_resources(0, 0);
    upgrade(&mut game, depots[0]);
    let depot = game.state.entities.get(depots[0]).unwrap();
    assert!(depot.is_industrial_center());
    assert_eq!(depot.prod_queue()[0].progress, progress + 1);
    game.enqueue(
        1,
        Command::Train {
            building: depots[1],
            unit: EntityKind::Worker,
        },
    );
    game.tick();
    let waiting = &game.state.entities.get(depots[1]).unwrap().prod_queue()[0];
    assert!(!waiting.paid);
    assert_eq!(waiting.progress, 0);
    let cost = crate::rules::economy::resource_cost(EntityKind::Worker);
    let tick = game.tick_count();
    game.state.players[0].add_gathered_resources(EntityKind::Steel, 800 + cost.steel, tick);
    game.tick();
    assert!(
        !game.state.entities.get(depots[1]).unwrap().prod_queue()[0].paid,
        "oil debt blocks a steel-only unit"
    );
    game.state.players[0].add_gathered_resources(EntityKind::Oil, 300 + cost.oil, tick);
    game.tick();
    let started = &game.state.entities.get(depots[1]).unwrap().prod_queue()[0];
    assert!(started.paid);
    assert_eq!(started.progress, 1);
}

#[test]
fn critical_events_report_immediate_industrial_centers() {
    let (mut game, depots) = fixture();
    assert!(game.critical_event_state()[0].upgrading_depots.is_empty());
    upgrade(&mut game, depots[0]);
    let facts = game.critical_event_state();
    assert_eq!(facts[0].upgrading_depots, vec![depots[0]]);
    assert_eq!(facts[0].supply_cap, 200);
    assert!(facts[1].upgrading_depots.is_empty());
}

#[test]
fn industrial_center_hp_bonus_preserves_damage_and_applies_once() {
    let (mut game, depots) = fixture();
    game.state
        .entities
        .get_mut(depots[0])
        .unwrap()
        .apply_damage(40, None);
    upgrade(&mut game, depots[0]);
    let depot = game.state.entities.get(depots[0]).unwrap();
    assert_eq!((depot.hp, depot.max_hp), (500, 540));
    let ordinary = game.state.entities.get(depots[1]).unwrap();
    assert_eq!((ordinary.hp, ordinary.max_hp), (180, 180));
    upgrade(&mut game, depots[0]);
    let depot = game.state.entities.get(depots[0]).unwrap();
    assert_eq!((depot.hp, depot.max_hp), (500, 540));
    restore_checkpoint_and_assert_equivalent(&game, "damaged industrial center HP");
}
