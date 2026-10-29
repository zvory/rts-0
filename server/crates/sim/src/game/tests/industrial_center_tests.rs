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
fn industrial_center_cost_timing_limit_loss_and_replacement() {
    let (mut game, depots) = fixture();
    assert_eq!(game.snapshot_for(1).supply_cap, 100);
    upgrade(&mut game, depots[0]);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (9_200, 9_700)
    );
    assert_eq!(
        game.state.entities.get(depots[0]).unwrap().research_queue()[0].total,
        1_800
    );
    upgrade(&mut game, depots[0]); // Same depot cannot reserve a second center.
    assert_eq!(
        game.state
            .entities
            .get(depots[0])
            .unwrap()
            .research_queue()
            .len(),
        1
    );
    upgrade(&mut game, depots[1]);
    upgrade(&mut game, depots[2]); // Two pending centers reserve both slots.
    assert!(game
        .state
        .entities
        .get(depots[2])
        .unwrap()
        .research_queue()
        .is_empty());
    assert_eq!(game.snapshot_for(1).supply_cap, 100);
    let mut restored =
        restore_checkpoint_and_assert_equivalent(&game, "pending industrial centers");
    while game.tick_count() < 1_799 {
        game.tick();
        restored.tick();
    }
    assert_eq!(game.snapshot_for(1).supply_cap, 100);
    game.tick();
    restored.tick();
    assert_eq!(game.snapshot_for(1).supply_cap, 200);
    assert_eq!(restored.snapshot_for(1).supply_cap, 200);
    assert!(!game.state.players[0]
        .upgrades
        .contains(&UpgradeKind::IndustrialCenter));
    let center = game
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|e| e.id == depots[0])
        .unwrap();
    assert!(center.industrial_center);
    assert_eq!((center.hp, center.max_hp), (540, 540));
    assert_eq!(center.kind, "resource_depot");
    game.tick();
    game.tick();
    assert_eq!(game.snapshot_for(1).supply_cap, 300);
    game.state
        .entities
        .get_mut(depots[0])
        .unwrap()
        .apply_damage(u32::MAX, None);
    // Capacity reacts to death before cleanup and before the next production reservation.
    assert_eq!(game.snapshot_for(1).supply_cap, 200);
    upgrade(&mut game, depots[2]);
    assert_eq!(
        game.state
            .entities
            .get(depots[2])
            .unwrap()
            .research_queue()
            .len(),
        1
    );
    restore_checkpoint_and_assert_equivalent(&game, "completed and replacement industrial centers");
}

#[test]
fn industrial_center_unpaid_cancel_and_invalid_ownership() {
    let (mut game, depots) = fixture();
    game.state.players[0].set_resources(0, 0);
    upgrade(&mut game, depots[0]);
    assert!(!game.state.entities.get(depots[0]).unwrap().research_queue()[0].paid);
    assert!(game.critical_event_state()[0].upgrading_depots.is_empty());
    game.state.players[0].set_resources(800, 300);
    game.tick();
    assert!(game.state.entities.get(depots[0]).unwrap().research_queue()[0].paid);
    assert_eq!(
        game.critical_event_state()[0].upgrading_depots,
        vec![depots[0]]
    );
    assert_eq!(game.snapshot_for(1).supply_cap, 100);
    game.enqueue(
        1,
        Command::Cancel {
            building: depots[0],
            construction: false,
        },
    );
    game.tick();
    assert!(game
        .state
        .entities
        .get(depots[0])
        .unwrap()
        .research_queue()
        .is_empty());
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (800, 300)
    );
    upgrade(&mut game, depots[3]);
    assert!(game
        .state
        .entities
        .get(depots[3])
        .unwrap()
        .research_queue()
        .is_empty());
    let unfinished = game
        .state
        .entities
        .spawn_building(1, EntityKind::ResourceDepot, 800.0, 800.0, false)
        .unwrap();
    upgrade(&mut game, unfinished);
    assert!(game
        .state
        .entities
        .get(unfinished)
        .unwrap()
        .research_queue()
        .is_empty());
}

#[test]
fn industrial_center_over_cap_blocks_only_new_supply_and_other_factions_stay_unchanged() {
    let (mut game, _) = fixture();
    game.state.players[0].set_supply_used(110);
    assert!(!game.state.players[0].can_reserve_supply(1, &game.state.entities));
    assert!(game.state.players[0].can_reserve_supply(0, &game.state.entities));
    for faction in ["ekat", "cultivators"] {
        game.state.players[0].faction_id = faction.into();
        assert_eq!(game.snapshot_for(1).supply_cap, 300);
    }
}

#[test]
fn industrial_center_completion_does_not_pause_worker_training() {
    let (mut game, depots) = fixture();
    upgrade(&mut game, depots[0]);
    let depot = game.state.entities.get_mut(depots[0]).unwrap();
    let research = depot.research_queue_mut().unwrap().first_mut().unwrap();
    research.progress = research.total - 1;
    game.enqueue(
        1,
        Command::Train {
            building: depots[0],
            unit: EntityKind::Worker,
        },
    );
    game.tick();
    let depot = game.state.entities.get(depots[0]).unwrap();
    assert!(depot.is_industrial_center());
    assert_eq!(depot.prod_queue()[0].progress, 1);
}

#[test]
fn critical_events_report_authoritative_supply_and_only_accepted_depot_upgrades() {
    let (mut game, depots) = fixture();
    assert!(game.critical_event_state()[0].upgrading_depots.is_empty());
    upgrade(&mut game, depots[0]);
    let facts = game.critical_event_state();
    assert_eq!(facts[0].upgrading_depots, vec![depots[0]]);
    assert_eq!(facts[0].supply_cap, game.snapshot_for(1).supply_cap);
    assert_eq!(facts[0].supply_used, game.snapshot_for(1).supply_used);
    assert!(facts[1].upgrading_depots.is_empty());
    upgrade(&mut game, depots[1]);
    upgrade(&mut game, depots[2]);
    assert_eq!(game.critical_event_state()[0].upgrading_depots.len(), 2);
}

#[test]
fn industrial_center_hp_bonus_preserves_damage_and_applies_once() {
    let (mut game, depots) = fixture();
    upgrade(&mut game, depots[0]);
    let depot = game.state.entities.get_mut(depots[0]).unwrap();
    depot.apply_damage(40, None);
    assert_eq!((depot.hp, depot.max_hp), (140, 180));
    let research = depot.research_queue_mut().unwrap().first_mut().unwrap();
    research.progress = research.total - 1;
    game.tick();
    let depot = game.state.entities.get(depots[0]).unwrap();
    assert_eq!((depot.hp, depot.max_hp), (500, 540));
    let ordinary = game.state.entities.get(depots[1]).unwrap();
    assert_eq!((ordinary.hp, ordinary.max_hp), (180, 180));
    upgrade(&mut game, depots[0]);
    let depot = game.state.entities.get(depots[0]).unwrap();
    assert_eq!((depot.hp, depot.max_hp), (500, 540));
    restore_checkpoint_and_assert_equivalent(&game, "damaged industrial center HP");
}
