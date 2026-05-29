use crate::config;
use crate::game::entity::{CarryState, EntityKind, EntityStore, GatherPhase, Order};
use crate::game::map::Map;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::pathing::PathingService;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::{dist2, interact_range};
use crate::game::PlayerState;

/// Worker harvest loop: walk to node → harvest → carry a load → return to the
/// nearest own Industrial Center → deposit → repeat. Depletes the node; when empty, retargets a nearby
/// same-kind node or goes idle.
pub(crate) fn gather_system(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
    spatial: &SpatialIndex,
    pathing: &mut PathingService,
) {
    let interact = config::TILE_SIZE as f32 * 1.5; // close enough to mine / deposit

    for id in entities.ids() {
        let node = match entities.get(id) {
            Some(e) if e.kind == EntityKind::Worker => match e.order {
                Order::Gather { node } => node,
                _ => continue,
            },
            _ => continue,
        };

        let phase = entities
            .get(id)
            .map(|e| e.gather_phase)
            .unwrap_or(GatherPhase::ToNode);
        match phase {
            GatherPhase::ToNode => gather_to_node(map, entities, occ, pathing, id, node, interact),
            GatherPhase::Harvesting => gather_harvesting(map, entities, occ, pathing, id, node, interact),
            GatherPhase::ToHome => gather_to_home(map, entities, players, occ, spatial, pathing, id, node, interact),
        }
    }
}

fn gather_to_node(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    pathing: &mut PathingService,
    id: u32,
    node: u32,
    interact: f32,
) {
    // Node still valid?
    let node_pos = match entities.get(node) {
        Some(n) if n.is_node() && n.remaining > 0 => (n.pos_x, n.pos_y),
        _ => {
            retarget_or_idle(map, entities, occ, pathing, id, node);
            return;
        }
    };
    let (wx, wy) = match entities.get(id) {
        Some(e) => (e.pos_x, e.pos_y),
        None => return,
    };
    if dist2(wx, wy, node_pos.0, node_pos.1).sqrt() <= interact {
        // Arrived. Only one worker may occupy a node's harvest slot at a time. Claim it if
        // free (or stale); otherwise queue in place — stop and face the node — until the
        // current miner releases it (deposits, dies, or is re-ordered).
        let can_mine = !matches!(slot_held(entities, node), Some(m) if m != id);
        if let Some(e) = entities.get_mut(id) {
            e.path.clear();
            e.facing = (node_pos.1 - wy).atan2(node_pos.0 - wx);
            if can_mine {
                e.gather_phase = GatherPhase::Harvesting;
                e.harvest_progress = 0;
            }
        }
        if can_mine {
            if let Some(n) = entities.get_mut(node) {
                n.miner = Some(id);
            }
        }
    } else if entities.get(id).map(|e| e.path.is_empty()).unwrap_or(true) {
        // Lost the path; recompute toward the node.
        pathing.repath_entity(map, entities, occ, id, node_pos.0, node_pos.1);
    }
}

fn gather_harvesting(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    pathing: &mut PathingService,
    id: u32,
    node: u32,
    _interact: f32,
) {
    // Node still valid?
    let node_kind_amount = match entities.get(node) {
        Some(n) if n.is_node() && n.remaining > 0 => (n.kind, n.remaining),
        _ => {
            retarget_or_idle(map, entities, occ, pathing, id, node);
            return;
        }
    };

    // Re-affirm sole ownership of the harvest slot. If another live worker holds it (e.g. a
    // race where two workers reached contact on the same tick), yield back to queuing; the
    // slot owner keeps mining. Otherwise (re)claim it so the reservation tracks us.
    match slot_held(entities, node) {
        Some(m) if m != id => {
            if let Some(e) = entities.get_mut(id) {
                e.gather_phase = GatherPhase::ToNode;
                e.harvest_progress = 0;
            }
            return;
        }
        _ => {
            if let Some(n) = entities.get_mut(node) {
                n.miner = Some(id);
            }
        }
    }

    let done = {
        let e = match entities.get_mut(id) {
            Some(e) => e,
            None => return,
        };
        e.harvest_progress += 1;
        e.harvest_progress >= config::HARVEST_TICKS
    };
    if !done {
        return;
    }

    // Extract a load (capped by remaining), deplete the node, then head home.
    let is_oil = node_kind_amount.0 == EntityKind::Oil;
    let load_cap = if is_oil {
        config::OIL_LOAD
    } else {
        config::STEEL_LOAD
    };
    let taken = load_cap.min(node_kind_amount.1);
    if let Some(n) = entities.get_mut(node) {
        n.remaining = n.remaining.saturating_sub(taken);
        // Release the harvest slot now that we're leaving to deposit, so a queued worker can
        // step in while we ferry the load home.
        if n.miner == Some(id) {
            n.miner = None;
        }
    }
    if let Some(e) = entities.get_mut(id) {
        e.carry = Some(CarryState {
            amount: taken,
            kind: if is_oil {
                EntityKind::Oil
            } else {
                EntityKind::Steel
            },
        });
        e.harvest_progress = 0;
        e.gather_phase = GatherPhase::ToHome;
    }
    // Route to the nearest own Industrial Center.
    route_home(map, entities, occ, pathing, id);
}

fn gather_to_home(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
    spatial: &SpatialIndex,
    pathing: &mut PathingService,
    id: u32,
    node: u32,
    interact: f32,
) {
    let (owner, wx, wy) = match entities.get(id) {
        Some(e) => (e.owner, e.pos_x, e.pos_y),
        None => return,
    };
    // Find nearest own, finished Industrial Center.
    let industrial_center = nearest_own_industrial_center(entities, spatial, owner, wx, wy);
    let Some((industrial_center_id, hx, hy)) = industrial_center else {
        // No Industrial Center to deposit into: hold the load and wait (idle path).
        if let Some(e) = entities.get_mut(id) {
            e.path.clear();
        }
        return;
    };

    // Deposit range accounts for the Industrial Center footprint (a 3×3 building's center is ~1.5 tiles from
    // its passable edge, which is as close as the worker can path).
    let deposit_range = interact_range(entities, industrial_center_id).unwrap_or(interact);
    if dist2(wx, wy, hx, hy).sqrt() <= deposit_range {
        // Deposit.
        let (amount, kind) = entities
            .get(id)
            .and_then(|e| e.carry)
            .map(|c| (c.amount, c.kind))
            .unwrap_or((0, EntityKind::Steel));
        if amount > 0 {
            if let Some(ps) = players.iter_mut().find(|p| p.id == owner) {
                if kind == EntityKind::Oil {
                    ps.oil += amount;
                } else {
                    ps.steel += amount;
                }
            }
        }
        if let Some(e) = entities.get_mut(id) {
            e.carry = None;
            e.home_industrial_center = Some(industrial_center_id);
            // Loop back to the node (or retarget if depleted).
            e.gather_phase = GatherPhase::ToNode;
            e.path.clear();
        }
        // Send back to the node now (handles depletion / retargeting).
        gather_to_node(map, entities, occ, pathing, id, node, interact);
    } else if entities.get(id).map(|e| e.path.is_empty()).unwrap_or(true) {
        pathing.repath_entity(map, entities, occ, id, hx, hy);
    }
}

/// Route a laden worker to its nearest own Industrial Center.
fn route_home(map: &Map, entities: &mut EntityStore, occ: &Occupancy, pathing: &mut PathingService, id: u32) {
    let (owner, wx, wy) = match entities.get(id) {
        Some(e) => (e.owner, e.pos_x, e.pos_y),
        None => return,
    };
    // We don't have the spatial index here (called from gather_harvesting), but the gather
    // system will repath in the ToHome phase next tick. For now, try to route via a large scan.
    if let Some((industrial_center_id, hx, hy)) =
        nearest_own_industrial_center_no_spatial(entities, owner, wx, wy)
    {
        if let Some(e) = entities.get_mut(id) {
            e.home_industrial_center = Some(industrial_center_id);
        }
        pathing.repath_entity(map, entities, occ, id, hx, hy);
    }
}

/// Resolve who, if anyone, currently holds `node`'s single harvest slot.
///
/// The node's `miner` field is advisory: it is only honored while the recorded worker is alive
/// and still actively [`GatherPhase::Harvesting`] this very node. A worker that died, was
/// re-ordered, retargeted, or walked off to deposit no longer holds the slot, so this returns
/// `None` and the slot is free for the next worker to claim. This makes the reservation
/// self-healing without needing an explicit release on every code path.
fn slot_held(entities: &EntityStore, node: u32) -> Option<u32> {
    let m = entities.get(node).and_then(|n| n.miner)?;
    let w = entities.get(m)?;
    let on_this_node = matches!(w.order, Order::Gather { node: n } if n == node);
    if w.hp > 0
        && w.kind == EntityKind::Worker
        && on_this_node
        && w.gather_phase == GatherPhase::Harvesting
    {
        Some(m)
    } else {
        None
    }
}

/// When a gather node is gone, try to find a nearby same-kind node; else go idle.
fn retarget_or_idle(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    pathing: &mut PathingService,
    id: u32,
    old_node: u32,
) {
    let (owner, wx, wy, want_oil) = {
        let e = match entities.get(id) {
            Some(e) => e,
            None => return,
        };
        let want_oil = matches!(entities.get(old_node), Some(n) if n.kind == EntityKind::Oil);
        (e.owner, e.pos_x, e.pos_y, want_oil)
    };
    let _ = owner;
    let want_kind = if want_oil {
        EntityKind::Oil
    } else {
        EntityKind::Steel
    };

    // Nearest same-kind, non-empty node within a reasonable radius.
    // We scan all nodes because the spatial index isn't passed here (this is called from
    // gather phases that don't have it readily available). In practice node counts are low.
    let mut best: Option<(u32, f32, f32, f32)> = None;
    for n in entities.iter() {
        if n.is_node() && n.remaining > 0 && n.kind == want_kind {
            let d = dist2(wx, wy, n.pos_x, n.pos_y);
            if best.map(|(_, _, _, bd)| d < bd).unwrap_or(true) {
                best = Some((n.id, n.pos_x, n.pos_y, d));
            }
        }
    }

    match best {
        Some((nid, nx, ny, _)) => {
            entities.release_miner(id);
            if let Some(e) = entities.get_mut(id) {
                e.order = Order::Gather { node: nid };
                e.target_id = Some(nid);
                e.gather_phase = GatherPhase::ToNode;
                e.harvest_progress = 0;
            }
            pathing.repath_entity(map, entities, occ, id, nx, ny);
        }
        None => {
            entities.release_miner(id);
            if let Some(e) = entities.get_mut(id) {
                e.clear_orders();
            }
        }
    }
}

/// Nearest finished Industrial Center owned by `owner` to a point, as `(id, x, y)`.
/// Uses the spatial index for an efficient range query.
pub(crate) fn nearest_own_industrial_center(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    owner: u32,
    x: f32,
    y: f32,
) -> Option<(u32, f32, f32)> {
    let max_radius = config::TILE_SIZE as f32 * 64.0; // generous max search radius
    let result = spatial.nearest(
        x,
        y,
        max_radius,
        entities,
        |e: &crate::game::entity::Entity| {
            e.owner == owner && e.kind == EntityKind::IndustrialCenter && !e.under_construction
        },
    );
    result.and_then(|(id, _)| entities.get(id).map(|e| (id, e.pos_x, e.pos_y)))
}

/// Fallback that scans all entities (used when the spatial index isn't available, e.g. during
/// internal routing inside the gather phase).
fn nearest_own_industrial_center_no_spatial(
    entities: &EntityStore,
    owner: u32,
    x: f32,
    y: f32,
) -> Option<(u32, f32, f32)> {
    let mut best: Option<(u32, f32, f32, f32)> = None;
    for e in entities.iter() {
        if e.owner == owner && e.kind == EntityKind::IndustrialCenter && !e.under_construction {
            let d = dist2(x, y, e.pos_x, e.pos_y);
            if best.map(|(_, _, _, bd)| d < bd).unwrap_or(true) {
                best = Some((e.id, e.pos_x, e.pos_y, d));
            }
        }
    }
    best.map(|(id, hx, hy, _)| (id, hx, hy))
}
