use crate::config;
use crate::game::entity::{EntityKind, EntityStore, GatherPhase};
use crate::game::map::Map;
use crate::game::services::dist2;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
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
    let interact = config::TILE_SIZE as f32 * 1.5;

    for id in entities.ids() {
        let node = match entities.get(id) {
            Some(e) if e.hp > 0 && e.kind == EntityKind::Worker => match e.order().gather_node() {
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
                gather_to_node(map, entities, occ, coordinator, id, node, interact)
            }
            GatherPhase::Harvesting => gather_harvesting(entities, players, id, node),
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
    interact: f32,
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

    if dist2(wx, wy, node_pos.0, node_pos.1).sqrt() <= interact {
        let can_mine = !matches!(slot_held(entities, node), Some(m) if m != id);
        if let Some(e) = entities.get_mut(id) {
            e.clear_path();
            e.set_facing((node_pos.1 - wy).atan2(node_pos.0 - wx));
            if can_mine {
                e.mark_gather_phase(GatherPhase::Harvesting);
            } else {
                e.clear_orders();
            }
        }
        if can_mine {
            if let Some(n) = entities.get_mut(node) {
                if let Some(node) = n.resource_node.as_mut() {
                    node.miner = Some(id);
                }
            }
        }
    } else if entities.get(id).map(|e| e.path_is_empty()).unwrap_or(true) {
        coordinator.request_gather_path(entities, id, (node_pos.0, node_pos.1));
    }
}

fn gather_harvesting(entities: &mut EntityStore, players: &mut [PlayerState], id: u32, node: u32) {
    let node_kind_amount = match entities.get(node) {
        Some(n) if n.is_node() && n.remaining().unwrap_or(0) > 0 => {
            (n.kind, n.remaining().unwrap_or(0))
        }
        _ => {
            idle_gatherer(entities, id);
            return;
        }
    };

    match slot_held(entities, node) {
        Some(m) if m != id => {
            if let Some(e) = entities.get_mut(id) {
                e.clear_orders();
            }
            return;
        }
        _ => {
            if let Some(n) = entities.get_mut(node) {
                if let Some(node) = n.resource_node.as_mut() {
                    node.miner = Some(id);
                }
            }
        }
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

    let is_oil = node_kind_amount.0 == EntityKind::Oil;
    let load_cap = if is_oil {
        config::OIL_LOAD
    } else {
        config::STEEL_LOAD
    };
    let taken = load_cap.min(node_kind_amount.1);
    if let Some(n) = entities.get_mut(node) {
        if let Some(node) = n.resource_node.as_mut() {
            node.remaining = node.remaining.saturating_sub(taken);
        }
    }

    let owner = match entities.get(id) {
        Some(e) => e.owner,
        None => return,
    };
    if taken > 0 {
        if let Some(ps) = players.iter_mut().find(|p| p.id == owner) {
            if is_oil {
                ps.oil += taken;
            } else {
                ps.steel += taken;
            }
        }
    }

    if let Some(e) = entities.get_mut(id) {
        if let Some(w) = e.worker.as_mut() {
            w.carry = None;
        }
        e.reset_gather_harvest();
        e.clear_path();
    }
    if taken == node_kind_amount.1 {
        entities.release_miner(id);
        idle_gatherer(entities, id);
    }
}

/// Resolve who, if anyone, currently holds `node`'s single harvest slot.
///
/// A patch is occupied only after the worker has actually latched, not merely because a gather
/// command has been issued toward it.
fn slot_held(entities: &EntityStore, node: u32) -> Option<u32> {
    let m = entities.get(node).and_then(|n| n.miner())?;
    let w = entities.get(m)?;
    let on_this_node = w.order().gather_node() == Some(node);
    if w.hp > 0
        && w.kind == EntityKind::Worker
        && on_this_node
        && w.gather_phase() == Some(GatherPhase::Harvesting)
    {
        Some(m)
    } else {
        None
    }
}

fn idle_gatherer(entities: &mut EntityStore, id: u32) {
    entities.release_miner(id);
    if let Some(e) = entities.get_mut(id) {
        e.clear_orders();
    }
}
