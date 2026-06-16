use super::*;
use crate::game::entity::{EntityKind, EntityStore, GatherPhase, Order, OrderIntent};
use crate::game::map::Map;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::{footprint_center, Occupancy};
use crate::game::services::pathing::PathingService;
use crate::game::services::spatial::SpatialIndex;
use crate::protocol::terrain;

fn flat_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(4, 4)],
        expansion_sites: Vec::new(),
    }
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
    let spatial = SpatialIndex::build(&entities, map.size);
    let mut pathing = PathingService::new(1024, 32);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    let mut players = Vec::new();

    gather_system(
        &map,
        &mut entities,
        &mut players,
        &occ,
        &spatial,
        &mut coordinator,
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
    let spatial = SpatialIndex::build(&entities, map.size);
    let mut pathing = PathingService::new(1024, 32);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    let mut players = Vec::new();

    gather_system(
        &map,
        &mut entities,
        &mut players,
        &occ,
        &spatial,
        &mut coordinator,
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
    let spatial = SpatialIndex::build(&entities, map.size);
    let mut pathing = PathingService::new(1024, 32);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    let mut players = Vec::new();

    gather_system(
        &map,
        &mut entities,
        &mut players,
        &occ,
        &spatial,
        &mut coordinator,
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
