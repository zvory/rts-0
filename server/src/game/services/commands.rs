use std::collections::{HashMap, HashSet};

use crate::config;
use crate::game::entity::{EntityKind, EntityStore, GatherPhase, ProdItem};
use crate::game::map::Map;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::{footprint_center, footprint_placeable, Occupancy};
use crate::game::services::spatial::SpatialIndex;
use crate::game::PlayerState;
use crate::protocol::{Command, Event};

/// Max unique unit ids honored per multi-unit command. Caps the per-id work a single command can
/// force, so a repeated/huge id list can't be used to stall the tick loop.
const MAX_UNITS_PER_COMMAND: usize = 256;

/// Drain + apply queued commands (validate ownership / cost / supply / tech / placement).
pub(crate) fn apply_commands(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    pending: Vec<(u32, Command)>,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    // Tiles reserved by build commands already applied this tick. Prevents two commands
    // in the same tick from placing buildings on the same footprint before the spatial index
    // is rebuilt.
    let mut reserved_tiles: HashSet<(u32, u32)> = HashSet::new();

    for (player, cmd) in pending {
        match cmd {
            Command::Move { units, x, y } => {
                let valid: Vec<u32> = dedupe_cap_units(units)
                    .into_iter()
                    .filter(|id| owns_unit(entities, player, *id))
                    .collect();
                coordinator.order_group_move(entities, player, &valid, (x, y), false);
            }
            Command::AttackMove { units, x, y } => {
                let valid: Vec<u32> = dedupe_cap_units(units)
                    .into_iter()
                    .filter(|id| owns_unit(entities, player, *id))
                    .collect();
                coordinator.order_group_move(entities, player, &valid, (x, y), true);
            }
            Command::Attack { units, target } => {
                for id in dedupe_cap_units(units) {
                    if let Some(e) = entities.get(id) {
                        if !e.is_unit() || e.owner != player {
                            continue;
                        }
                    } else {
                        continue;
                    }
                    let target_ok = matches!(entities.get(target),
                        Some(t) if t.is_targetable() && t.id != id && t.owner != player);
                    if !target_ok {
                        continue;
                    }
                    coordinator.order_attack(entities, id, target);
                }
            }
            Command::Gather { units, node } => {
                for id in dedupe_cap_units(units) {
                    if !owns_unit(entities, player, id) {
                        continue;
                    }
                    let is_worker = matches!(entities.get(id), Some(e) if e.kind == EntityKind::Worker);
                    let node_ok = matches!(entities.get(node), Some(n)
                        if n.is_node() && n.remaining().unwrap_or(0) > 0);
                    if !is_worker || !node_ok {
                        continue;
                    }
                    if matches!(gather_slot_holder(entities, node), Some(holder) if holder != id) {
                        continue;
                    }
                    coordinator.order_gather(entities, id, node);
                }
            }
            Command::Build {
                worker,
                building,
                tile_x,
                tile_y,
            } => {
                order_build(
                    map,
                    entities,
                    players,
                    occ,
                    spatial,
                    coordinator,
                    player,
                    worker,
                    &building,
                    tile_x,
                    tile_y,
                    events,
                    &mut reserved_tiles,
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
                        entities.release_miner(id);
                        if let Some(e) = entities.get_mut(id) {
                            e.clear_orders();
                            if let Some(w) = e.worker.as_mut() {
                                w.carry = None; // stop returns carried load is dropped intentionally
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Dedupe a command's unit ids (preserving first-seen order) and cap the count at
/// `MAX_UNITS_PER_COMMAND`.
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

/// Issue a build order. Deducts cost immediately, places the building in CONSTRUCT state, and
/// sends the worker to the site.
#[allow(clippy::too_many_arguments)]
fn order_build(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    _occ: &Occupancy,
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    player: u32,
    worker: u32,
    building: &str,
    tile_x: u32,
    tile_y: u32,
    events: &mut HashMap<u32, Vec<Event>>,
    reserved_tiles: &mut HashSet<(u32, u32)>,
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

    // Tech requirement.
    let owned: Vec<EntityKind> = entities
        .iter()
        .filter(|e| e.owner == player && e.is_building())
        .map(|e| e.kind)
        .collect();
    if !config::build_requirement_met(kind, &owned) {
        notice(events, player, "Requirement not met");
        return;
    }

    // Reject clearly out-of-range top-left coords.
    if tile_x >= map.size || tile_y >= map.size {
        notice(events, player, "Cannot build there");
        return;
    }

    // Placement: footprint in bounds, on passable terrain, and not overlapping a building.
    if !footprint_placeable(map, entities, spatial, kind, tile_x, tile_y) {
        notice(events, player, "Cannot build there");
        return;
    }

    // Also reject footprints already reserved by another build command this tick.
    let tiles = crate::game::services::occupancy::footprint_tiles(kind, tile_x, tile_y);
    for t in &tiles {
        if reserved_tiles.contains(t) {
            notice(events, player, "Cannot build there");
            return;
        }
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
    for t in tiles {
        reserved_tiles.insert(t);
    }

    // Walk the worker to the site.
    coordinator.order_build(entities, worker, site, (tile_x, tile_y));
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
    let ok = matches!(entities.get(building), Some(b)
        if b.owner == player && b.is_building() && !b.under_construction()
        && config::trainable_units(b.kind).contains(&kind));
    if !ok {
        notice(events, player, "Cannot train that here");
        return;
    }
    let owned_complete: Vec<EntityKind> = entities
        .iter()
        .filter(|e| e.owner == player && e.is_building() && !e.under_construction())
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
    ps.steel -= stats.cost_steel;
    ps.oil -= stats.cost_oil;
    ps.supply_used += stats.supply;

    if let Some(b) = entities.get_mut(building) {
        if let Some(queue) = b.prod_queue_mut() {
            queue.push(ProdItem {
                unit: kind,
                progress: 0,
                total: stats.build_ticks,
            });
        }
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
            Some(b) if b.owner == player && b.is_building() && !b.prod_queue().is_empty() => b,
            _ => return,
        };
        match b.prod_queue_mut() {
            Some(queue) => queue.remove(0).unit,
            None => return,
        }
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

/// Resolve who, if anyone, currently holds `node`'s single harvest slot.
fn gather_slot_holder(entities: &EntityStore, node: u32) -> Option<u32> {
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
