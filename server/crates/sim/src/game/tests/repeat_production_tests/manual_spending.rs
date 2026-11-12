use super::*;
use crate::game::entity::{BuildPhase, Order, ResearchItem};

fn queued_unit(game: &mut Game, building: u32, unit: EntityKind, paid: bool) {
    assert!(game
        .state
        .entities
        .get_mut(building)
        .unwrap()
        .push_production(ProdItem {
            unit,
            progress: 0,
            total: 100,
            paid,
        }));
}

fn repeat_count(game: &Game, building: u32) -> usize {
    game.state
        .entities
        .get(building)
        .unwrap()
        .prod_queue()
        .len()
}

fn builder(game: &mut Game, at_site: bool) -> u32 {
    let (x, y) = if at_site {
        footprint_center(&game.state.map, EntityKind::ResourceDepot, 22, 22)
    } else {
        game.state.map.tile_center(15, 15)
    };
    let worker = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, x, y)
        .unwrap();
    game.state
        .entities
        .get_mut(worker)
        .unwrap()
        .set_order(Order::build(EntityKind::ResourceDepot, 22, 22));
    worker
}

#[test]
fn manual_unit_gets_new_income_before_an_earlier_automatic_producer() {
    let (mut game, automatic) = repeat_fixture();
    let manual = spawn_building(&mut game, 1, EntityKind::Barracks, (15, 8));
    game.state.players[0].set_resources(0, 0);
    game.enqueue(
        1,
        SimCommand::Train {
            building: manual,
            unit: EntityKind::Rifleman,
        },
    );
    game.tick();
    assert!(!game.state.entities.get(manual).unwrap().prod_queue()[0].paid);

    let cost = rules::economy::resource_cost(EntityKind::Rifleman);
    game.state.players[0].set_resources(cost.steel, cost.oil);
    game.tick();
    let item = &game.state.entities.get(manual).unwrap().prod_queue()[0];
    assert!(item.paid);
    assert_eq!(item.progress, 1);
    assert_eq!(repeat_count(&game, automatic), 0);
}

#[test]
fn manual_extractor_payment_claims_the_patch_before_the_next_depot() {
    let mut game = empty_flat_game(&players());
    let first = spawn_building(&mut game, 1, EntityKind::ResourceDepot, (10, 10));
    let second = spawn_building(&mut game, 1, EntityKind::ResourceDepot, (18, 10));
    spawn_building(&mut game, 2, EntityKind::ResourceDepot, (50, 50));
    let patch = game.state.map.tile_center(15, 15);
    spawn_node(&mut game, EntityKind::Steel, patch);
    queued_unit(&mut game, first, EntityKind::SteelMine, false);
    queued_unit(&mut game, second, EntityKind::SteelMine, false);
    game.state.players[0].set_resources(10_000, 10_000);

    game.tick();

    let first_item = &game.state.entities.get(first).unwrap().prod_queue()[0];
    assert!(first_item.paid);
    assert_eq!(first_item.progress, 1);
    let second_item = &game.state.entities.get(second).unwrap().prod_queue()[0];
    assert!(
        !second_item.paid,
        "an occupied patch must leave the order unpaid"
    );
    assert_eq!(second_item.progress, 0);
    assert_eq!(
        game.state
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::SteelMine)
            .count(),
        1
    );
}

#[test]
fn oil_starved_manual_unit_allows_only_surplus_steel_to_auto_build() {
    let (mut game, automatic) = repeat_fixture();
    let second_auto = spawn_building(&mut game, 1, EntityKind::Barracks, (15, 8));
    game.state
        .entities
        .get_mut(second_auto)
        .unwrap()
        .set_repeat_production(Some(EntityKind::Rifleman), true);
    let factory = spawn_building(&mut game, 1, EntityKind::Factory, (30, 8));
    queued_unit(&mut game, factory, EntityKind::Tank, false);
    let tank = rules::economy::resource_cost(EntityKind::Tank);
    let rifle = rules::economy::resource_cost(EntityKind::Rifleman);
    assert!(tank.oil > 0 && rifle.oil == 0);
    game.state.players[0].set_resources(tank.steel + rifle.steel, 0);

    game.tick();
    assert_eq!(repeat_count(&game, automatic), 1);
    assert_eq!(repeat_count(&game, second_auto), 0);
    assert_eq!(game.state.players[0].steel, tank.steel);
    assert!(!game.state.entities.get(factory).unwrap().prod_queue()[0].paid);

    game.state.players[0].set_resources(tank.steel, tank.oil);
    game.tick();
    assert!(game.state.entities.get(factory).unwrap().prod_queue()[0].paid);
}

#[test]
fn ready_research_protects_its_budget_but_unfinished_research_does_not() {
    for complete in [false, true] {
        let (mut game, automatic) = repeat_fixture();
        let definition = upgrade::definition(UpgradeKind::TankUnlock);
        let (x, y) = footprint_center(&game.state.map, definition.researched_at, 30, 8);
        let building = game
            .state
            .entities
            .spawn_building(1, definition.researched_at, x, y, complete)
            .unwrap();
        game.state
            .entities
            .get_mut(building)
            .unwrap()
            .push_research(ResearchItem {
                upgrade: UpgradeKind::TankUnlock,
                progress: 0,
                total: 100,
                paid: false,
            });
        let rifle = rules::economy::resource_cost(EntityKind::Rifleman);
        assert!(definition.cost_oil > 0 && definition.cost_steel >= rifle.steel);
        game.state.players[0].set_resources(definition.cost_steel, 0);
        game.tick();
        assert_eq!(repeat_count(&game, automatic), usize::from(!complete));
    }
}

#[test]
fn construction_protects_on_first_arrival_and_retries_until_oil_arrives() {
    let (mut game, automatic) = repeat_fixture();
    let worker = builder(&mut game, true);
    let cost = rules::economy::resource_cost(EntityKind::ResourceDepot);
    assert!(cost.oil > 0);
    game.state.players[0].set_resources(cost.steel, 0);

    for _ in 0..3 {
        game.tick();
        assert_eq!(repeat_count(&game, automatic), 0);
        assert_eq!(game.state.players[0].steel, cost.steel);
        assert_eq!(
            game.state.entities.get(worker).unwrap().build_phase(),
            Some(BuildPhase::WaitingAtSite)
        );
    }
    game.state.players[0].set_resources(cost.steel, cost.oil);
    game.tick();
    assert!(matches!(
        game.state.entities.get(worker).unwrap().build_phase(),
        Some(BuildPhase::Constructing { .. })
    ));
    assert_eq!(game.state.players[0].steel, 0);

    // Paid construction still in progress must not hold another building's worth of money.
    let rifle = rules::economy::resource_cost(EntityKind::Rifleman);
    game.state.players[0].set_resources(rifle.steel, 0);
    game.tick();
    assert_eq!(repeat_count(&game, automatic), 1);
}

#[test]
fn travelling_or_queued_construction_does_not_protect_resources() {
    for queued in [false, true] {
        let (mut game, automatic) = repeat_fixture();
        let worker = builder(&mut game, false);
        if queued {
            let worker = game.state.entities.get_mut(worker).unwrap();
            worker.clear_active_order();
            worker.set_order(Order::HoldPosition);
            worker.append_queued_order(crate::game::entity::OrderIntent::build(
                EntityKind::ResourceDepot,
                22,
                22,
            ));
        }
        let rifle = rules::economy::resource_cost(EntityKind::Rifleman);
        game.state.players[0].set_resources(rifle.steel, 0);
        game.tick();
        assert_eq!(repeat_count(&game, automatic), 1);
    }
}

#[test]
fn cancelled_dead_or_invalid_build_orders_release_the_budget() {
    for invalidation in 0..3 {
        let (mut game, automatic) = repeat_fixture();
        let worker = builder(&mut game, true);
        let cost = rules::economy::resource_cost(EntityKind::ResourceDepot);
        game.state.players[0].set_resources(cost.steel, 0);
        game.tick();
        assert_eq!(repeat_count(&game, automatic), 0);
        match invalidation {
            0 => game
                .state
                .entities
                .get_mut(worker)
                .unwrap()
                .clear_active_order(),
            1 => {
                game.state
                    .entities
                    .get_mut(worker)
                    .unwrap()
                    .apply_damage(u32::MAX, None);
            }
            _ => {
                let (x, y) = game.state.map.tile_center(21, 24);
                let builder = game.state.entities.get_mut(worker).unwrap();
                builder.set_position(x, y);
                spawn_building(&mut game, 2, EntityKind::ResourceDepot, (22, 22));
            }
        }
        game.tick();
        assert_eq!(repeat_count(&game, automatic), 1);
    }
}

#[test]
fn items_behind_a_paid_front_do_not_protect_resources() {
    let (mut game, automatic) = repeat_fixture();
    let factory = spawn_building(&mut game, 1, EntityKind::Factory, (30, 8));
    queued_unit(&mut game, factory, EntityKind::Tank, true);
    queued_unit(&mut game, factory, EntityKind::Tank, false);
    let rifle = rules::economy::resource_cost(EntityKind::Rifleman);
    game.state.players[0].set_resources(rifle.steel, 0);
    game.tick();
    assert_eq!(repeat_count(&game, automatic), 1);
}

#[test]
fn pending_costs_sum_across_ready_purchases_and_stay_owner_scoped() {
    let (mut game, automatic) = repeat_fixture();
    builder(&mut game, true);
    let depot_cost = rules::economy::resource_cost(EntityKind::ResourceDepot);
    let factory = spawn_building(&mut game, 1, EntityKind::Factory, (30, 8));
    queued_unit(&mut game, factory, EntityKind::Tank, false);
    let tank_cost = rules::economy::resource_cost(EntityKind::Tank);
    game.state.players[0].set_resources(depot_cost.steel + tank_cost.steel, 0);
    game.tick();
    assert_eq!(repeat_count(&game, automatic), 0);

    // Removing this player's commitments leaves enemy commitments irrelevant.
    game.state.entities.remove(factory);
    let enemy_factory = spawn_building(&mut game, 2, EntityKind::Factory, (30, 8));
    queued_unit(&mut game, enemy_factory, EntityKind::Tank, false);
    let workers: Vec<_> = game
        .state
        .entities
        .iter()
        .filter(|e| e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .collect();
    for id in workers {
        game.state
            .entities
            .get_mut(id)
            .unwrap()
            .clear_active_order();
    }
    game.tick();
    assert_eq!(repeat_count(&game, automatic), 1);
}

#[test]
fn menu_floor_and_pending_budget_overlap_instead_of_adding() {
    let (mut game, automatic) = repeat_fixture();
    builder(&mut game, true);
    let cost = rules::economy::resource_cost(EntityKind::ResourceDepot);
    let rifle = rules::economy::resource_cost(EntityKind::Rifleman);
    game.state.players[0].auto_build.reserve_steel = cost.steel;
    game.state.players[0].set_resources(cost.steel + rifle.steel, 0);
    game.tick();
    assert_eq!(repeat_count(&game, automatic), 1);
    assert_eq!(game.state.players[0].steel, cost.steel);
}
