use super::fixtures::empty_flat_game;
use super::*;
use crate::game::entity::EntityKind;
use crate::game::services::occupancy::footprint_center;
use crate::game::upgrade::UpgradeKind;
use crate::rules;

fn players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".to_string(),
            color: "#fff".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".to_string(),
            color: "#000".to_string(),
            is_ai: false,
        },
    ]
}

fn spawn_building(game: &mut Game, owner: u32, kind: EntityKind, tile: (u32, u32)) -> u32 {
    let (x, y) = footprint_center(&game.state.map, kind, tile.0, tile.1);
    game.state
        .entities
        .spawn_building(owner, kind, x, y, true)
        .expect("building should spawn")
}

fn repeat_fixture() -> (Game, u32) {
    let mut game = empty_flat_game(&players());
    spawn_building(&mut game, 1, EntityKind::CityCentre, (3, 3));
    spawn_building(&mut game, 2, EntityKind::CityCentre, (50, 50));
    let barracks = spawn_building(&mut game, 1, EntityKind::Barracks, (8, 8));
    game.state
        .entities
        .get_mut(barracks)
        .expect("barracks")
        .set_repeat_production(Some(EntityKind::Rifleman));
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    (game, barracks)
}

#[test]
fn repeat_production_retries_then_charges_and_reserves_once() {
    let (mut game, barracks) = repeat_fixture();
    let cost = rules::economy::resource_cost(EntityKind::Rifleman);
    let supply = rules::economy::supply_cost(EntityKind::Rifleman);
    game.state.players[0].set_resources(cost.steel.saturating_sub(1), cost.oil);

    game.tick();
    assert!(game
        .state
        .entities
        .get(barracks)
        .expect("barracks")
        .prod_queue()
        .is_empty());
    assert_eq!(game.state.players[0].supply_used, 0);

    game.state.players[0].set_resources(cost.steel, cost.oil);
    game.tick();
    let queued = game
        .state
        .entities
        .get(barracks)
        .expect("barracks")
        .prod_queue();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].unit, EntityKind::Rifleman);
    assert_eq!(queued[0].progress, 1);
    assert_eq!(game.state.players[0].steel, 0);
    assert_eq!(game.state.players[0].oil, 0);
    assert_eq!(game.state.players[0].supply_used, supply);

    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(barracks)
            .expect("barracks")
            .prod_queue()
            .len(),
        1
    );
    assert_eq!(game.state.players[0].supply_used, supply);
}

#[test]
fn repeat_production_revalidates_producer_compatibility() {
    let (mut game, barracks) = repeat_fixture();
    spawn_building(&mut game, 1, EntityKind::Factory, (14, 8));
    game.state
        .entities
        .get_mut(barracks)
        .expect("barracks")
        .set_repeat_production(Some(EntityKind::Tank));
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::TankUnlock);
    let cost = rules::economy::resource_cost(EntityKind::Tank);
    game.state.players[0].set_resources(cost.steel, cost.oil);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);

    game.tick();

    assert!(game
        .state
        .entities
        .get(barracks)
        .expect("barracks")
        .prod_queue()
        .is_empty());
    assert_eq!(game.state.players[0].steel, cost.steel);
    assert_eq!(game.state.players[0].oil, cost.oil);
    assert_eq!(game.state.players[0].supply_used, 0);
}
