//! Per-tick simulation systems. See `DESIGN.md` §3 (`systems.rs`).
//!
//! [`run_tick`] is the entry point called by [`crate::game::Game::tick`]. It runs the systems
//! in the order mandated by the design:
//!   1. drain + apply queued commands (validate ownership / cost / supply / tech / placement)
//!   2. movement (advance along path at unit speed, clamp to passable tiles)
//!   3. combat (acquire targets, deal damage on cooldown, emit `Attack`)
//!   4. gather progression (fetch → harvest → return → deposit → repeat)
//!   5. production progression + spawning
//!   6. construction progression (emit `Build` on completion)
//!   7. deaths (hp <= 0 → remove, emit `Death`; dead building drops its queue)
//!   8. recompute supply cap
//!
//! Everything is panic-free: entity lookups are fallible and stale ids are ignored. The
//! functions are deliberately small and mostly pure helpers to keep the tick loop readable.

use std::collections::{HashMap, HashSet};

use crate::config;
use crate::game::entity::{CarryState, Entity, EntityStore, GatherPhase, Order, ProdItem, NEUTRAL};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::pathfinding::{self, Passability};
use crate::game::PlayerState;
use crate::protocol::{kinds, Command, Event};

/// World pixels at which a unit is considered "arrived" at a waypoint / target point.
const ARRIVE_EPS: f32 = 2.0;
/// Extra slack (px) added to attack range checks so units don't dance at the exact boundary.
const RANGE_SLACK: f32 = 4.0;
/// Max unique unit ids honored per multi-unit command. Caps the per-id work (one A* each) a
/// single command can force, so a repeated/huge id list can't be used to stall the tick loop.
const MAX_UNITS_PER_COMMAND: usize = 256;

// ---------------------------------------------------------------------------
// Occupancy: combined terrain + building-footprint passability for pathfinding.
// ---------------------------------------------------------------------------

/// A snapshot of which tiles are blocked by buildings this tick, layered over terrain. Units
/// never block (soft overlap is allowed), so only static structures appear here.
struct Occupancy<'a> {
    map: &'a Map,
    blocked: Vec<bool>,
}

impl<'a> Occupancy<'a> {
    fn build(map: &'a Map, entities: &EntityStore) -> Self {
        let size = map.size;
        let mut blocked = vec![false; (size * size) as usize];
        for e in entities.iter() {
            if !e.is_building() {
                continue;
            }
            for (tx, ty) in building_footprint(map, e) {
                if tx < size && ty < size {
                    blocked[(ty * size + tx) as usize] = true;
                }
            }
        }
        Occupancy { map, blocked }
    }
}

impl Passability for Occupancy<'_> {
    fn passable(&self, tx: i32, ty: i32) -> bool {
        if !self.map.is_passable(tx, ty) {
            return false;
        }
        let size = self.map.size as i32;
        if tx < 0 || ty < 0 || tx >= size || ty >= size {
            return false;
        }
        !self.blocked[(ty * self.map.size as i32 + tx) as usize]
    }
}

/// The set of tiles a building's footprint covers, centered on its position. Footprints are
/// `foot_w × foot_h`; we center them on the tile under the building center.
fn building_footprint(map: &Map, e: &Entity) -> Vec<(u32, u32)> {
    let Some(s) = config::building_stats(&e.kind) else {
        return Vec::new();
    };
    let (cx, cy) = map.tile_of(e.pos_x, e.pos_y);
    let mut out = Vec::with_capacity((s.foot_w * s.foot_h) as usize);
    // Offsets so the footprint is centered on the building's tile.
    let ox = s.foot_w as i32 / 2;
    let oy = s.foot_h as i32 / 2;
    for dy in 0..s.foot_h as i32 {
        for dx in 0..s.foot_w as i32 {
            let tx = cx as i32 + dx - ox;
            let ty = cy as i32 + dy - oy;
            if tx >= 0 && ty >= 0 {
                out.push((tx as u32, ty as u32));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tick entry point
// ---------------------------------------------------------------------------

/// Run all per-tick systems in order. `events` is the per-player event accumulator (already
/// keyed for every player). `tick` is the new tick number (post-increment).
pub(crate) fn run_tick(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    fog: &Fog,
    pending: Vec<(u32, Command)>,
    events: &mut HashMap<u32, Vec<Event>>,
    _tick: u32,
) {
    // Build occupancy once up front; commands that need pathing reuse it.
    let occ = Occupancy::build(map, entities);

    apply_commands(map, entities, players, &occ, pending, events);
    movement_system(map, entities, &occ);
    combat_system(map, entities, &occ, events);
    gather_system(map, entities, players, &occ);
    production_system(map, entities, players, events);
    construction_system(entities, events);
    death_system(entities, fog, events);
    recompute_supply(players, entities);
}

// ---------------------------------------------------------------------------
// 1. Commands
// ---------------------------------------------------------------------------

fn apply_commands(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
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
                    map, entities, players, occ, player, worker, &building, tile_x, tile_y, events,
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
    let is_worker = matches!(entities.get(id), Some(e) if e.kind == kinds::WORKER);
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
    if !matches!(entities.get(worker), Some(e) if e.kind == kinds::WORKER) {
        notice(events, player, "Only workers can build");
        return;
    }
    let stats = match config::building_stats(building) {
        Some(s) => s,
        None => {
            notice(events, player, "Unknown building");
            return;
        }
    };

    // Tech requirement (e.g. barracks needs an Industrial Center).
    let owned: Vec<&str> = entities
        .iter()
        .filter(|e| e.owner == player && e.is_building())
        .map(|e| e.kind.as_str())
        .collect();
    if !config::build_requirement_met(building, &owned) {
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
    if !footprint_placeable(map, entities, building, tile_x, tile_y) {
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
    let (cx, cy) = footprint_center(map, building, tile_x, tile_y);
    let site = match entities.spawn_building(player, building, cx, cy, false) {
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
    // Building must be owned, finished, and able to train this unit.
    let ok = matches!(entities.get(building), Some(b)
        if b.owner == player && b.is_building() && !b.under_construction
        && config::trainable_units(&b.kind).contains(&unit));
    if !ok {
        notice(events, player, "Cannot train that here");
        return;
    }
    let owned_complete: Vec<&str> = entities
        .iter()
        .filter(|e| e.owner == player && e.is_building() && !e.under_construction)
        .map(|e| e.kind.as_str())
        .collect();
    if !config::train_requirement_met(unit, &owned_complete) {
        notice(events, player, "Requirement not met");
        return;
    }
    let stats = match config::unit_stats(unit) {
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
            unit: unit.to_string(),
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
    if let Some(stats) = config::unit_stats(&unit) {
        if let Some(ps) = players.iter_mut().find(|p| p.id == player) {
            ps.steel += stats.cost_steel;
            ps.oil += stats.cost_oil;
            ps.supply_used = ps.supply_used.saturating_sub(stats.supply);
        }
    }
}

/// Push a best-effort `Notice` event to a player.
fn notice(events: &mut HashMap<u32, Vec<Event>>, player: u32, msg: &str) {
    events.entry(player).or_default().push(Event::Notice {
        msg: msg.to_string(),
    });
}

// ---------------------------------------------------------------------------
// Placement helpers
// ---------------------------------------------------------------------------

/// The tiles a footprint of `building` would cover if its top-left tile were `(tile_x,
/// tile_y)`. The command specifies the top-left tile of the footprint.
fn footprint_tiles(building: &str, tile_x: u32, tile_y: u32) -> Vec<(u32, u32)> {
    let Some(s) = config::building_stats(building) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity((s.foot_w * s.foot_h) as usize);
    for dy in 0..s.foot_h {
        for dx in 0..s.foot_w {
            // Guard against coordinate overflow on huge tile_x/tile_y. An empty footprint is
            // treated as not-placeable by `footprint_placeable`, so the build is cleanly rejected.
            let (Some(tx), Some(ty)) = (tile_x.checked_add(dx), tile_y.checked_add(dy)) else {
                return Vec::new();
            };
            out.push((tx, ty));
        }
    }
    out
}

/// World-pixel center of a footprint placed at top-left tile `(tile_x, tile_y)`.
fn footprint_center(map: &Map, building: &str, tile_x: u32, tile_y: u32) -> (f32, f32) {
    let s = config::building_stats(building).expect("building stats");
    let ts = config::TILE_SIZE as f32;
    let x = tile_x as f32 * ts + (s.foot_w as f32 * ts) * 0.5;
    let y = tile_y as f32 * ts + (s.foot_h as f32 * ts) * 0.5;
    // map is unused beyond stats here, kept for signature symmetry / future clamping.
    let _ = map;
    (x, y)
}

/// Whether `building`'s footprint at `(tile_x, tile_y)` is fully in bounds, on passable
/// terrain, and clear of existing building footprints and resource nodes. `(tile_x, tile_y)` is
/// the footprint's top-left tile. Shared with the AI (`ai.rs`) for picking valid build sites.
pub(crate) fn footprint_placeable(
    map: &Map,
    entities: &EntityStore,
    building: &str,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    let tiles = footprint_tiles(building, tile_x, tile_y);
    if tiles.is_empty() {
        return false;
    }
    // In bounds + passable terrain.
    for &(tx, ty) in &tiles {
        if !map.in_bounds(tx as i32, ty as i32) {
            return false;
        }
        if !map.is_passable(tx as i32, ty as i32) {
            return false;
        }
    }
    // Not overlapping another building's footprint or a resource node tile.
    let mut occupied: Vec<(u32, u32)> = Vec::new();
    for e in entities.iter() {
        if e.is_building() {
            occupied.extend(building_footprint(map, e));
        } else if e.is_node() {
            occupied.push(map.tile_of(e.pos_x, e.pos_y));
        }
    }
    for t in &tiles {
        if occupied.contains(t) {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// 2. Movement
// ---------------------------------------------------------------------------

/// Advance every moving unit along its waypoint path at its speed. Clamps the final landing
/// tile to passable terrain (soft overlap with other units is allowed, so we don't resolve
/// unit-unit collisions here). Arriving at the last waypoint of a plain Move clears the order.
fn movement_system(map: &Map, entities: &mut EntityStore, occ: &Occupancy) {
    for id in entities.ids() {
        // Pull the data we need, then mutate.
        let (speed, mut x, mut y) = {
            let e = match entities.get(id) {
                Some(e) if e.is_unit() && !e.path.is_empty() => e,
                _ => continue,
            };
            let speed = config::unit_stats(&e.kind).map(|s| s.speed).unwrap_or(0.0);
            (speed, e.pos_x, e.pos_y)
        };
        if speed <= 0.0 {
            continue;
        }

        let mut budget = speed;
        let mut new_facing = None;
        // Consume waypoints (stored reversed, next = last element) within this tick's budget.
        loop {
            let next = {
                let e = entities.get(id).unwrap();
                e.path.last().copied()
            };
            let Some((wx, wy)) = next else { break };
            let dx = wx - x;
            let dy = wy - y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= ARRIVE_EPS {
                // Reached this waypoint exactly; drop it and continue with the remaining budget.
                if let Some(e) = entities.get_mut(id) {
                    e.path.pop();
                }
                x = wx;
                y = wy;
                continue;
            }
            new_facing = Some(dy.atan2(dx));
            if dist <= budget {
                // We can reach this waypoint this tick.
                x = wx;
                y = wy;
                budget -= dist;
                if let Some(e) = entities.get_mut(id) {
                    e.path.pop();
                }
            } else {
                // Partial step toward the waypoint.
                let nx = x + dx / dist * budget;
                let ny = y + dy / dist * budget;
                // Clamp landing to a passable tile (don't slide into rock/water/buildings).
                if tile_passable_at(occ, map, nx, ny) {
                    x = nx;
                    y = ny;
                }
                break;
            }
        }

        if let Some(e) = entities.get_mut(id) {
            e.pos_x = x.clamp(0.0, map.world_size_px() - 0.01);
            e.pos_y = y.clamp(0.0, map.world_size_px() - 0.01);
            if let Some(f) = new_facing {
                e.facing = f;
            }
            // A plain Move with an empty path has arrived → go idle.
            if e.path.is_empty() {
                if let Order::Move { .. } = e.order {
                    e.order = Order::Idle;
                }
            }
        }
    }
}

/// Whether a world point lands on a passable (terrain + building) tile.
fn tile_passable_at(occ: &Occupancy, map: &Map, x: f32, y: f32) -> bool {
    let (tx, ty) = map.tile_of(x, y);
    occ.passable(tx as i32, ty as i32)
}

// ---------------------------------------------------------------------------
// 3. Combat
// ---------------------------------------------------------------------------

/// Combat: acquire targets for aggressive / attack-move units, let idle units auto-defend,
/// fire bunkers, and deal damage when off cooldown. Damage is applied immediately and emits an
/// `Attack` event (for tracers). Cooldowns tick down here too.
fn combat_system(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    // Tick down cooldowns first.
    for e in entities.iter_mut() {
        if e.attack_cd > 0 {
            e.attack_cd -= 1;
        }
    }

    // Snapshot lightweight target candidates (id, owner, pos, alive) to avoid borrow conflicts
    // while we mutate attackers and victims.
    let candidates: Vec<(u32, u32, f32, f32)> = entities
        .iter()
        .filter(|e| e.is_targetable() && e.hp > 0)
        .map(|e| (e.id, e.owner, e.pos_x, e.pos_y))
        .collect();

    for id in entities.ids() {
        // Determine this attacker's combat parameters.
        let (owner, px, py, range_px, aggro_px, dmg, cd_reset, mode, is_unit) = {
            let e = match entities.get(id) {
                Some(e) => e,
                None => continue,
            };
            if e.hp == 0 || !e.can_attack() {
                continue;
            }
            let (range_tiles, dmg, cd) = attack_profile(e);
            let range_px = range_tiles as f32 * config::TILE_SIZE as f32 + e.radius() + RANGE_SLACK;
            // Aggro radius: mobile units detect and chase enemies out to their sight radius so
            // attack-move / auto-defend actually close the gap. Buildings (bunkers) never move,
            // so they only ever engage within their firing range.
            let aggro_px = if e.is_unit() {
                (e.sight_tiles() as f32 * config::TILE_SIZE as f32).max(range_px)
            } else {
                range_px
            };
            (
                e.owner,
                e.pos_x,
                e.pos_y,
                range_px,
                aggro_px,
                dmg,
                cd,
                combat_mode(e),
                e.is_unit(),
            )
        };
        if dmg == 0 {
            continue;
        }

        // Resolve / acquire a target id (explicit target for Ordered, nearest enemy in aggro
        // radius for Aggressive).
        let target = resolve_target(entities, &candidates, id, owner, px, py, aggro_px, mode);
        let Some(tid) = target else {
            // No target: clear stale combat target id for non-attack orders.
            if let Some(e) = entities.get_mut(id) {
                if matches!(e.order, Order::AttackMove { .. } | Order::Idle) {
                    e.target_id = None;
                }
            }
            continue;
        };

        // Distance to chosen target.
        let (tx, ty, t_owner) = match entities.get(tid) {
            Some(t) => (t.pos_x, t.pos_y, t.owner),
            None => continue,
        };
        if t_owner == owner {
            continue; // never friendly fire
        }
        let dist = dist2(px, py, tx, ty).sqrt();

        if dist <= range_px {
            // In range: face it, stop, and fire if off cooldown.
            let ready = matches!(entities.get(id), Some(e) if e.attack_cd == 0);
            if let Some(e) = entities.get_mut(id) {
                e.facing = (ty - py).atan2(tx - px);
                e.target_id = Some(tid);
                // Hold position while a target is in weapon range (don't overshoot it).
                e.path.clear();
            }
            if ready {
                apply_damage(entities, events, id, tid, dmg, owner);
                if let Some(e) = entities.get_mut(id) {
                    e.attack_cd = cd_reset;
                }
            }
        } else if is_unit {
            // Out of weapon range but within aggro: chase. Re-path with A* toward the target
            // tile when we have no path, so units route around obstacles rather than stalling.
            let want_repath = entities.get(id).map(|e| e.path.is_empty()).unwrap_or(false);
            if let Some(e) = entities.get_mut(id) {
                e.target_id = Some(tid);
            }
            if want_repath {
                repath(map, entities, occ, id, tx, ty);
            }
        }
    }
}

/// Attack profile (range_tiles, dmg, cooldown) for a unit or bunker.
fn attack_profile(e: &Entity) -> (u32, u32, u32) {
    if let Some(s) = config::unit_stats(&e.kind) {
        (s.range_tiles, s.dmg, s.cooldown)
    } else if let Some(s) = config::building_stats(&e.kind) {
        (s.range_tiles, s.dmg, s.cooldown)
    } else {
        (0, 0, 0)
    }
}

/// How a combatant chooses targets.
#[derive(Copy, Clone, PartialEq)]
enum CombatMode {
    /// Has an explicit attack target id.
    Ordered,
    /// Engages any enemy within range (attack-move, bunkers, idle auto-defend).
    Aggressive,
}

fn combat_mode(e: &Entity) -> CombatMode {
    match e.order {
        Order::Attack { .. } => CombatMode::Ordered,
        _ => CombatMode::Aggressive,
    }
}

/// Resolve which entity an attacker should engage this tick.
fn resolve_target(
    entities: &EntityStore,
    candidates: &[(u32, u32, f32, f32)],
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    mode: CombatMode,
) -> Option<u32> {
    // Ordered attackers keep their explicit target if it still exists.
    if mode == CombatMode::Ordered {
        if let Some(e) = entities.get(self_id) {
            if let Order::Attack { target } = e.order {
                if entities.get(target).map(|t| t.hp > 0).unwrap_or(false) {
                    return Some(target);
                }
            }
        }
        // Explicit target gone → fall through to acquisition so we don't stand idle.
    }

    // Aggressive acquisition: the nearest enemy within the acquire radius (weapon range for
    // buildings, sight range for mobile units so they chase).
    let mut best: Option<(u32, f32)> = None;
    for &(cid, c_owner, cx, cy) in candidates {
        if cid == self_id || c_owner == owner || c_owner == NEUTRAL {
            continue;
        }
        let d = dist2(px, py, cx, cy);
        if d <= acquire_px * acquire_px {
            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((cid, d));
            }
        }
    }
    best.map(|(cid, _)| cid)
}

/// Apply `dmg` to `victim` from `attacker`, emitting an `Attack` event to the attacker's
/// owner. Death itself is handled by the death system (we only zero hp here).
fn apply_damage(
    entities: &mut EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
    attacker: u32,
    victim: u32,
    dmg: u32,
    attacker_owner: u32,
) {
    if let Some(v) = entities.get_mut(victim) {
        v.hp = v.hp.saturating_sub(dmg);
    }
    events
        .entry(attacker_owner)
        .or_default()
        .push(Event::Attack {
            from: attacker,
            to: victim,
        });
}

// ---------------------------------------------------------------------------
// 4. Gather
// ---------------------------------------------------------------------------

/// Worker harvest loop: walk to node → harvest `HARVEST_TICKS` → carry a load → return to the
/// nearest own Industrial Center → deposit → repeat. Depletes the node; when empty, retargets a nearby
/// same-kind node or goes idle.
fn gather_system(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
) {
    let interact = config::TILE_SIZE as f32 * 1.5; // close enough to mine / deposit

    for id in entities.ids() {
        let node = match entities.get(id) {
            Some(e) if e.kind == kinds::WORKER => match e.order {
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
            GatherPhase::ToNode => gather_to_node(map, entities, occ, id, node, interact),
            GatherPhase::Harvesting => gather_harvesting(map, entities, occ, id, node, interact),
            GatherPhase::ToHome => gather_to_home(map, entities, players, occ, id, node, interact),
        }
    }
}

fn gather_to_node(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    id: u32,
    node: u32,
    interact: f32,
) {
    // Node still valid?
    let node_pos = match entities.get(node) {
        Some(n) if n.is_node() && n.remaining > 0 => (n.pos_x, n.pos_y),
        _ => {
            retarget_or_idle(map, entities, occ, id, node);
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
        repath(map, entities, occ, id, node_pos.0, node_pos.1);
    }
}

fn gather_harvesting(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    id: u32,
    node: u32,
    _interact: f32,
) {
    // Node still valid?
    let node_kind_amount = match entities.get(node) {
        Some(n) if n.is_node() && n.remaining > 0 => (n.kind.clone(), n.remaining),
        _ => {
            retarget_or_idle(map, entities, occ, id, node);
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
    let is_oil = node_kind_amount.0 == kinds::OIL;
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
            is_oil,
        });
        e.harvest_progress = 0;
        e.gather_phase = GatherPhase::ToHome;
    }
    // Route to the nearest own Industrial Center.
    route_home(map, entities, occ, id);
}

fn gather_to_home(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    occ: &Occupancy,
    id: u32,
    node: u32,
    interact: f32,
) {
    let (owner, wx, wy) = match entities.get(id) {
        Some(e) => (e.owner, e.pos_x, e.pos_y),
        None => return,
    };
    // Find nearest own, finished Industrial Center.
    let industrial_center = nearest_own_industrial_center(entities, owner, wx, wy);
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
        let (amount, is_oil) = entities
            .get(id)
            .and_then(|e| e.carry)
            .map(|c| (c.amount, c.is_oil))
            .unwrap_or((0, false));
        if amount > 0 {
            if let Some(ps) = players.iter_mut().find(|p| p.id == owner) {
                if is_oil {
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
        gather_to_node(map, entities, occ, id, node, interact);
    } else if entities.get(id).map(|e| e.path.is_empty()).unwrap_or(true) {
        repath(map, entities, occ, id, hx, hy);
    }
}

/// Route a laden worker to its nearest own Industrial Center.
fn route_home(map: &Map, entities: &mut EntityStore, occ: &Occupancy, id: u32) {
    let (owner, wx, wy) = match entities.get(id) {
        Some(e) => (e.owner, e.pos_x, e.pos_y),
        None => return,
    };
    if let Some((industrial_center_id, hx, hy)) =
        nearest_own_industrial_center(entities, owner, wx, wy)
    {
        if let Some(e) = entities.get_mut(id) {
            e.home_industrial_center = Some(industrial_center_id);
        }
        repath(map, entities, occ, id, hx, hy);
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
        && w.kind == kinds::WORKER
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
    id: u32,
    old_node: u32,
) {
    let (owner, wx, wy, want_oil) = {
        let e = match entities.get(id) {
            Some(e) => e,
            None => return,
        };
        let want_oil = matches!(entities.get(old_node), Some(n) if n.kind == kinds::OIL);
        (e.owner, e.pos_x, e.pos_y, want_oil)
    };
    let _ = owner;
    let want_kind = if want_oil {
        kinds::OIL
    } else {
        kinds::STEEL
    };

    // Nearest same-kind, non-empty node within a reasonable radius.
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
            if let Some(e) = entities.get_mut(id) {
                e.order = Order::Gather { node: nid };
                e.target_id = Some(nid);
                e.gather_phase = GatherPhase::ToNode;
                e.harvest_progress = 0;
            }
            repath(map, entities, occ, id, nx, ny);
        }
        None => {
            if let Some(e) = entities.get_mut(id) {
                e.clear_orders();
            }
        }
    }
}

/// Nearest finished Industrial Center owned by `owner` to a point, as `(id, x, y)`.
fn nearest_own_industrial_center(
    entities: &EntityStore,
    owner: u32,
    x: f32,
    y: f32,
) -> Option<(u32, f32, f32)> {
    let mut best: Option<(u32, f32, f32, f32)> = None;
    for e in entities.iter() {
        if e.owner == owner && e.kind == kinds::INDUSTRIAL_CENTER && !e.under_construction {
            let d = dist2(x, y, e.pos_x, e.pos_y);
            if best.map(|(_, _, _, bd)| d < bd).unwrap_or(true) {
                best = Some((e.id, e.pos_x, e.pos_y, d));
            }
        }
    }
    best.map(|(id, hx, hy, _)| (id, hx, hy))
}

/// Recompute an A* path for a unit toward a world point and store it.
fn repath(map: &Map, entities: &mut EntityStore, occ: &Occupancy, id: u32, gx: f32, gy: f32) {
    let (sx, sy) = match entities.get(id) {
        Some(e) => map.tile_of(e.pos_x, e.pos_y),
        None => return,
    };
    let (gtx, gty) = map.tile_of(gx, gy);
    let path = pathfinding::find_path(occ, sx as i32, sy as i32, gtx as i32, gty as i32);
    let mut waypoints = pathfinding::to_world_waypoints(&path);
    if !waypoints.is_empty() {
        waypoints[0] = (gx, gy);
    } else {
        // Best-effort straight-line waypoint so the worker still nudges toward the goal.
        waypoints = vec![(gx, gy)];
    }
    if let Some(e) = entities.get_mut(id) {
        e.path = waypoints;
    }
}

// ---------------------------------------------------------------------------
// 5. Production
// ---------------------------------------------------------------------------

/// Advance each building's front production item; on completion spawn the unit adjacent to the
/// building and remove the item from the queue. Supply was already reserved on enqueue, so
/// spawning does not re-charge it. Cost was charged at enqueue too.
fn production_system(
    map: &Map,
    entities: &mut EntityStore,
    _players: &mut [PlayerState],
    _events: &mut HashMap<u32, Vec<Event>>,
) {
    for id in entities.ids() {
        // Is this a finished building with a non-empty queue?
        let (owner, kind, completed_unit) = {
            let b = match entities.get_mut(id) {
                Some(b) if b.is_building() && !b.under_construction && !b.prod_queue.is_empty() => {
                    b
                }
                _ => continue,
            };
            let front = &mut b.prod_queue[0];
            front.progress += 1;
            if front.progress >= front.total {
                let unit = b.prod_queue.remove(0).unit;
                (b.owner, b.kind.clone(), Some(unit))
            } else {
                (b.owner, b.kind.clone(), None)
            }
        };

        if let Some(unit) = completed_unit {
            // Spawn adjacent to the building footprint.
            let (bx, by) = match entities.get(id) {
                Some(b) => (b.pos_x, b.pos_y),
                None => continue,
            };
            let (sx, sy) = spawn_point_near(map, &kind, bx, by);
            entities.spawn_unit(owner, &unit, sx, sy);
        }
    }
}

/// A reasonable spawn point just outside a building's footprint toward the map below it.
fn spawn_point_near(map: &Map, building_kind: &str, bx: f32, by: f32) -> (f32, f32) {
    let ts = config::TILE_SIZE as f32;
    let half = config::building_stats(building_kind)
        .map(|s| (s.foot_h as f32 * ts) * 0.5)
        .unwrap_or(ts);
    // Prefer spawning below the building; clamp into the world.
    let max = map.world_size_px() - 1.0;
    let x = bx.clamp(0.0, max);
    let y = (by + half + ts * 0.5).clamp(0.0, max);
    (x, y)
}

// ---------------------------------------------------------------------------
// 6. Construction
// ---------------------------------------------------------------------------

/// Advance construction for buildings that have a worker actively building them. A worker on a
/// `Build` order that has arrived at the site contributes one tick of progress per tick. On
/// completion the building leaves CONSTRUCT, the worker is freed (idle), and a `Build` event
/// fires to the owner.
fn construction_system(entities: &mut EntityStore, events: &mut HashMap<u32, Vec<Event>>) {
    // Collect (worker_id, site_id) build assignments where the worker has reached the site.
    let mut working: Vec<(u32, u32)> = Vec::new();
    for e in entities.iter() {
        if e.is_unit() {
            if let Order::Build { site } = e.order {
                if let Some(b) = entities.get(site) {
                    let arrive =
                        interact_range(entities, site).unwrap_or(config::TILE_SIZE as f32 * 2.0);
                    if b.under_construction
                        && dist2(e.pos_x, e.pos_y, b.pos_x, b.pos_y).sqrt() <= arrive
                    {
                        working.push((e.id, site));
                    }
                }
            }
        }
    }

    for (worker, site) in working {
        // Stop the worker moving while it builds.
        if let Some(w) = entities.get_mut(worker) {
            w.path.clear();
        }
        let completed = {
            let b = match entities.get_mut(site) {
                Some(b) if b.under_construction => b,
                _ => continue,
            };
            b.build_progress += 1;
            if b.build_progress >= b.build_total {
                b.under_construction = false;
                b.build_progress = b.build_total;
                true
            } else {
                false
            }
        };
        if completed {
            let (owner, kind) = entities
                .get(site)
                .map(|b| (b.owner, b.kind.clone()))
                .unwrap_or((0, String::new()));
            events
                .entry(owner)
                .or_default()
                .push(Event::Build { id: site, kind });
            // Free the worker.
            if let Some(w) = entities.get_mut(worker) {
                w.clear_orders();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 7. Deaths
// ---------------------------------------------------------------------------

/// Remove entities whose hp has hit zero, emitting a fog-respecting `Death` event: a player
/// gets the poof only if they owned the entity or its death position is currently visible to
/// them (events are best-effort flavor). `death_system` runs before the fog recompute, so the
/// current fog still reflects who could see the unit while it was alive — exactly the players
/// who should see it die. A dead building drops its queue implicitly by being removed. Workers
/// building a since-removed site are reset elsewhere.
fn death_system(entities: &mut EntityStore, fog: &Fog, events: &mut HashMap<u32, Vec<Event>>) {
    let dead: Vec<(u32, u32, f32, f32, String)> = entities
        .iter()
        .filter(|e| e.is_targetable() && e.hp == 0)
        .map(|e| (e.id, e.owner, e.pos_x, e.pos_y, e.kind.clone()))
        .collect();

    for (id, owner, x, y, kind) in dead {
        entities.remove(id);
        // Deliver the death only to players who owned the entity or could see where it died,
        // so a death poof never reveals an entity hidden in a player's fog.
        let pids: Vec<u32> = events.keys().copied().collect();
        for pid in pids {
            if pid != owner && !fog.is_visible_world(pid, x, y) {
                continue;
            }
            events.entry(pid).or_default().push(Event::Death {
                id,
                x,
                y,
                kind: kind.clone(),
            });
        }
    }

    // Clean up dangling orders that reference removed entities (build sites, attack targets)
    // so units don't chase ghosts. Gather orders self-heal via `retarget_or_idle`.
    for id in entities.ids() {
        let stale = {
            let e = entities.get(id).unwrap();
            match e.order {
                Order::Attack { target } => !entities.contains(target),
                Order::Build { site } => !entities.contains(site),
                _ => false,
            }
        };
        if stale {
            if let Some(e) = entities.get_mut(id) {
                e.clear_orders();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 8. Supply
// ---------------------------------------------------------------------------

/// Recompute each player's supply cap (from completed Industrial Centers/Depots) and supply used (living
/// units + units still in production queues). Cap is clamped to `SUPPLY_CAP_MAX`.
pub(crate) fn recompute_supply(players: &mut [PlayerState], entities: &EntityStore) {
    for ps in players.iter_mut() {
        let mut cap = 0u32;
        let mut used = 0u32;
        for e in entities.iter() {
            if e.owner != ps.id {
                continue;
            }
            if e.is_building() && !e.under_construction {
                if let Some(s) = config::building_stats(&e.kind) {
                    cap += s.provides_supply;
                }
                // Units queued for production reserve supply too.
                for item in &e.prod_queue {
                    if let Some(us) = config::unit_stats(&item.unit) {
                        used += us.supply;
                    }
                }
            } else if e.is_unit() {
                if let Some(us) = config::unit_stats(&e.kind) {
                    used += us.supply;
                }
            }
        }
        ps.supply_cap = cap.min(config::SUPPLY_CAP_MAX);
        ps.supply_used = used;
    }
}

// ---------------------------------------------------------------------------
// Small pure helpers
// ---------------------------------------------------------------------------

/// Squared euclidean distance (avoids a sqrt where only comparisons are needed).
#[inline]
fn dist2(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

/// Distance (px) at which a worker is "in contact" with an entity for harvest / deposit.
/// Accounts for the target's radius (a 3×3 Industrial Center is ~1.5 tiles wide) so a worker standing just
/// outside a building footprint still counts as adjacent. `None` for a missing entity.
fn interact_range(entities: &EntityStore, target: u32) -> Option<f32> {
    let t = entities.get(target)?;
    // Target half-extent + roughly one worker reach (one tile) of slack.
    Some(t.radius() + config::TILE_SIZE as f32)
}
