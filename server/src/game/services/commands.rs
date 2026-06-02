use std::collections::{HashMap, HashSet};

use crate::config;
use crate::game::entity::{BuildPhase, EntityKind, EntityStore, ProdItem};
use crate::game::map::Map;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::footprint_placeable;
use crate::game::services::occupancy::footprint_tiles;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::world_query;
use crate::game::PlayerState;
use crate::protocol::{Command, Event};
use crate::rules;

/// Max unique unit ids honored per multi-unit command. Caps the per-id work a single command can
/// force, so a repeated/huge id list can't be used to stall the tick loop.
const MAX_UNITS_PER_COMMAND: usize = 256;

/// Drain + apply queued commands (validate ownership / cost / supply / tech / placement).
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_commands(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    pending: Vec<(u32, Command)>,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    for (player, cmd) in pending {
        match cmd {
            Command::Move { units, x, y } => {
                let valid: Vec<u32> = dedupe_cap_units(units)
                    .into_iter()
                    .filter(|id| {
                        owns_unit(entities, player, *id) && !is_constructing(entities, *id)
                    })
                    .collect();
                coordinator.order_group_move(entities, player, &valid, (x, y), false);
            }
            Command::AttackMove { units, x, y } => {
                let valid: Vec<u32> = dedupe_cap_units(units)
                    .into_iter()
                    .filter(|id| {
                        owns_unit(entities, player, *id) && !is_constructing(entities, *id)
                    })
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
                    if is_constructing(entities, id) {
                        continue;
                    }
                    let target_ok = matches!(entities.get(target),
                        Some(t) if world_query::is_enemy_targetable(t, player, id));
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
                    if is_constructing(entities, id) {
                        continue;
                    }
                    let is_worker =
                        matches!(entities.get(id), Some(e) if e.kind == EntityKind::Worker);
                    let node_ok = matches!(entities.get(node), Some(n)
                        if n.is_node() && n.remaining().unwrap_or(0) > 0);
                    if !is_worker || !node_ok {
                        continue;
                    }
                    if matches!(world_query::node_holder(entities, node), Some(holder) if holder != id)
                    {
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
                    spatial,
                    coordinator,
                    player,
                    worker,
                    &building,
                    tile_x,
                    tile_y,
                    events,
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
                    if owns_unit(entities, player, id) && !is_constructing(entities, id) {
                        entities.release_miner(id);
                        if let Some(e) = entities.get_mut(id) {
                            e.clear_orders();
                            if let Some(w) = e.worker.as_mut() {
                                w.carry = None;
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

/// Whether `player` owns a *unit* with this id. Local re-export of
/// [`world_query::owns_unit`] to keep call sites in this module terse.
fn owns_unit(entities: &EntityStore, player: u32, id: u32) -> bool {
    world_query::owns_unit(entities, player, id)
}

/// True if this unit is a worker that has already begun laying concrete — it cannot
/// be pulled away until the building finishes or is destroyed.
fn is_constructing(entities: &EntityStore, id: u32) -> bool {
    matches!(
        entities.get(id),
        Some(e) if matches!(e.build_phase(), Some(BuildPhase::Constructing { .. }))
    )
}

/// Issue a build order under the "reserve on arrival" model. Validates intent, emits
/// best-effort feedback notices to the player, then walks the worker toward the target
/// tile. Resources are not deducted and no building is spawned here; that happens in the
/// construction system when the worker arrives, at which point placement and affordability
/// are re-checked. Other units may walk through the tile and other build commands may race
/// for it — first arrival wins.
#[allow(clippy::too_many_arguments)]
fn order_build(
    map: &Map,
    entities: &mut EntityStore,
    players: &[PlayerState],
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
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
    if config::building_stats(kind).is_none() {
        notice(events, player, "Unknown building");
        return;
    }

    let owned = world_query::owned_building_kinds(entities, player);
    if !rules::economy::build_requirement_met(kind, &owned) {
        notice(events, player, "Requirement not met");
        return;
    }

    if tile_x >= map.size || tile_y >= map.size {
        notice(events, player, "Cannot build there");
        return;
    }

    // Feedback only — re-checked at arrival.
    let footprint = footprint_tiles(kind, tile_x, tile_y);
    let worker_inside_footprint = matches!(entities.get(worker), Some(w) if footprint.contains(&map.tile_of(w.pos_x, w.pos_y)));
    if !footprint_placeable(map, entities, spatial, kind, tile_x, tile_y)
        && !worker_inside_footprint
    {
        notice(events, player, "Cannot build there");
        return;
    }

    let ps = match players.iter().find(|p| p.id == player) {
        Some(p) => p,
        None => return,
    };
    let (cost_steel, cost_oil) = rules::economy::cost(kind);
    if ps.steel < cost_steel || ps.oil < cost_oil {
        notice(events, player, "Not enough resources");
        return;
    }

    let built = coordinator.order_build(entities, worker, kind, tile_x, tile_y);
    if !built {
        if worker_inside_footprint {
            notice(events, player, "Worker blocks building");
        } else {
            notice(events, player, "Cannot build there");
        }
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
    let ok = matches!(entities.get(building), Some(b)
        if b.owner == player && b.is_building() && !b.under_construction()
        && rules::economy::trainable_units(b.kind).contains(&kind));
    if !ok {
        notice(events, player, "Cannot train that here");
        return;
    }
    let owned_complete = world_query::completed_building_kinds(entities, player);
    if !rules::economy::train_requirement_met(kind, &owned_complete) {
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
    let (cost_steel, cost_oil) = rules::economy::cost(kind);
    let supply = rules::economy::supply_cost(kind);
    if ps.steel < cost_steel || ps.oil < cost_oil {
        notice(events, player, "Not enough resources");
        return;
    }
    if ps.supply_used + supply > ps.supply_cap {
        notice(events, player, "Not enough supply");
        return;
    }
    ps.steel -= cost_steel;
    ps.oil -= cost_oil;
    ps.supply_used += supply;

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
    if config::unit_stats(unit).is_some() {
        if let Some(ps) = players.iter_mut().find(|p| p.id == player) {
            let (cost_steel, cost_oil) = rules::economy::cost(unit);
            ps.steel += cost_steel;
            ps.oil += cost_oil;
            ps.supply_used = ps
                .supply_used
                .saturating_sub(rules::economy::supply_cost(unit));
        }
    }
}

/// Push a best-effort `Notice` event to a player.
pub(crate) fn notice(events: &mut HashMap<u32, Vec<Event>>, player: u32, msg: &str) {
    events.entry(player).or_default().push(Event::Notice {
        msg: msg.to_string(),
    });
}
