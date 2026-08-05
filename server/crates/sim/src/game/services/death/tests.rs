use crate::game::command::SimCommand as Command;
use crate::game::{services, upgrade, Game, PlayerInit};

use super::*;

fn players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Human".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Computer".into(),
            color: "#000".into(),
            is_ai: true,
        },
    ]
}

#[test]
fn destroyed_producer_refunds_paid_work_but_not_unpaid_queue_entries() {
    let mut game =
        Game::new_for_replay_with_starting_resources(&players(), 5_000, 5_000, 0xD1E5_0001);
    let resource_depot = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::ResourceDepot)
        .map(|entity| entity.id)
        .expect("player resource depot should exist");
    let (starting_steel, starting_oil, starting_supply) = game
        .state
        .players
        .iter()
        .find(|player| player.id == 1)
        .map(|player| (player.steel, player.oil, player.supply_used))
        .expect("player one should exist");

    for _ in 0..2 {
        game.enqueue(
            1,
            Command::Train {
                building: resource_depot,
                unit: EntityKind::Worker,
            },
        );
    }
    game.tick();

    let worker_cost = economy::resource_cost(EntityKind::Worker);
    let worker_supply = economy::supply_cost(EntityKind::Worker);
    let producer = game
        .state
        .entities
        .get(resource_depot)
        .expect("resource depot should survive production tick");
    assert_eq!(producer.prod_queue().len(), 2);
    assert!(producer.prod_queue()[0].progress > 0);
    assert!(producer.prod_queue()[0].paid);
    assert!(!producer.prod_queue()[1].paid);
    let player = game
        .state
        .players
        .iter()
        .find(|player| player.id == 1)
        .expect("player one should exist");
    assert_eq!(player.steel, starting_steel - worker_cost.steel);
    assert_eq!(player.oil, starting_oil - worker_cost.oil);
    assert_eq!(player.supply_used, starting_supply + worker_supply);

    {
        let entity = game
            .state
            .entities
            .get_mut(resource_depot)
            .expect("resource depot should exist before destruction");
        entity.apply_damage(entity.max_hp, None);
    }
    game.tick();

    assert!(game.state.entities.get(resource_depot).is_none());
    let player = game
        .state
        .players
        .iter()
        .find(|player| player.id == 1)
        .expect("player one should exist after destruction");
    assert_eq!(player.steel, starting_steel);
    assert_eq!(player.oil, starting_oil);
    assert_eq!(player.supply_used, starting_supply);
}

#[test]
fn destroyed_depot_does_not_refund_linked_extractor_construction() {
    let mut game =
        Game::new_for_replay_with_starting_resources(&players(), 5_000, 5_000, 0xD1E5_0003);
    let resource_depot = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::ResourceDepot)
        .map(|entity| entity.id)
        .expect("player resource depot should exist");
    let starting_steel = game.state.players[0].steel;

    game.enqueue(
        1,
        Command::Train {
            building: resource_depot,
            unit: EntityKind::PumpJack,
        },
    );
    game.tick();

    let pump_jack_cost = economy::resource_cost(EntityKind::PumpJack);
    assert_eq!(
        game.state.players[0].steel,
        starting_steel - pump_jack_cost.steel
    );
    let scaffold = game
        .state
        .entities
        .iter()
        .find(|entity| entity.construction_producer_id() == Some(resource_depot))
        .map(|entity| entity.id)
        .expect("extractor scaffold should be linked to its producer");

    {
        let entity = game
            .state
            .entities
            .get_mut(resource_depot)
            .expect("resource depot should exist before destruction");
        entity.apply_damage(entity.max_hp, None);
    }
    game.tick();

    assert!(game.state.entities.get(resource_depot).is_none());
    assert!(game.state.entities.get(scaffold).is_none());
    assert_eq!(
        game.state.players[0].steel,
        starting_steel - pump_jack_cost.steel,
        "extractor construction destroyed with its producer must not be refunded"
    );
}

#[test]
fn destroyed_automatic_extractor_scaffold_waits_five_seconds_then_restarts_free() {
    let mut game =
        Game::new_for_replay_with_starting_resources(&players(), 5_000, 5_000, 0xD1E5_0004);
    let resource_depot = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::ResourceDepot)
        .map(|entity| entity.id)
        .expect("player resource depot should exist");
    let starting_resources = (game.state.players[0].steel, game.state.players[0].oil);
    game.tick();

    let (scaffold, kind) = game
        .state
        .entities
        .iter()
        .find(|entity| entity.construction_producer_id() == Some(resource_depot))
        .map(|entity| (entity.id, entity.kind))
        .expect("automatic extractor scaffold should be linked to its producer");
    let sibling = game
        .state
        .entities
        .iter()
        .find(|entity| {
            entity.kind.is_resource_extractor()
                && entity.kind != kind
                && entity.resource_extractor_producer_id() == Some(resource_depot)
        })
        .map(|entity| entity.id)
        .expect("the other automatic extractor scaffold should exist");
    let sibling_progress_before = game
        .state
        .entities
        .get(sibling)
        .and_then(|entity| entity.build_progress_fraction())
        .expect("the other automatic extractor should be under construction");
    {
        let entity = game
            .state
            .entities
            .get_mut(scaffold)
            .expect("automatic extractor scaffold should exist before destruction");
        entity.apply_damage(entity.max_hp, None);
    }

    game.tick();

    assert!(game.state.entities.get(scaffold).is_none());
    assert!(game
        .state
        .entities
        .get(resource_depot)
        .expect("resource depot should survive")
        .prod_queue()
        .is_empty());
    let sibling_progress_after = game
        .state
        .entities
        .get(sibling)
        .and_then(|entity| entity.build_progress_fraction())
        .expect("the other automatic extractor should keep building");
    assert!(sibling_progress_after > sibling_progress_before);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        starting_resources,
        "automatic extractor construction must not spend or refund resources"
    );

    for _ in 0..config::TICK_HZ * 5 - 1 {
        game.tick();
        assert!(game.state.entities.iter().all(|entity| {
            entity.kind != kind
                || !entity.under_construction()
                || entity.resource_extractor_producer_id() != Some(resource_depot)
        }));
    }

    game.tick();

    let replacement = game
        .state
        .entities
        .iter()
        .find(|entity| {
            entity.kind == kind && entity.construction_producer_id() == Some(resource_depot)
        })
        .expect("permanent automatic job should replace the destroyed scaffold");
    assert_ne!(replacement.id, scaffold);
}

#[test]
fn killed_completed_automatic_extractor_cooldown_survives_checkpoint_restore() {
    let mut game =
        Game::new_for_replay_with_starting_resources(&players(), 5_000, 5_000, 0xD1E5_0005);
    let resource_depot = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::ResourceDepot)
        .map(|entity| entity.id)
        .expect("player resource depot should exist");

    for _ in 0..config::building_stats(EntityKind::PumpJack)
        .expect("pump jack stats")
        .build_ticks
    {
        game.tick();
    }

    let killed = game
        .state
        .entities
        .iter()
        .find(|entity| {
            entity.kind == EntityKind::PumpJack
                && !entity.under_construction()
                && entity.resource_extractor_producer_id() == Some(resource_depot)
        })
        .map(|entity| entity.id)
        .expect("automatic Pump Jack should complete and retain its producer");
    game.state
        .entities
        .get_mut(killed)
        .expect("completed Pump Jack should exist before destruction")
        .apply_damage(u32::MAX, None);

    game.tick();
    assert!(game.state.entities.get(killed).is_none());
    assert!(game.state.entities.iter().all(|entity| {
        entity.kind != EntityKind::PumpJack
            || entity.resource_extractor_producer_id() != Some(resource_depot)
    }));
    let checkpoint = game
        .checkpoint_payload_text_for_test()
        .expect("extractor restart cooldown checkpoint");
    let map = game.state.map.clone();
    let map_metadata = game.map_metadata().clone();
    game = Game::restore_checkpoint_payload_text_for_test(&checkpoint, map, map_metadata)
        .expect("restore extractor restart cooldown");

    for _ in 0..config::TICK_HZ * 5 - 1 {
        game.tick();
        assert!(game.state.entities.iter().all(|entity| {
            entity.kind != EntityKind::PumpJack
                || entity.resource_extractor_producer_id() != Some(resource_depot)
        }));
    }

    game.tick();
    assert!(game.state.entities.iter().any(|entity| {
        entity.kind == EntityKind::PumpJack
            && entity.under_construction()
            && entity.resource_extractor_producer_id() == Some(resource_depot)
    }));
}

#[test]
fn destroyed_research_building_refunds_in_progress_research() {
    let mut game =
        Game::new_for_replay_with_starting_resources(&players(), 5_000, 5_000, 0xD1E5_0002);
    let (tile_x, tile_y) = (0..game.state.map.width)
        .flat_map(|tile_y| (0..game.state.map.width).map(move |tile_x| (tile_x, tile_y)))
        .find(|(tile_x, tile_y)| {
            services::standability::building_site_clear(
                &game.state.map,
                &game.state.entities,
                EntityKind::TrainingCentre,
                *tile_x,
                *tile_y,
            )
        })
        .expect("map should have a clear training centre site");
    let (x, y) = services::occupancy::footprint_center(
        &game.state.map,
        EntityKind::TrainingCentre,
        tile_x,
        tile_y,
    );
    let training_centre = game
        .state
        .entities
        .spawn_building(1, EntityKind::TrainingCentre, x, y, true)
        .expect("training centre should spawn");
    let (starting_steel, starting_oil) = game
        .state
        .players
        .iter()
        .find(|player| player.id == 1)
        .map(|player| (player.steel, player.oil))
        .expect("player one should exist");

    game.enqueue(
        1,
        Command::Research {
            building: training_centre,
            upgrade: upgrade::UpgradeKind::Entrenchment,
        },
    );
    game.tick();

    let research = game
        .state
        .entities
        .get(training_centre)
        .and_then(|building| building.research_queue().first())
        .expect("research should be in progress");
    assert!(research.progress > 0);
    let definition = upgrade::definition(upgrade::UpgradeKind::Entrenchment);
    let player = game
        .state
        .players
        .iter()
        .find(|player| player.id == 1)
        .expect("player one should exist");
    assert_eq!(player.steel, starting_steel - definition.cost_steel);
    assert_eq!(player.oil, starting_oil - definition.cost_oil);

    {
        let entity = game
            .state
            .entities
            .get_mut(training_centre)
            .expect("training centre should exist before destruction");
        entity.apply_damage(entity.max_hp, None);
    }
    game.tick();

    assert!(game.state.entities.get(training_centre).is_none());
    let player = game
        .state
        .players
        .iter()
        .find(|player| player.id == 1)
        .expect("player one should exist after destruction");
    assert_eq!(player.steel, starting_steel);
    assert_eq!(player.oil, starting_oil);
}
