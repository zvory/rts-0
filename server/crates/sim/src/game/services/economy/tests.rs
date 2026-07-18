use super::*;
use crate::game::entity::{EntityKind, EntityStore, GatherPhase, Order, OrderIntent};
use crate::game::map::Map;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::{footprint_center, Occupancy};
use crate::game::services::pathing::PathingService;
use crate::game::teams::TeamRelations;
use crate::protocol::terrain;

fn flat_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(4, 4)],
        base_sites: Vec::new(),
    }
}

fn team_relations(players: &[(u32, u32)]) -> TeamRelations {
    TeamRelations::from_player_teams(players.iter().copied())
}

fn spawn_completed_mining_anchor(entities: &mut EntityStore, owner: u32, x: f32, y: f32) {
    entities
        .spawn_building(
            owner,
            EntityKind::CityCentre,
            x - config::TILE_SIZE as f32 * 2.0,
            y,
            true,
        )
        .expect("completed mining anchor should spawn");
}

#[test]
fn entering_harvesting_clears_pending_queued_orders() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (ccx, ccy) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, ccx, ccy, true)
        .expect("city centre should spawn");
    let node = entities
        .spawn_node(EntityKind::Steel, ccx + 48.0, ccy)
        .expect("steel node should spawn");
    let (nx, ny) = entities
        .get(node)
        .map(|n| (n.pos_x, n.pos_y))
        .expect("node pos");
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, nx, ny)
        .expect("worker should spawn");
    {
        let w = entities.get_mut(worker).expect("worker should exist");
        w.set_order(Order::gather(node));
        w.set_target_id(Some(node));
        w.append_queued_order(OrderIntent::move_to(nx + 96.0, ny));
    }

    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(1024, 32);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    let mut players = Vec::new();

    gather_system(
        &map,
        &mut entities,
        &mut players,
        &occ,
        &mut coordinator,
        &team_relations(&[]),
        1,
    );

    let w = entities.get(worker).expect("worker should exist");
    assert_eq!(
        w.gather_phase(),
        Some(GatherPhase::Harvesting),
        "worker in interact range should transition to harvesting"
    );
    assert!(
        w.queued_orders().is_empty(),
        "harvesting transition is terminal; queued handoff orders should be dropped"
    );
}

#[test]
fn worker_direct_oil_gather_order_is_idled() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (ccx, ccy) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, ccx, ccy, true)
        .expect("city centre should spawn");
    let oil = entities
        .spawn_node(EntityKind::Oil, ccx + 48.0, ccy)
        .expect("oil node should spawn");
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, ccx + 48.0, ccy)
        .expect("worker should spawn");
    {
        let w = entities.get_mut(worker).expect("worker should exist");
        w.set_order(Order::gather(oil));
        w.set_target_id(Some(oil));
    }

    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(1024, 32);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    let mut players = Vec::new();

    gather_system(
        &map,
        &mut entities,
        &mut players,
        &occ,
        &mut coordinator,
        &team_relations(&[]),
        1,
    );

    let w = entities.get(worker).expect("worker should exist");
    assert!(
        !matches!(w.order(), Order::Gather(_)),
        "workers must not direct-mine oil patches"
    );
    assert_eq!(
        entities.node_slot_holder(oil),
        None,
        "oil patches should not reserve worker mining slots"
    );
}

#[test]
fn completed_pump_jack_mines_overlapping_oil_at_worker_rate() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (pump_x, pump_y) = footprint_center(&map, EntityKind::PumpJack, 4, 4);
    let oil = entities
        .spawn_node(EntityKind::Oil, pump_x, pump_y)
        .expect("oil node should spawn");
    entities
        .spawn_building(1, EntityKind::PumpJack, pump_x, pump_y, true)
        .expect("pump jack should spawn");
    spawn_completed_mining_anchor(&mut entities, 1, pump_x, pump_y);
    let teams = team_relations(&[(1, 1)]);
    let oil_before = entities
        .get(oil)
        .and_then(|node| node.remaining())
        .expect("oil node remaining");
    let mut payouts = Vec::new();

    for _ in 0..config::HARVEST_TICKS {
        payouts.extend(pump_jack::tick(&mut entities, &teams));
    }

    assert_eq!(payouts.len(), 1);
    assert_eq!(payouts[0].owner, 1);
    assert_eq!(
        payouts[0].oil,
        config::OIL_LOAD,
        "Pump Jack should pay the same oil load as one worker harvest"
    );
    assert_eq!(
        entities.get(oil).and_then(|node| node.remaining()),
        Some(oil_before - config::OIL_LOAD),
        "Pump Jack income should deplete the oil node by the paid amount"
    );
}

#[test]
fn pump_jack_waits_for_a_completed_friendly_mining_anchor() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let (pump_x, pump_y) = footprint_center(&map, EntityKind::PumpJack, 4, 4);
    let (far_x, far_y) = footprint_center(&map, EntityKind::CityCentre, 24, 24);
    let oil = entities
        .spawn_node(EntityKind::Oil, pump_x, pump_y)
        .expect("oil node should spawn");
    entities
        .spawn_building(1, EntityKind::PumpJack, pump_x, pump_y, true)
        .expect("pump jack should spawn");
    entities
        .spawn_building(1, EntityKind::CityCentre, far_x, far_y, true)
        .expect("distant owned city centre should spawn");
    spawn_completed_mining_anchor(&mut entities, 3, pump_x, pump_y);
    let incomplete_ally = entities
        .spawn_building(
            2,
            EntityKind::CityCentre,
            pump_x,
            pump_y - config::TILE_SIZE as f32 * 2.0,
            false,
        )
        .expect("incomplete allied city centre should spawn");
    let teams = team_relations(&[(1, 7), (2, 7), (3, 3)]);
    let oil_before = entities
        .get(oil)
        .and_then(|node| node.remaining())
        .expect("oil node remaining");

    for _ in 0..config::HARVEST_TICKS.saturating_mul(2) {
        assert!(pump_jack::tick(&mut entities, &teams).is_empty());
    }
    assert_eq!(
        entities.get(oil).and_then(|node| node.remaining()),
        Some(oil_before),
        "distant owned, nearby enemy, and incomplete allied anchors must not enable extraction"
    );

    let _ = entities.remove(incomplete_ally);
    spawn_completed_mining_anchor(&mut entities, 2, pump_x, pump_y);
    let mut payouts = Vec::new();
    for _ in 0..config::HARVEST_TICKS {
        payouts.extend(pump_jack::tick(&mut entities, &teams));
    }

    assert_eq!(payouts.len(), 1);
    assert_eq!(payouts[0].owner, 1);
    assert_eq!(payouts[0].oil, config::OIL_LOAD);
    assert_eq!(
        entities.get(oil).and_then(|node| node.remaining()),
        Some(oil_before - config::OIL_LOAD),
        "a completed allied mining anchor should activate the Pump Jack"
    );
}

#[test]
fn pump_jack_disappears_with_its_depleted_oil_patch() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (pump_x, pump_y) = footprint_center(&map, EntityKind::PumpJack, 4, 4);
    let oil = entities
        .spawn_node(EntityKind::Oil, pump_x, pump_y)
        .expect("oil node should spawn");
    let pump = entities
        .spawn_building(1, EntityKind::PumpJack, pump_x, pump_y, true)
        .expect("pump jack should spawn");
    spawn_completed_mining_anchor(&mut entities, 1, pump_x, pump_y);
    let teams = team_relations(&[(1, 1)]);
    let remaining_before_final_load = entities
        .get(oil)
        .and_then(|node| node.remaining())
        .expect("oil node remaining")
        .saturating_sub(config::OIL_LOAD);
    entities
        .get_mut(oil)
        .expect("oil node should exist")
        .harvest_resources(remaining_before_final_load);

    for _ in 0..config::HARVEST_TICKS.saturating_sub(1) {
        assert!(pump_jack::tick(&mut entities, &teams).is_empty());
        assert!(entities.contains(pump));
    }

    let payouts = pump_jack::tick(&mut entities, &teams);

    assert_eq!(payouts.len(), 1);
    assert_eq!(payouts[0].oil, config::OIL_LOAD);
    assert_eq!(entities.get(oil).and_then(|node| node.remaining()), Some(0));
    assert!(
        !entities.contains(pump),
        "Pump Jack should disappear when it extracts the last oil"
    );
}

#[test]
fn pump_jack_does_not_retarget_another_oil_patch_after_depletion() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (pump_x, pump_y) = footprint_center(&map, EntityKind::PumpJack, 4, 4);
    let depleted_oil = entities
        .spawn_node(EntityKind::Oil, pump_x, pump_y)
        .expect("oil node should spawn");
    let other_oil = entities
        .spawn_node(
            EntityKind::Oil,
            pump_x + config::TILE_SIZE as f32 * 0.25,
            pump_y,
        )
        .expect("second oil node should spawn");
    let pump = entities
        .spawn_building(1, EntityKind::PumpJack, pump_x, pump_y, true)
        .expect("pump jack should spawn");
    spawn_completed_mining_anchor(&mut entities, 1, pump_x, pump_y);
    let teams = team_relations(&[(1, 1)]);
    let remaining_before_final_load = entities
        .get(depleted_oil)
        .and_then(|node| node.remaining())
        .expect("oil node remaining")
        .saturating_sub(config::OIL_LOAD);
    entities
        .get_mut(depleted_oil)
        .expect("oil node should exist")
        .harvest_resources(remaining_before_final_load);
    let other_oil_before = entities
        .get(other_oil)
        .and_then(|node| node.remaining())
        .expect("second oil node remaining");

    let mut payouts = Vec::new();
    for _ in 0..config::HARVEST_TICKS {
        payouts.extend(pump_jack::tick(&mut entities, &teams));
    }

    assert_eq!(payouts.len(), 1);
    assert_eq!(payouts[0].oil, config::OIL_LOAD);
    assert!(
        !entities.contains(pump),
        "Pump Jack should disappear when its supporting patch is depleted"
    );
    assert_eq!(
        entities.get(other_oil).and_then(|node| node.remaining()),
        Some(other_oil_before),
        "Pump Jack must not retarget another oil patch in its footprint"
    );
}

#[test]
fn pump_jack_mines_only_oil_centered_in_its_footprint() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (pump_x, pump_y) = footprint_center(&map, EntityKind::PumpJack, 4, 4);
    let (adjacent_x, adjacent_y) = footprint_center(&map, EntityKind::PumpJack, 5, 4);
    let adjacent_oil = entities
        .spawn_node(EntityKind::Oil, adjacent_x, adjacent_y)
        .expect("adjacent oil node should spawn");
    let centered_oil = entities
        .spawn_node(EntityKind::Oil, pump_x, pump_y)
        .expect("centered oil node should spawn");
    entities
        .spawn_building(1, EntityKind::PumpJack, pump_x, pump_y, true)
        .expect("pump jack should spawn");
    spawn_completed_mining_anchor(&mut entities, 1, pump_x, pump_y);
    let teams = team_relations(&[(1, 1)]);
    let adjacent_before = entities
        .get(adjacent_oil)
        .and_then(|node| node.remaining())
        .expect("adjacent oil remaining");
    let centered_before = entities
        .get(centered_oil)
        .and_then(|node| node.remaining())
        .expect("centered oil remaining");

    let mut payouts = Vec::new();
    for _ in 0..config::HARVEST_TICKS {
        payouts.extend(pump_jack::tick(&mut entities, &teams));
    }

    assert_eq!(payouts.len(), 1);
    assert_eq!(
        entities.get(adjacent_oil).and_then(|node| node.remaining()),
        Some(adjacent_before),
        "edge-touching adjacent oil must not be depleted by this Pump Jack"
    );
    assert_eq!(
        entities.get(centered_oil).and_then(|node| node.remaining()),
        Some(centered_before - config::OIL_LOAD),
        "Pump Jack should extract from the oil whose center lies inside its footprint"
    );
}

#[test]
fn occupied_resource_arrival_redirects_to_nearest_same_resource_node() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let (ccx, ccy) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, ccx, ccy, true)
        .expect("city centre should spawn");
    let occupied = entities
        .spawn_node(EntityKind::Steel, ccx + 48.0, ccy)
        .expect("occupied steel node should spawn");
    let nearby = entities
        .spawn_node(
            EntityKind::Steel,
            ccx + 48.0 + config::TILE_SIZE as f32 * 3.0,
            ccy,
        )
        .expect("nearby steel node should spawn");
    let farther = entities
        .spawn_node(
            EntityKind::Steel,
            ccx + 48.0 + config::TILE_SIZE as f32 * 6.0,
            ccy,
        )
        .expect("farther steel node should spawn");
    let oil = entities
        .spawn_node(
            EntityKind::Oil,
            ccx + 48.0 + config::TILE_SIZE as f32 * 2.0,
            ccy,
        )
        .expect("nearby oil node should spawn");
    let (nx, ny) = entities
        .get(occupied)
        .map(|n| (n.pos_x, n.pos_y))
        .expect("node pos");
    let holder = entities
        .spawn_unit(1, EntityKind::Worker, nx, ny)
        .expect("slot holder should spawn");
    {
        let h = entities.get_mut(holder).expect("holder should exist");
        h.set_order(Order::gather(occupied));
        h.mark_gather_phase(GatherPhase::Harvesting);
    }
    assert!(entities.claim_miner(occupied, holder));
    let bouncer = entities
        .spawn_unit(1, EntityKind::Worker, nx, ny)
        .expect("bouncing worker should spawn");
    {
        let w = entities.get_mut(bouncer).expect("worker should exist");
        w.set_order(Order::gather(occupied));
        w.set_target_id(Some(occupied));
    }

    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(1024, 32);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    let mut players = Vec::new();

    gather_system(
        &map,
        &mut entities,
        &mut players,
        &occ,
        &mut coordinator,
        &team_relations(&[]),
        1,
    );

    let w = entities.get(bouncer).expect("worker should exist");
    assert_eq!(w.order().gather_node(), Some(nearby));
    assert_eq!(w.target_id(), Some(nearby));
    assert_ne!(w.order().gather_node(), Some(oil));
    assert_ne!(w.order().gather_node(), Some(farther));
}

#[test]
fn occupied_resource_without_free_neighbor_moves_worker_to_open_grass_and_stops_queue() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (ccx, ccy) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, ccx, ccy, true)
        .expect("city centre should spawn");
    let occupied = entities
        .spawn_node(EntityKind::Steel, ccx + 48.0, ccy)
        .expect("occupied steel node should spawn");
    let (nx, ny) = entities
        .get(occupied)
        .map(|n| (n.pos_x, n.pos_y))
        .expect("node pos");
    let holder = entities
        .spawn_unit(1, EntityKind::Worker, nx, ny)
        .expect("slot holder should spawn");
    {
        let h = entities.get_mut(holder).expect("holder should exist");
        h.set_order(Order::gather(occupied));
        h.mark_gather_phase(GatherPhase::Harvesting);
    }
    assert!(entities.claim_miner(occupied, holder));
    let bouncer = entities
        .spawn_unit(1, EntityKind::Worker, nx, ny)
        .expect("bouncing worker should spawn");
    {
        let w = entities.get_mut(bouncer).expect("worker should exist");
        w.set_order(Order::gather(occupied));
        w.set_target_id(Some(occupied));
        w.append_queued_order(OrderIntent::move_to(nx + 96.0, ny));
    }

    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(1024, 32);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    let mut players = Vec::new();

    gather_system(
        &map,
        &mut entities,
        &mut players,
        &occ,
        &mut coordinator,
        &team_relations(&[]),
        1,
    );

    let w = entities.get(bouncer).expect("worker should exist");
    assert!(
        matches!(w.order(), Order::Move(_)),
        "worker should move off the full resource line when no free same-resource node exists"
    );
    assert!(
        w.queued_orders().is_empty(),
        "fallback move is terminal so the worker stops after moving to open grass"
    );
    let goal = w.path_goal().expect("fallback move should have a goal");
    let goal_tile = map.tile_of(goal.0, goal.1);
    assert_eq!(map.terrain_at(goal_tile.0, goal_tile.1), terrain::GRASS);
    assert_ne!(goal_tile, map.tile_of(nx, ny));
}
