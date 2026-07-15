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
        .set_repeat_production(Some(EntityKind::Rifleman), true);
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
        .set_repeat_production(None, false);
    game.state
        .entities
        .get_mut(barracks)
        .expect("barracks")
        .set_repeat_production(Some(EntityKind::Tank), true);
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

#[test]
fn repeat_production_alternates_enabled_units() {
    let (mut game, barracks) = repeat_fixture();
    spawn_building(&mut game, 1, EntityKind::TrainingCentre, (14, 8));
    game.state
        .entities
        .get_mut(barracks)
        .expect("barracks")
        .set_repeat_production(Some(EntityKind::MachineGunner), true);
    game.state.players[0].set_resources(10_000, 10_000);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);

    for (index, expected) in [
        EntityKind::Rifleman,
        EntityKind::MachineGunner,
        EntityKind::Rifleman,
    ]
    .into_iter()
    .enumerate()
    {
        game.tick();
        if index == 0 {
            let repeat_kinds = game
                .snapshot_for(1)
                .entities
                .into_iter()
                .find(|entity| entity.id == barracks)
                .expect("barracks projection")
                .prod_repeat_kinds;
            assert_eq!(
                repeat_kinds,
                vec!["rifleman".to_string(), "machine_gunner".to_string()]
            );
        }
        assert_eq!(
            game.state
                .entities
                .get(barracks)
                .expect("barracks")
                .prod_queue()[0]
                .unit,
            expected
        );
        game.state
            .entities
            .get_mut(barracks)
            .expect("barracks")
            .remove_front_production();
    }
}

#[test]
fn disabling_repeat_units_preserves_the_next_unit() {
    let (mut game, barracks) = repeat_fixture();
    let producer = game.state.entities.get_mut(barracks).expect("barracks");
    producer.set_repeat_production(Some(EntityKind::MachineGunner), true);
    producer.set_repeat_production(Some(EntityKind::Rifleman), true);

    producer.set_repeat_production(None, true);
    assert_eq!(
        producer.repeat_production(),
        Some(EntityKind::MachineGunner)
    );

    producer.set_repeat_production(Some(EntityKind::Rifleman), false);
    assert_eq!(
        producer.repeat_production(),
        Some(EntityKind::MachineGunner),
        "removing a later unit must not move the cursor"
    );

    producer.set_repeat_production(Some(EntityKind::Rifleman), true);
    producer.set_repeat_production(Some(EntityKind::Worker), false);
    assert_eq!(
        producer.repeat_production(),
        Some(EntityKind::MachineGunner),
        "removing an earlier unit must preserve the cursor's semantic target"
    );

    producer.set_repeat_production(Some(EntityKind::MachineGunner), false);
    producer.set_repeat_production(Some(EntityKind::Rifleman), true);
    assert_eq!(
        &producer
            .production
            .as_ref()
            .expect("production")
            .repeat_units,
        &[EntityKind::Rifleman],
        "removing the current unit must select its successor without adding duplicates"
    );
    assert_eq!(producer.repeat_production(), Some(EntityKind::Rifleman));
}
