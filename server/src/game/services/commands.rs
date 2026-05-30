use std::collections::{HashMap, HashSet};

use crate::config;
use crate::game::entity::{EntityKind, EntityStore, GatherPhase, Order, ProdItem};
use crate::game::map::Map;
use crate::game::pathfinding;
use crate::game::services::occupancy::{footprint_center, footprint_placeable, Occupancy};
use crate::game::services::spatial::SpatialIndex;
use crate::game::PlayerState;
use crate::protocol::{Command, Event};

/// Max unique unit ids honored per multi-unit command. Caps the per-id work (one A* each) a
/// single command can force, so a repeated/huge id list can't be used to stall the tick loop.
const MAX_UNITS_PER_COMMAND: usize = 256;

/// Drain + apply queued commands (validate ownership / cost / supply / tech / placement).
pub(crate) fn apply_commands(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
    spatial: &SpatialIndex,
    pending: Vec<(u32, Command)>,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    for (player, cmd) in pending {
        match cmd {
            Command::Move { units, x, y } => {
                for id in dedupe_cap_units(units) {
                    order_move(map, entities, occ, player, id, x, y, false);
                }
            }
            Command::AttackMove { units, x, y } => {
                for id in dedupe_cap_units(units) {
                    order_move(map, entities, occ, player, id, x, y, true);
                }
            }
            Command::Attack { units, target } => {
                for id in dedupe_cap_units(units) {
                    order_attack(map, entities, occ, player, id, target);
                }
            }
            Command::Gather { units, node } => {
                for id in dedupe_cap_units(units) {
                    order_gather(map, entities, occ, player, id, node);
                }
            }
            Command::Build {
                worker,
                building,
                tile_x,
                tile_y,
            } => {
                order_build(
                    map, entities, players, occ, spatial, player, worker, &building, tile_x, tile_y, events,
                );
            }
            Command::Train { building, unit } => {
                order_train(entities, players, player, building, &unit, events);
            }
            Command::Cancel { building } => {
                order_cancel(entities, players, player, building);
            }
            Command::Stop { units } => {
                for id in dedupe_cap_units(units) {
                    if owns_unit(entities, player, id) {
                        if let Some(e) = entities.get_mut(id) {
                            e.clear_orders();
                            e.carry = None; // stop returns carried load is dropped intentionally
                        }
                    }
                }
            }
        }
    }
}

/// Dedupe a command's unit ids (preserving first-seen order) and cap the count at
/// `MAX_UNITS_PER_COMMAND`. This stops a repeated or oversized id list from forcing one A* path
/// per duplicate and stalling the tick loop.
fn dedupe_cap_units(units: Vec<u32>) -> Vec<u32> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(units.len().min(MAX_UNITS_PER_COMMAND));
    for id in units {
        if out.len() >= MAX_UNITS_PER_COMMAND {
            break;
        }
        if seen.insert(id) {
            out.push(id);
        }
    }
    out
}

/// Whether `player` owns a *unit* with this id (buildings/nodes excluded).
fn owns_unit(entities: &EntityStore, player: u32, id: u32) -> bool {
    matches!(entities.get(id), Some(e) if e.owner == player && e.is_unit())
}

/// Issue a move / attack-move order to a unit, computing an A* path to the goal tile.
#[allow(clippy::too_many_arguments)]
fn order_move(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    player: u32,
    id: u32,
    x: f32,
    y: f32,
    attack_move: bool,
) {
    if !owns_unit(entities, player, id) {
        return;
    }
    let (sx, sy) = {
        let e = match entities.get(id) {
            Some(e) => e,
            None => return,
        };
        map.tile_of(e.pos_x, e.pos_y)
    };
    let (gx, gy) = map.tile_of(x, y);
    let path = pathfinding::find_path(occ, sx as i32, sy as i32, gx as i32, gy as i32);
    let mut waypoints = pathfinding::to_world_waypoints(&path);
    // Waypoints are stored reversed (next = last element), so the goal is `waypoints[0]` — the
    // element popped last. Snap it to the exact requested point for precise arrival.
    if !waypoints.is_empty() {
        waypoints[0] = (x, y);
    }

    if let Some(e) = entities.get_mut(id) {
        e.path = waypoints;
        e.target_id = None;
        e.order = if attack_move {
            Order::AttackMove { x, y }
        } else {
            Order::Move { x, y }
        };
        // Leaving a gather order: reset gather sub-state.
        e.gather_phase = GatherPhase::ToNode;
        e.harvest_progress = 0;
    }
}

/// Issue an attack order against a specific target entity.
fn order_attack(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    player: u32,
    id: u32,
    target: u32,
) {
    if !owns_unit(entities, player, id) {
        return;
    }
    // Target must exist and be attackable; ignore self and same-owner entities (no friendly
    // fire — mirrors combat_system's `t_owner == owner` guard, so a unit can't be locked onto
    // an allied/own target).
    let target_ok = matches!(entities.get(target),
        Some(t) if t.is_targetable() && t.id != id && t.owner != player);
    if !target_ok {
        return;
    }
    let (tx, ty, sx, sy) = {
        let t = entities.get(target).unwrap();
        let e = entities.get(id).unwrap();
        let (tx, ty) = map.tile_of(t.pos_x, t.pos_y);
        let (sx, sy) = map.tile_of(e.pos_x, e.pos_y);
        (tx, ty, sx, sy)
    };
    let path = pathfinding::find_path(occ, sx as i32, sy as i32, tx as i32, ty as i32);
    let waypoints = pathfinding::to_world_waypoints(&path);
    if let Some(e) = entities.get_mut(id) {
        e.order = Order::Attack { target };
        e.target_id = Some(target);
        e.path = waypoints;
        e.gather_phase = GatherPhase::ToNode;
        e.harvest_progress = 0;
    }
}

/// Issue a gather order: only workers gather, only from resource nodes.
fn order_gather(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    player: u32,
    id: u32,
    node: u32,
) {
    if !owns_unit(entities, player, id) {
        return;
    }
    // Must be a worker, and the target must be a (non-empty) resource node.
    let is_worker = matches!(entities.get(id), Some(e) if e.kind == EntityKind::Worker);
    let node_ok = matches!(entities.get(node), Some(n) if n.is_node() && n.remaining > 0);
    if !is_worker || !node_ok {
        return;
    }
    let (nx, ny, sx, sy) = {
        let n = entities.get(node).unwrap();
        let e = entities.get(id).unwrap();
        let (nx, ny) = map.tile_of(n.pos_x, n.pos_y);
        let (sx, sy) = map.tile_of(e.pos_x, e.pos_y);
        (nx, ny, sx, sy)
    };
    let path = pathfinding::find_path(occ, sx as i32, sy as i32, nx as i32, ny as i32);
    let waypoints = pathfinding::to_world_waypoints(&path);
    if let Some(e) = entities.get_mut(id) {
        e.order = Order::Gather { node };
        e.target_id = Some(node);
        e.path = waypoints;
        // Resume sensibly: if already laden, head home; else go to the node.
        e.gather_phase = if e.carry.map(|c| c.amount > 0).unwrap_or(false) {
            GatherPhase::ToHome
        } else {
            GatherPhase::ToNode
        };
        e.harvest_progress = 0;
    }
}

/// Issue a build order. Deducts cost immediately, places the building in CONSTRUCT state, and
/// sends the worker to the site. Emits a `Notice` on any validation failure.
#[allow(clippy::too_many_arguments)]
fn order_build(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
    spatial: &SpatialIndex,
    player: u32,
    worker: u32,
    building: &str,
    tile_x: u32,
    tile_y: u32,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    if !owns_unit(entities, player, worker) {
        return;
    }
    if !matches!(entities.get(worker), Some(e) if e.kind == EntityKind::Worker) {
        notice(events, player, "Only workers can build");
        return;
    }
    let kind = match building.parse::<EntityKind>() {
        Ok(k) => k,
        Err(_) => {
            notice(events, player, "Unknown building");
            return;
        }
    };
    let stats = match config::building_stats(kind) {
        Some(s) => s,
        None => {
            notice(events, player, "Unknown building");
            return;
        }
    };

    // Tech requirement (e.g. barracks needs an Industrial Center).
    let owned: Vec<EntityKind> = entities
        .iter()
        .filter(|e| e.owner == player && e.is_building())
        .map(|e| e.kind)
        .collect();
    if !config::build_requirement_met(kind, &owned) {
        notice(events, player, "Requirement not met");
        return;
    }

    // Reject clearly out-of-range top-left coords before footprint work (also avoids any
    // coordinate overflow downstream).
    if tile_x >= map.size || tile_y >= map.size {
        notice(events, player, "Cannot build there");
        return;
    }

    // Placement: footprint in bounds, on passable terrain, and not overlapping a building.
    if !footprint_placeable(map, entities, spatial, kind, tile_x, tile_y) {
        notice(events, player, "Cannot build there");
        return;
    }

    // Cost.
    let ps = match players.iter_mut().find(|p| p.id == player) {
        Some(p) => p,
        None => return,
    };
    if ps.steel < stats.cost_steel || ps.oil < stats.cost_oil {
        notice(events, player, "Not enough resources");
        return;
    }
    ps.steel -= stats.cost_steel;
    ps.oil -= stats.cost_oil;

    // Spawn the building in CONSTRUCT state at the footprint center.
    let (cx, cy) = footprint_center(map, kind, tile_x, tile_y);
    let site = match entities.spawn_building(player, kind, cx, cy, false) {
        Some(id) => id,
        None => return,
    };

    // Walk the worker to the site and mark it occupied with a build order.
    let (sx, sy) = {
        let e = entities.get(worker).unwrap();
        map.tile_of(e.pos_x, e.pos_y)
    };
    let path = pathfinding::find_path(occ, sx as i32, sy as i32, tile_x as i32, tile_y as i32);
    let waypoints = pathfinding::to_world_waypoints(&path);
    if let Some(e) = entities.get_mut(worker) {
        e.order = Order::Build { site };
        e.target_id = Some(site);
        e.path = waypoints;
        e.gather_phase = GatherPhase::ToNode;
        e.harvest_progress = 0;
    }
}

/// Queue a unit at a production building. Reserves cost + supply on enqueue.
fn order_train(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    player: u32,
    building: u32,
    unit: &str,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    let kind = match unit.parse::<EntityKind>() {
        Ok(k) => k,
        Err(_) => {
            notice(events, player, "Unknown unit");
            return;
        }
    };
    // Building must be owned, finished, and able to train this unit.
    let ok = matches!(entities.get(building), Some(b)
        if b.owner == player && b.is_building() && !b.under_construction
        && config::trainable_units(b.kind).contains(&kind));
    if !ok {
        notice(events, player, "Cannot train that here");
        return;
    }
    let owned_complete: Vec<EntityKind> = entities
        .iter()
        .filter(|e| e.owner == player && e.is_building() && !e.under_construction)
        .map(|e| e.kind)
        .collect();
    if !config::train_requirement_met(kind, &owned_complete) {
        notice(events, player, "Requirement not met");
        return;
    }
    let stats = match config::unit_stats(kind) {
        Some(s) => s,
        None => {
            notice(events, player, "Unknown unit");
            return;
        }
    };

    let ps = match players.iter_mut().find(|p| p.id == player) {
        Some(p) => p,
        None => return,
    };
    if ps.steel < stats.cost_steel || ps.oil < stats.cost_oil {
        notice(events, player, "Not enough resources");
        return;
    }
    if ps.supply_used + stats.supply > ps.supply_cap {
        notice(events, player, "Not enough supply");
        return;
    }
    // Reserve cost + supply now; refunded on cancel.
    ps.steel -= stats.cost_steel;
    ps.oil -= stats.cost_oil;
    ps.supply_used += stats.supply;

    if let Some(b) = entities.get_mut(building) {
        b.prod_queue.push(ProdItem {
            unit: kind,
            progress: 0,
            total: stats.build_ticks,
        });
    }
}

/// Cancel the front item of a building's production queue, refunding its cost + supply.
fn order_cancel(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    player: u32,
    building: u32,
) {
    let unit = {
        let b = match entities.get_mut(building) {
            Some(b) if b.owner == player && b.is_building() && !b.prod_queue.is_empty() => b,
            _ => return,
        };
        b.prod_queue.remove(0).unit
    };
    if let Some(stats) = config::unit_stats(unit) {
        if let Some(ps) = players.iter_mut().find(|p| p.id == player) {
            ps.steel += stats.cost_steel;
            ps.oil += stats.cost_oil;
            ps.supply_used = ps.supply_used.saturating_sub(stats.supply);
        }
    }
}

/// Push a best-effort `Notice` event to a player.
pub(crate) fn notice(events: &mut HashMap<u32, Vec<Event>>, player: u32, msg: &str) {
    events.entry(player).or_default().push(Event::Notice {
        msg: msg.to_string(),
    });
}
