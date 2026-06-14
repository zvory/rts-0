use crate::config;
use crate::game::entity::{EntityKind, EntityStore, GatherPhase};
use crate::game::map::Map;
use crate::game::services::dist2;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::world_query;
use crate::game::PlayerState;

/// Worker harvest loop: walk to node -> latch onto one free patch -> mine in place.
/// Depletes the node; when empty, the worker goes idle.
pub(crate) fn gather_system(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
    _spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
) {
    for id in entities.ids() {
        let node = match entities.get(id) {
            Some(e) if e.hp > 0 && e.kind.is_worker() => match e.order().gather_node() {
                Some(node) => node,
                None => continue,
            },
            _ => continue,
        };

        let phase = entities
            .get(id)
            .and_then(|e| e.gather_phase())
            .unwrap_or(GatherPhase::ToNode);
        match phase {
            GatherPhase::ToNode | GatherPhase::ToHome => {
                gather_to_node(map, entities, occ, coordinator, id, node)
            }
            GatherPhase::Harvesting => {
                gather_harvesting(map, entities, players, coordinator, id, node)
            }
        }
    }
}

fn gather_to_node(
    _map: &Map,
    entities: &mut EntityStore,
    _occ: &Occupancy,
    coordinator: &mut MoveCoordinator<'_>,
    id: u32,
    node: u32,
) {
    let node_pos = match entities.get(node) {
        Some(n) if n.is_node() && n.remaining().unwrap_or(0) > 0 => (n.pos_x, n.pos_y),
        _ => {
            idle_gatherer(entities, id);
            return;
        }
    };
    let (wx, wy) = match entities.get(id) {
        Some(e) => (e.pos_x, e.pos_y),
        None => return,
    };
    let owner = match entities.get(id) {
        Some(e) => e.owner,
        None => return,
    };

    if !world_query::resource_has_completed_mining_cc(entities, owner, node) {
        idle_gatherer(entities, id);
        return;
    }

    let interact = match (entities.get(id), entities.get(node)) {
        (Some(worker), Some(node)) => {
            worker.radius() + node.radius() + config::TILE_SIZE as f32 * 0.1
        }
        _ => return,
    };

    if dist2(wx, wy, node_pos.0, node_pos.1).sqrt() <= interact {
        let can_mine = !matches!(entities.node_slot_holder(node), Some(m) if m != id);
        if let Some(e) = entities.get_mut(id) {
            e.clear_path();
            e.set_facing((node_pos.1 - wy).atan2(node_pos.0 - wx));
            if can_mine {
                // Gather is terminal once harvesting begins: drop any later queued
                // handoff orders so the worker stays on the node.
                e.clear_queued_orders();
                e.mark_gather_phase(GatherPhase::Harvesting);
            } else {
                e.clear_active_order();
            }
        }
        if can_mine && !entities.claim_miner(node, id) {
            idle_gatherer(entities, id);
        }
    } else if entities.get(id).map(|e| e.path_is_empty()).unwrap_or(true) {
        coordinator.request_gather_path(entities, id, (node_pos.0, node_pos.1));
    }
}

fn gather_harvesting(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    coordinator: &mut MoveCoordinator<'_>,
    id: u32,
    node: u32,
) {
    let owner = match entities.get(id) {
        Some(e) => e.owner,
        None => return,
    };
    if !world_query::resource_has_completed_mining_cc(entities, owner, node) {
        scatter_gatherer_from_node(map, entities, coordinator, id, node);
        return;
    }

    let node_kind_amount = match entities.get(node) {
        Some(n) if n.is_node() && n.remaining().unwrap_or(0) > 0 => {
            (n.kind, n.remaining().unwrap_or(0))
        }
        _ => {
            idle_gatherer(entities, id);
            return;
        }
    };

    match entities.node_slot_holder(node) {
        Some(m) if m != id => {
            if let Some(e) = entities.get_mut(id) {
                e.clear_active_order();
            }
            return;
        }
        _ => {}
    }
    if !entities.claim_miner(node, id) {
        idle_gatherer(entities, id);
        return;
    }

    let done = {
        let e = match entities.get_mut(id) {
            Some(e) => e,
            None => return,
        };
        e.tick_gather_harvest()
            .map(|progress| progress >= config::HARVEST_TICKS)
            .unwrap_or(false)
    };
    if !done {
        return;
    }

    let load_cap = if node_kind_amount.0 == EntityKind::Oil {
        config::OIL_LOAD
    } else {
        config::STEEL_LOAD
    };
    let taken = entities
        .get_mut(node)
        .map(|n| n.harvest_resources(load_cap))
        .unwrap_or(0);

    if taken > 0 {
        if let Some(ps) = players.iter_mut().find(|p| p.id == owner) {
            ps.add_gathered_resources(node_kind_amount.0, taken);
        }
    }

    if let Some(e) = entities.get_mut(id) {
        e.clear_worker_carry();
        e.reset_gather_harvest();
        e.clear_path();
    }
    if taken == node_kind_amount.1 {
        entities.release_miner(id);
        idle_gatherer(entities, id);
    }
}

fn idle_gatherer(entities: &mut EntityStore, id: u32) {
    entities.release_miner(id);
    if let Some(e) = entities.get_mut(id) {
        e.clear_active_order();
    }
}

fn scatter_gatherer_from_node(
    map: &Map,
    entities: &mut EntityStore,
    coordinator: &mut MoveCoordinator<'_>,
    id: u32,
    node: u32,
) {
    let Some((owner, wx, wy)) = entities.get(id).map(|e| (e.owner, e.pos_x, e.pos_y)) else {
        return;
    };
    let Some((nx, ny)) = entities.get(node).map(|e| (e.pos_x, e.pos_y)) else {
        idle_gatherer(entities, id);
        return;
    };

    let dx = wx - nx;
    let dy = wy - ny;
    let len = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = if len > 0.001 {
        (dx / len, dy / len)
    } else {
        let angle = (id as f32 * 2.399_963_1).rem_euclid(std::f32::consts::TAU);
        (angle.cos(), angle.sin())
    };
    let step = config::TILE_SIZE as f32;
    let max = (map.world_size_px() - 1.0).max(0.0);
    let goal = (
        (wx + ux * step).clamp(0.0, max),
        (wy + uy * step).clamp(0.0, max),
    );

    coordinator.order_group_move(entities, owner, &[id], goal, false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore, GatherPhase, Order, OrderIntent};
    use crate::game::map::Map;
    use crate::game::services::move_coordinator::MoveCoordinator;
    use crate::game::services::occupancy::{footprint_center, Occupancy};
    use crate::game::services::pathing::PathingService;
    use crate::game::services::spatial::SpatialIndex;
    use crate::game::ScoreState;
    use crate::protocol::terrain;

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![(4, 4)],
            expansion_sites: Vec::new(),
        }
    }

    fn player_state(id: u32) -> PlayerState {
        PlayerState {
            id,
            team_id: id,
            faction_id: "kriegsia".to_string(),
            name: format!("Player {id}"),
            color: "#fff".to_string(),
            start_tile: (0, 0),
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 20,
            is_ai: false,
            score: ScoreState::default(),
            upgrades: Default::default(),
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
        let mut players = vec![player_state(1)];

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
}
