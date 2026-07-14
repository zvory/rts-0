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
fn destroyed_producer_refunds_in_progress_and_queued_units() {
    let mut game =
        Game::new_for_replay_with_starting_resources(&players(), 5_000, 5_000, 0xD1E5_0001);
    let city_centre = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::CityCentre)
        .map(|entity| entity.id)
        .expect("player city centre should exist");
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
                building: city_centre,
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
        .get(city_centre)
        .expect("city centre should survive production tick");
    assert_eq!(producer.prod_queue().len(), 2);
    assert!(producer.prod_queue()[0].progress > 0);
    let player = game
        .state
        .players
        .iter()
        .find(|player| player.id == 1)
        .expect("player one should exist");
    assert_eq!(player.steel, starting_steel - 2 * worker_cost.steel);
    assert_eq!(player.oil, starting_oil - 2 * worker_cost.oil);
    assert_eq!(player.supply_used, starting_supply + 2 * worker_supply);

    {
        let entity = game
            .state
            .entities
            .get_mut(city_centre)
            .expect("city centre should exist before destruction");
        entity.apply_damage(entity.max_hp, None);
    }
    game.tick();

    assert!(game.state.entities.get(city_centre).is_none());
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
fn destroyed_research_building_refunds_in_progress_research() {
    let mut game =
        Game::new_for_replay_with_starting_resources(&players(), 5_000, 5_000, 0xD1E5_0002);
    let (tile_x, tile_y) = (0..game.state.map.size)
        .flat_map(|tile_y| (0..game.state.map.size).map(move |tile_x| (tile_x, tile_y)))
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
