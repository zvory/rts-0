use super::fixtures::*;
use super::*;

fn player(id: u32) -> PlayerInit {
    PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".to_string(),
        name: format!("Player {id}"),
        color: "#fff".to_string(),
        is_ai: false,
    }
}

fn refresh_derived_state(game: &mut Game) {
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state.fog.recompute(&ids, &game.state.entities, &game.state.map);
    game.assert_invariants();
}

fn depot_center(game: &Game, tile_x: u32, tile_y: u32) -> (f32, f32) {
    services::occupancy::footprint_center(&game.state.map, EntityKind::Depot, tile_x, tile_y)
}

fn staging_point(game: &Game, tile_x: u32, tile_y: u32, x_offset: i32) -> (f32, f32) {
    game.state.map
        .tile_center((tile_x as i32 + x_offset) as u32, tile_y)
}

fn notice_count(events: &[(u32, Vec<Event>)], player: u32, expected: &str) -> usize {
    events
        .iter()
        .filter(|(pid, _)| *pid == player)
        .flat_map(|(_, events)| events)
        .filter(|event| matches!(event, Event::Notice { msg, .. } if msg == expected))
        .count()
}

fn under_construction_depots(game: &Game) -> Vec<u32> {
    game.state.entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::Depot && entity.under_construction())
        .map(|entity| entity.id)
        .collect()
}

fn under_construction_city_centres(game: &Game) -> Vec<u32> {
    game.state.entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::CityCentre && entity.under_construction())
        .map(|entity| entity.id)
        .collect()
}

#[test]
fn build_wait_full_tick_retries_resources_without_notice_spam() {
    let players = [player(1)];
    let mut game = empty_flat_game(&players);
    let (tile_x, tile_y) = (10, 10);
    let worker_pos = staging_point(&game, tile_x, tile_y, -1);
    let worker = game.state.entities
        .spawn_unit(1, EntityKind::Worker, worker_pos.0, worker_pos.1)
        .expect("worker should spawn");
    game.state.players[0].set_resources(0, 0);
    refresh_derived_state(&mut game);

    game.enqueue(
        1,
        Command::Build {
            units: vec![worker],
            building: EntityKind::CityCentre,
            tile_x,
            tile_y,
            queued: false,
        },
    );

    let first_events = game.tick();
    assert_eq!(
        game.state.entities
            .get(worker)
            .expect("worker should survive")
            .build_phase(),
        Some(crate::game::entity::BuildPhase::WaitingAtSite)
    );
    assert!(
        under_construction_city_centres(&game).is_empty(),
        "broke arrival must not spawn a scaffold"
    );
    assert_eq!(notice_count(&first_events, 1, "Not enough steel"), 1);

    let repeat_events = game.tick();
    assert_eq!(
        notice_count(&repeat_events, 1, "Not enough steel"),
        0,
        "continuing the same resource wait should stay quiet"
    );
    assert!(
        under_construction_city_centres(&game).is_empty(),
        "resource wait must keep retrying without reserving or spawning"
    );

    let cost = crate::rules::economy::resource_cost(EntityKind::CityCentre);
    game.state.players[0].set_resources(cost.steel, cost.oil);
    game.tick();

    let scaffolds = under_construction_city_centres(&game);
    assert_eq!(scaffolds.len(), 1);
    assert_eq!(
        game.state.entities
            .get(worker)
            .expect("worker should survive")
            .build_phase(),
        Some(crate::game::entity::BuildPhase::Constructing { site: scaffolds[0] })
    );
    assert_eq!(game.state.players[0].steel, 0);
    assert_eq!(game.state.players[0].oil, 0);
}

#[test]
fn unit_block_timeout_preserves_queue_until_next_promotion_tick() {
    let players = [player(1)];
    let mut game = empty_flat_game(&players);
    let (tile_x, tile_y) = (10, 10);
    let worker_pos = staging_point(&game, tile_x, tile_y, -1);
    let site = depot_center(&game, tile_x, tile_y);
    let handoff = game.state.map.tile_center(15, 10);
    let worker = game.state.entities
        .spawn_unit(1, EntityKind::Worker, worker_pos.0, worker_pos.1)
        .expect("worker should spawn");
    game.state.entities
        .spawn_unit(1, EntityKind::Tank, site.0, site.1)
        .expect("unit blocker should spawn");
    {
        let worker = game.state.entities.get_mut(worker).expect("worker should exist");
        worker.set_order(Order::build(EntityKind::Depot, tile_x, tile_y));
        worker.append_queued_order(OrderIntent::move_to(handoff.0, handoff.1));
    }
    refresh_derived_state(&mut game);

    let grace_ticks = config::TICK_HZ * 3;
    let mut timeout_events = Vec::new();
    for _ in 0..grace_ticks {
        timeout_events = game.tick();
    }

    let worker_entity = game.state.entities.get(worker).expect("worker should survive");
    assert!(matches!(worker_entity.order(), Order::Idle));
    assert_eq!(
        worker_entity.queued_orders(),
        &[OrderIntent::move_to(handoff.0, handoff.1)],
        "timeout should drop only the active build order"
    );
    assert_eq!(notice_count(&timeout_events, 1, "Cannot build there"), 1);
    assert!(
        under_construction_depots(&game).is_empty(),
        "timed-out blocker must not spawn a scaffold"
    );

    game.tick();

    let worker_entity = game.state.entities.get(worker).expect("worker should survive");
    assert!(
        matches!(worker_entity.order(), Order::Move(_)),
        "queued handoff should promote on the next order-promotion pass"
    );
    assert!(worker_entity.queued_orders().is_empty());
}

#[test]
fn building_block_cancellation_preserves_queue_until_next_promotion_tick() {
    let players = [player(1), player(2)];
    let mut game = empty_flat_game(&players);
    let (tile_x, tile_y) = (10, 10);
    let worker_pos = staging_point(&game, tile_x, tile_y, -1);
    let site = depot_center(&game, tile_x, tile_y);
    let handoff = game.state.map.tile_center(15, 10);
    let worker = game.state.entities
        .spawn_unit(1, EntityKind::Worker, worker_pos.0, worker_pos.1)
        .expect("worker should spawn");
    game.state.entities
        .spawn_building(2, EntityKind::Depot, site.0, site.1, true)
        .expect("competing building should spawn");
    {
        let worker = game.state.entities.get_mut(worker).expect("worker should exist");
        worker.set_order(Order::build(EntityKind::Depot, tile_x, tile_y));
        worker.append_queued_order(OrderIntent::move_to(handoff.0, handoff.1));
    }
    refresh_derived_state(&mut game);

    let events = game.tick();

    let worker_entity = game.state.entities.get(worker).expect("worker should survive");
    assert!(matches!(worker_entity.order(), Order::Idle));
    assert_eq!(
        worker_entity.queued_orders(),
        &[OrderIntent::move_to(handoff.0, handoff.1)],
        "building-block cancellation should preserve queued handoff orders"
    );
    assert_eq!(notice_count(&events, 1, "Cannot build there"), 1);

    game.tick();

    let worker_entity = game.state.entities.get(worker).expect("worker should survive");
    assert!(matches!(worker_entity.order(), Order::Move(_)));
    assert!(worker_entity.queued_orders().is_empty());
}

#[test]
fn overlapping_build_race_charges_only_the_worker_that_spawns_scaffold() {
    let players = [player(1), player(2)];
    let mut game = empty_flat_game(&players);
    let (tile_x, tile_y) = (10, 10);
    let left = staging_point(&game, tile_x, tile_y, -1);
    let right = staging_point(&game, tile_x, tile_y, 3);
    let worker_a = game.state.entities
        .spawn_unit(1, EntityKind::Worker, left.0, left.1)
        .expect("first worker should spawn");
    let worker_b = game.state.entities
        .spawn_unit(2, EntityKind::Worker, right.0, right.1)
        .expect("second worker should spawn");
    let cost = crate::rules::economy::resource_cost(EntityKind::CityCentre);
    game.state.players[0].set_resources(cost.steel, cost.oil);
    game.state.players[1].set_resources(cost.steel, cost.oil);
    refresh_derived_state(&mut game);

    for player_id in [1, 2] {
        let worker = if player_id == 1 { worker_a } else { worker_b };
        game.enqueue(
            player_id,
            Command::Build {
                units: vec![worker],
                building: EntityKind::CityCentre,
                tile_x,
                tile_y,
                queued: false,
            },
        );
    }

    let events = game.tick();

    let scaffolds = under_construction_city_centres(&game);
    assert_eq!(scaffolds.len(), 1);
    let scaffold = game.state.entities
        .get(scaffolds[0])
        .expect("scaffold should exist");
    assert_eq!(
        scaffold.owner, 1,
        "lower-id first arrival should win the race"
    );
    assert_eq!(
        game.state.entities
            .get(worker_a)
            .expect("first worker should survive")
            .build_phase(),
        Some(crate::game::entity::BuildPhase::Constructing { site: scaffolds[0] })
    );
    assert!(matches!(
        game.state.entities
            .get(worker_b)
            .expect("second worker should survive")
            .order(),
        Order::Idle
    ));
    assert_eq!(game.state.players[0].steel, 0);
    assert_eq!(
        game.state.players[1].steel, cost.steel,
        "losing worker must not pay for a footprint already claimed by a building"
    );
    assert_eq!(notice_count(&events, 2, "Cannot build there"), 1);
}
