use crate::config;
use crate::game::entity::{EntityKind, EntityStore, GatherPhase};
use crate::game::map::Map;
use crate::game::services::dist2;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::world_query;
use crate::game::teams::TeamRelations;
use crate::game::PlayerState;

mod pump_jack;

const SCATTER_RESOURCE_RANGE_TILES: f32 = 10.0;

/// Gatherer harvest loop: walk to node -> latch onto one free patch -> mine in place.
/// Depletes the node; when empty, the gatherer goes idle.
pub(crate) fn gather_system(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
    _spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    teams: &TeamRelations,
    tick: u32,
) {
    for id in entities.ids() {
        let node = match entities.get(id) {
            Some(e) if e.hp > 0 && is_gatherer_kind(e.kind) => match e.order().gather_node() {
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
                gather_harvesting(map, entities, players, coordinator, id, node, tick)
            }
        }
    }
    for payout in pump_jack::tick(entities, teams) {
        if let Some(ps) = players.iter_mut().find(|p| p.id == payout.owner) {
            ps.add_gathered_resources(EntityKind::Oil, payout.oil, tick);
        }
    }
}

fn gather_to_node(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    coordinator: &mut MoveCoordinator<'_>,
    id: u32,
    node: u32,
) {
    let node_pos = match entities.get(node) {
        Some(n) if direct_gather_node_mineable(n.kind) && n.remaining().unwrap_or(0) > 0 => {
            (n.pos_x, n.pos_y)
        }
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
                // handoff orders so the gatherer stays on the node.
                e.clear_queued_orders();
                e.mark_gather_phase(GatherPhase::Harvesting);
            }
        }
        if !can_mine {
            redirect_gatherer_from_occupied_node(map, entities, occ, coordinator, id, node);
            return;
        }
        if can_mine && !entities.claim_miner(node, id) {
            idle_gatherer(entities, id);
        }
    } else if entities.get(id).map(|e| e.path_is_empty()).unwrap_or(true) {
        coordinator.request_gather_path(entities, id, (node_pos.0, node_pos.1));
    }
}

fn redirect_gatherer_from_occupied_node(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    coordinator: &mut MoveCoordinator<'_>,
    id: u32,
    node: u32,
) {
    entities.release_miner(id);
    let Some(next_node) = closest_unoccupied_same_resource_node(entities, id, node) else {
        move_gatherer_to_nearby_open_grass(map, entities, occ, coordinator, id, node);
        return;
    };
    coordinator.order_gather(entities, id, next_node);
}

fn closest_unoccupied_same_resource_node(
    entities: &EntityStore,
    worker: u32,
    node: u32,
) -> Option<u32> {
    let (owner, target_kind, nx, ny) = match (entities.get(worker), entities.get(node)) {
        (Some(worker), Some(target)) if target.is_node() => {
            (worker.owner, target.kind, target.pos_x, target.pos_y)
        }
        _ => return None,
    };
    let range = SCATTER_RESOURCE_RANGE_TILES * config::TILE_SIZE as f32;
    let range2 = range * range + 0.01;

    let mut candidates: Vec<(u32, f32)> = entities
        .iter()
        .filter(|candidate| {
            candidate.id != node
                && candidate.kind == target_kind
                && direct_gather_node_mineable(candidate.kind)
                && candidate.remaining().unwrap_or(0) > 0
                && world_query::resource_has_completed_mining_cc(entities, owner, candidate.id)
                && entities.node_slot_holder(candidate.id).is_none()
        })
        .filter_map(|candidate| {
            let d2 = dist2(nx, ny, candidate.pos_x, candidate.pos_y);
            (d2 <= range2).then_some((candidate.id, d2))
        })
        .collect();
    candidates
        .sort_by(|(a_id, a_d2), (b_id, b_d2)| a_d2.total_cmp(b_d2).then_with(|| a_id.cmp(b_id)));
    candidates.first().map(|(id, _)| *id)
}

fn move_gatherer_to_nearby_open_grass(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    coordinator: &mut MoveCoordinator<'_>,
    id: u32,
    node: u32,
) {
    let Some((owner, wx, wy)) = entities.get(id).map(|e| (e.owner, e.pos_x, e.pos_y)) else {
        return;
    };
    let anchor = entities
        .get(node)
        .map(|e| (e.pos_x, e.pos_y))
        .unwrap_or((wx, wy));
    let Some(goal) = nearest_open_non_resource_passable_tile(map, entities, occ, anchor) else {
        idle_gatherer(entities, id);
        return;
    };
    if let Some(e) = entities.get_mut(id) {
        e.clear_queued_orders();
    }
    coordinator.order_group_move(entities, owner, &[id], goal, false);
}

fn nearest_open_non_resource_passable_tile(
    map: &Map,
    entities: &EntityStore,
    occ: &Occupancy,
    anchor: (f32, f32),
) -> Option<(f32, f32)> {
    let (ax, ay) = map.tile_of(anchor.0, anchor.1);
    let max_radius = SCATTER_RESOURCE_RANGE_TILES as i32;
    let resource_tiles: Vec<(u32, u32)> = entities
        .iter()
        .filter(|e| e.is_node() && e.remaining().unwrap_or(0) > 0)
        .map(|e| map.tile_of(e.pos_x, e.pos_y))
        .collect();

    let mut best: Option<((f32, f32), i32, f32)> = None;
    for radius in 1..=max_radius {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let tx = ax as i32 + dx;
                let ty = ay as i32 + dy;
                if !map.in_bounds(tx, ty)
                    || !map.is_passable(tx, ty)
                    || occ.building_blocked_at_tile(tx, ty)
                    || resource_tiles.contains(&(tx as u32, ty as u32))
                {
                    continue;
                }
                let goal = map.tile_center(tx as u32, ty as u32);
                let d2 = dist2(anchor.0, anchor.1, goal.0, goal.1);
                let replace = best
                    .as_ref()
                    .map(|(_, best_radius, best_d2)| {
                        radius < *best_radius || (radius == *best_radius && d2 < *best_d2)
                    })
                    .unwrap_or(true);
                if replace {
                    best = Some((goal, radius, d2));
                }
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map(|(goal, _, _)| goal)
}

fn gather_harvesting(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    coordinator: &mut MoveCoordinator<'_>,
    id: u32,
    node: u32,
    tick: u32,
) {
    let (owner, gatherer_kind) = match entities.get(id) {
        Some(e) => (e.owner, e.kind),
        None => return,
    };
    if !world_query::resource_has_completed_mining_cc(entities, owner, node) {
        scatter_gatherer_from_node(map, entities, coordinator, id, node);
        return;
    }

    let node_kind_amount = match entities.get(node) {
        Some(n) if direct_gather_node_mineable(n.kind) && n.remaining().unwrap_or(0) > 0 => {
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

    let load_cap = gather_load_cap(gatherer_kind, node_kind_amount.0);
    let taken = entities
        .get_mut(node)
        .map(|n| n.harvest_resources(load_cap))
        .unwrap_or(0);

    if taken > 0 {
        if let Some(ps) = players.iter_mut().find(|p| p.id == owner) {
            ps.add_gathered_resources(node_kind_amount.0, taken, tick);
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

fn is_gatherer_kind(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::Worker | EntityKind::Golem)
}

fn direct_gather_node_mineable(kind: EntityKind) -> bool {
    kind.is_node() && kind != EntityKind::Oil
}

fn gather_load_cap(gatherer_kind: EntityKind, node_kind: EntityKind) -> u32 {
    let base = if node_kind == EntityKind::Oil {
        config::OIL_LOAD
    } else {
        config::STEEL_LOAD
    };
    if gatherer_kind == EntityKind::Golem {
        base.saturating_mul(4)
    } else {
        base
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
mod tests;
