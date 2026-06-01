//! Movement and Pathing Coordinator. See `PLAN-4.1.md`.
//!
//! The coordinator is the single place that decides:
//! - when a path request is issued,
//! - how much budget it gets,
//! - whether a cached/shared result can be reused,
//! - how a blocked or failed move is represented,
//! - where a spawned unit should try to stand.
//!
//! It wraps the low-level [`PathingService`] (A* + LRU cache) and adds:
//! - per-tick request budgeting,
//! - deterministic goal spreading for multi-unit moves,
//! - repath throttling,
//! - spawn-point search around buildings.

use crate::config;
use crate::game::entity::{EntityKind, EntityStore, MovePhase, Order, WeaponSetup};
use crate::game::map::{Map, MobilityClass};
use crate::game::pathfinding::Passability;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::pathing::{PathRequest, PathingService};

/// Maximum number of fresh A* path requests serviced in a single tick. Beyond this,
/// remaining `AwaitingPath` units stay queued for the next tick.
const MAX_REQUESTS_PER_TICK: usize = 64;

/// Minimum ticks between repaths for a single unit. Prevents chase/gather spam.
const MIN_REPATH_TICKS: u32 = 3;

/// If the goal moves by more than this many world pixels, bypass the repath throttle.
const MATERIAL_GOAL_DELTA_PX: f32 = config::TILE_SIZE as f32;

/// The movement/pathing coordinator for one tick.
pub struct MoveCoordinator<'a> {
    pathing: &'a mut PathingService,
    map: &'a Map,
    occ: &'a Occupancy<'a>,
    tick: u32,
    budget: usize,
}

impl<'a> MoveCoordinator<'a> {
    pub fn new(
        pathing: &'a mut PathingService,
        map: &'a Map,
        occ: &'a Occupancy<'a>,
        tick: u32,
    ) -> Self {
        MoveCoordinator {
            pathing,
            map,
            occ,
            tick,
            budget: MAX_REQUESTS_PER_TICK,
        }
    }

    // -------------------------------------------------------------------
    // High-level order helpers (called by commands.rs)
    // -------------------------------------------------------------------

    /// Issue a move or attack-move order to a group of units owned by `player`. Computes a
    /// formation anchor, spreads individual goals around it, and marks every valid unit
    /// `AwaitingPath`. Non-units or entities not owned by `player` are skipped silently.
    pub fn order_group_move(
        &mut self,
        entities: &mut EntityStore,
        player: u32,
        ids: &[u32],
        goal: (f32, f32),
        attack_move: bool,
    ) {
        if ids.is_empty() {
            return;
        }
        let anchor = self.map.tile_of(goal.0, goal.1);
        let goals = spread_goals(self.map, self.occ, ids, anchor);

        for (id, g) in ids.iter().zip(goals.iter()) {
            entities.release_miner(*id);
            let Some(e) = entities.get_mut(*id) else {
                continue;
            };
            if !e.is_unit() || e.owner != player {
                continue;
            }
            let order = if attack_move {
                Order::attack_move_to(g.0, g.1)
            } else {
                Order::move_to(g.0, g.1)
            };
            e.set_order(order);
            e.set_target_id(None);
            e.set_path(Vec::new());
            e.set_path_goal(Some(*g));
            e.mark_move_phase(MovePhase::AwaitingPath);
            e.reset_gather_state();
            begin_machine_gunner_teardown(e);
            let (px, py) = (e.pos_x, e.pos_y);
            e.reset_stuck(px, py);
        }
    }

    /// Issue an attack order against a specific target. Sets the order and requests an
    /// initial path immediately (budget permitting).
    pub fn order_attack(&mut self, entities: &mut EntityStore, id: u32, target: u32) {
        let (tx, ty) = match entities.get(target) {
            Some(t) => (t.pos_x, t.pos_y),
            None => return,
        };
        entities.release_miner(id);
        let mut request_initial_path = true;
        if let Some(e) = entities.get_mut(id) {
            e.set_order(Order::attack(target));
            e.set_target_id(Some(target));
            e.set_path(Vec::new());
            e.set_path_goal(Some((tx, ty)));
            e.reset_gather_state();
            // An explicit attack order is not necessarily a move command for a deployed MG:
            // it may be able to slew and fire immediately. Combat requests a chase path only
            // if the target is actually out of range, after teardown if needed.
            request_initial_path = e.kind != EntityKind::MachineGunner;
            let (px, py) = (e.pos_x, e.pos_y);
            e.reset_stuck(px, py);
        }
        if request_initial_path {
            self.request_path(entities, id, (tx, ty));
        }
    }

    /// Issue a gather order. Sets the order and requests an initial path (budget permitting).
    pub fn order_gather(&mut self, entities: &mut EntityStore, id: u32, node: u32) {
        let (nx, ny) = match entities.get(node) {
            Some(n) => (n.pos_x, n.pos_y),
            None => return,
        };
        entities.release_miner(id);
        if let Some(e) = entities.get_mut(id) {
            e.set_order(Order::gather(node));
            e.set_target_id(Some(node));
            e.set_path(Vec::new());
            e.set_path_goal(Some((nx, ny)));
            if let Some(w) = e.worker.as_mut() {
                w.carry = None;
            }
            let (px, py) = (e.pos_x, e.pos_y);
            e.reset_stuck(px, py);
        }
        self.request_path(entities, id, (nx, ny));
    }

    /// Issue a build order: record the placement intent on the worker and walk it to the
    /// target top-left tile. No building is spawned and no resources are deducted here;
    /// that happens on arrival in the construction system.
    pub fn order_build(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        kind: EntityKind,
        tile_x: u32,
        tile_y: u32,
    ) {
        let (cx, cy) = self.map.tile_center(tile_x, tile_y);
        entities.release_miner(id);
        if let Some(e) = entities.get_mut(id) {
            e.set_order(Order::build(kind, tile_x, tile_y));
            e.set_target_id(None);
            e.set_path(Vec::new());
            e.set_path_goal(Some((cx, cy)));
            e.reset_gather_state();
            let (px, py) = (e.pos_x, e.pos_y);
            e.reset_stuck(px, py);
        }
        self.request_path(entities, id, (cx, cy));
    }

    // -------------------------------------------------------------------
    // Tick-scoped bulk processing
    // -------------------------------------------------------------------

    /// Process all units currently in `MovePhase::AwaitingPath` in deterministic entity-id
    /// order, assigning paths up to the tick budget. Units that can't be serviced this tick
    /// remain `AwaitingPath`; units that fail to get any route are marked `PathFailed`.
    pub fn process_awaiting_paths(&mut self, entities: &mut EntityStore) {
        let mut waiting: Vec<u32> = entities
            .iter()
            .filter(|e| e.is_unit() && e.move_phase() == Some(MovePhase::AwaitingPath))
            .map(|e| e.id)
            .collect();
        waiting.sort_unstable();

        for id in waiting {
            if self.budget == 0 {
                break;
            }
            let goal = match entities.get(id).and_then(|e| e.path_goal()) {
                Some(g) => g,
                None => continue,
            };
            self.request_path(entities, id, goal);
        }
    }

    // -------------------------------------------------------------------
    // Mid-tick repath requests (combat / gather)
    // -------------------------------------------------------------------

    /// Request a chase path for a combat unit, respecting throttle and budget.
    /// Returns `true` if a path was actually requested this call.
    pub fn request_chase_path(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        target_pos: (f32, f32),
    ) -> bool {
        if !self.can_repath(entities, id, target_pos) {
            return false;
        }
        if self.budget == 0 {
            return false;
        }
        self.request_path(entities, id, target_pos)
    }

    /// Request a path for a gatherer, respecting throttle and budget.
    /// Returns `true` if a path was actually requested this call.
    pub fn request_gather_path(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        node_pos: (f32, f32),
    ) -> bool {
        if !self.can_repath(entities, id, node_pos) {
            return false;
        }
        if self.budget == 0 {
            return false;
        }
        self.request_path(entities, id, node_pos)
    }

    // -------------------------------------------------------------------
    // Spawn search
    // -------------------------------------------------------------------

    /// Find a spawn point near a building using a deterministic outward search.
    /// Falls back to just below the building if no valid point exists.
    pub fn find_spawn_point(
        &self,
        _entities: &EntityStore,
        building_kind: EntityKind,
        spawned_kind: EntityKind,
        bx: f32,
        by: f32,
    ) -> (f32, f32) {
        let ts = config::TILE_SIZE as f32;
        let bstats = match config::building_stats(building_kind) {
            Some(s) => s,
            None => return (bx, by + ts),
        };
        let spawn_radius = config::unit_stats(spawned_kind)
            .map(|s| s.radius)
            .unwrap_or(0.0);
        let (btx, bty) = self.map.tile_of(bx, by);
        let half_w = (bstats.foot_w as i32) / 2;
        let half_h = (bstats.foot_h as i32) / 2;

        // Search outward in rings from the building footprint edge.
        for r in 1i32..=6 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    let tx = btx as i32 + dx;
                    let ty = bty as i32 + dy;
                    if tx < 0 || ty < 0 {
                        continue;
                    }
                    let (tx, ty) = (tx as u32, ty as u32);

                    // Must be outside the building footprint.
                    let min_x = btx as i32 - half_w;
                    let max_x = btx as i32 + half_w + (bstats.foot_w % 2) as i32;
                    let min_y = bty as i32 - half_h;
                    let max_y = bty as i32 + half_h + (bstats.foot_h % 2) as i32;
                    let in_footprint = (tx as i32) >= min_x
                        && (tx as i32) < max_x
                        && (ty as i32) >= min_y
                        && (ty as i32) < max_y;
                    if in_footprint {
                        continue;
                    }

                    // Must be passable for infantry (spawning units are currently always infantry).
                    if !self
                        .map
                        .is_passable_for(MobilityClass::Infantry, tx as i32, ty as i32)
                    {
                        continue;
                    }

                    // Must not be occupied by another building footprint.
                    if !self.occ.passable(tx as i32, ty as i32) {
                        continue;
                    }

                    let (cx, cy) = self.map.tile_center(tx, ty);
                    if !spawn_point_has_clearance(self.map, self.occ, cx, cy, spawn_radius) {
                        continue;
                    }
                    return (cx, cy);
                }
            }
        }

        // Fallback: below the building, clamped to world bounds.
        let max = self.map.world_size_px() - 1.0;
        let half = (bstats.foot_h as f32 * ts) * 0.5;
        let x = bx.clamp(0.0, max);
        let y = (by + half + ts * 0.5).clamp(0.0, max);
        if spawn_point_has_clearance(self.map, self.occ, x, y, spawn_radius) {
            return (x, y);
        }
        (x, y)
    }

    // -------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------

    /// Direct path request without throttle check. Updates budget, entity path, and phase.
    fn request_path(&mut self, entities: &mut EntityStore, id: u32, goal: (f32, f32)) -> bool {
        let ((sx, sy), kind) = match entities.get(id) {
            Some(e) => (self.map.tile_of(e.pos_x, e.pos_y), e.kind),
            None => return false,
        };
        let (gx, gy) = self.map.tile_of(goal.0, goal.1);
        let radius_tiles = config::unit_stats(kind)
            .map(|s| s.radius_tiles())
            .unwrap_or(0);
        let req = PathRequest {
            start: (sx as i32, sy as i32),
            goal: (gx as i32, gy as i32),
            class: MobilityClass::from_kind(kind),
            radius_tiles,
            budget: None,
        };
        let mut waypoints = self.pathing.request(self.map, self.occ, req);

        // Snap the final waypoint to the exact requested goal for precise arrival.
        if !waypoints.is_empty() {
            waypoints[0] = goal;
        }

        let path_ok = !waypoints.is_empty();
        if let Some(e) = entities.get_mut(id) {
            e.set_path(waypoints);
            e.set_last_repath_tick(self.tick);
            e.set_path_goal(Some(goal));
            if matches!(e.order(), Order::Move(_) | Order::AttackMove(_)) {
                e.mark_move_phase(if path_ok {
                    MovePhase::Moving
                } else {
                    MovePhase::PathFailed
                });
            }
        }
        self.budget = self.budget.saturating_sub(1);
        path_ok
    }

    /// Throttle check: has enough time passed, or did the goal materially change?
    fn can_repath(&self, entities: &EntityStore, id: u32, new_goal: (f32, f32)) -> bool {
        let e = match entities.get(id) {
            Some(e) if e.is_unit() => e,
            _ => return false,
        };
        let elapsed = self.tick.saturating_sub(e.last_repath_tick());
        if elapsed >= MIN_REPATH_TICKS {
            return true;
        }
        if let Some(old_goal) = e.path_goal() {
            let dx = (old_goal.0 - new_goal.0).abs();
            let dy = (old_goal.1 - new_goal.1).abs();
            if dx > MATERIAL_GOAL_DELTA_PX || dy > MATERIAL_GOAL_DELTA_PX {
                return true;
            }
        }
        false
    }
}

fn begin_machine_gunner_teardown(e: &mut crate::game::entity::Entity) {
    if e.kind != EntityKind::MachineGunner {
        return;
    }
    if !matches!(e.weapon_setup(), WeaponSetup::Packed) {
        e.set_weapon_setup(WeaponSetup::TearingDown {
            ticks: config::MACHINE_GUNNER_SETUP_TICKS,
        });
    }
}

// ---------------------------------------------------------------------------
// Goal spreading
// ---------------------------------------------------------------------------

/// Spread unit goals around the requested anchor tile. Returns one goal world point per unit
/// in the same order as `ids`.
fn spread_goals(map: &Map, occ: &Occupancy, ids: &[u32], anchor: (u32, u32)) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(ids.len());
    let mut taken: Vec<(u32, u32)> = Vec::new();

    for _ in ids {
        let tile = find_unique_tile_near(map, occ, anchor, &taken);
        taken.push(tile);
        out.push(map.tile_center(tile.0, tile.1));
    }

    out
}

/// Search outward from `anchor` in deterministic ring order and return the first passable tile
/// not already in `taken`. Falls back to `anchor` itself if nothing better exists.
fn find_unique_tile_near(
    map: &Map,
    occ: &Occupancy,
    anchor: (u32, u32),
    taken: &[(u32, u32)],
) -> (u32, u32) {
    // Try the anchor first.
    if is_free_goal(map, occ, anchor, taken) {
        return anchor;
    }

    for r in 1i32..=6 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r {
                    continue;
                }
                let tx = anchor.0 as i32 + dx;
                let ty = anchor.1 as i32 + dy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let t = (tx as u32, ty as u32);
                if is_free_goal(map, occ, t, taken) {
                    return t;
                }
            }
        }
    }

    // Absolute fallback: anchor itself even if occupied.
    anchor
}

fn is_free_goal(map: &Map, occ: &Occupancy, tile: (u32, u32), taken: &[(u32, u32)]) -> bool {
    if !map.is_passable_for(MobilityClass::Infantry, tile.0 as i32, tile.1 as i32) {
        return false;
    }
    if !occ.passable(tile.0 as i32, tile.1 as i32) {
        return false;
    }
    if taken.contains(&tile) {
        return false;
    }
    true
}

fn spawn_point_has_clearance(map: &Map, occ: &Occupancy, cx: f32, cy: f32, radius: f32) -> bool {
    if radius <= 0.0 {
        return true;
    }

    let max = map.world_size_px();
    if cx - radius < 0.0 || cy - radius < 0.0 || cx + radius > max || cy + radius > max {
        return false;
    }

    let min_tx = ((cx - radius) / config::TILE_SIZE as f32).floor() as i32;
    let min_ty = ((cy - radius) / config::TILE_SIZE as f32).floor() as i32;
    let max_tx = ((cx + radius) / config::TILE_SIZE as f32).floor() as i32;
    let max_ty = ((cy + radius) / config::TILE_SIZE as f32).floor() as i32;

    for ty in min_ty..=max_ty {
        for tx in min_tx..=max_tx {
            if !map.in_bounds(tx, ty) || !occ.passable(tx, ty) {
                let tile_left = tx as f32 * config::TILE_SIZE as f32;
                let tile_top = ty as f32 * config::TILE_SIZE as f32;
                let tile_right = tile_left + config::TILE_SIZE as f32;
                let tile_bottom = tile_top + config::TILE_SIZE as f32;

                let nearest_x = cx.clamp(tile_left, tile_right);
                let nearest_y = cy.clamp(tile_top, tile_bottom);
                let dx = cx - nearest_x;
                let dy = cy - nearest_y;
                if dx * dx + dy * dy <= radius * radius {
                    return false;
                }
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore, MovePhase, Order};
    use crate::game::map::Map;
    use crate::game::services::occupancy::Occupancy;

    #[test]
    fn goal_spreading_assigns_unique_tiles_deterministically() {
        let map = Map::generate(1, 0x1234_5678);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        let ids = vec![1, 2, 3, 4, 5];
        let anchor = (10u32, 10u32);
        let goals = spread_goals(&map, &occ, &ids, anchor);

        assert_eq!(goals.len(), ids.len());

        // All goals should be unique (no two units share the same tile center).
        let mut seen = std::collections::HashSet::new();
        for g in &goals {
            let tile = map.tile_of(g.0, g.1);
            assert!(
                seen.insert(tile),
                "duplicate goal tile {tile:?} for multi-unit spread"
            );
        }

        // First goal should be the anchor itself when it's free.
        let anchor_center = map.tile_center(anchor.0, anchor.1);
        assert_eq!(goals[0], anchor_center);
    }

    #[test]
    fn spawn_search_finds_point_outside_footprint() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        // Place a barracks at tile (15, 15); footprint is 3x2.
        let (cx, cy) = map.tile_center(15, 15);
        let b_id = entities
            .spawn_building(1, EntityKind::Barracks, cx, cy, true)
            .unwrap();
        let occ = Occupancy::build(&map, &entities);

        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        let b = entities.get(b_id).unwrap();
        let (sx, sy) =
            coordinator.find_spawn_point(&entities, b.kind, EntityKind::Tank, b.pos_x, b.pos_y);

        let (stx, sty) = map.tile_of(sx, sy);
        let (btx, bty) = map.tile_of(b.pos_x, b.pos_y);

        // Spawn tile must be outside the barracks footprint.
        assert!(
            stx < btx || stx >= btx + 3 || sty < bty || sty >= bty + 2,
            "spawn tile ({stx},{sty}) is inside the 3x2 footprint at ({btx},{bty})"
        );

        // Spawn tile must be passable for infantry.
        assert!(map.is_passable_for(MobilityClass::Infantry, stx as i32, sty as i32));
    }

    #[test]
    fn tank_spawn_point_keeps_clear_of_top_map_edge() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        let (bx, by) = map.tile_center(3, 0);
        let b_id = entities
            .spawn_building(1, EntityKind::TankFactory, bx, by, true)
            .unwrap();
        let occ = Occupancy::build(&map, &entities);

        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        let b = entities.get(b_id).unwrap();
        let (sx, sy) =
            coordinator.find_spawn_point(&entities, b.kind, EntityKind::Tank, b.pos_x, b.pos_y);

        assert!(
            spawn_point_has_clearance(
                &map,
                &occ,
                sx,
                sy,
                config::unit_stats(EntityKind::Tank).unwrap().radius,
            ),
            "tank spawn point clips the top map edge"
        );
    }

    #[test]
    fn tank_spawn_point_keeps_clear_of_adjacent_building() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        let (fx, fy) = map.tile_center(16, 16);
        let factory_id = entities
            .spawn_building(1, EntityKind::TankFactory, fx, fy, true)
            .unwrap();
        let (nx, ny) = map.tile_center(20, 16);
        entities
            .spawn_building(1, EntityKind::Depot, nx, ny, true)
            .unwrap();
        let occ = Occupancy::build(&map, &entities);

        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        let b = entities.get(factory_id).unwrap();
        let (sx, sy) =
            coordinator.find_spawn_point(&entities, b.kind, EntityKind::Tank, b.pos_x, b.pos_y);

        assert!(
            spawn_point_has_clearance(
                &map,
                &occ,
                sx,
                sy,
                config::unit_stats(EntityKind::Tank).unwrap().radius,
            ),
            "tank spawn point is too close to the adjacent building"
        );
    }

    #[test]
    fn repath_throttle_respects_min_ticks_and_material_goal_change() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        let id = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .unwrap();
        let occ = Occupancy::build(&map, &entities);

        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(10);
        let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 10);

        // Fresh unit: should be allowed to repath.
        assert!(coordinator.can_repath(&entities, id, (200.0, 200.0)));

        // Simulate a recent repath at tick 10.
        if let Some(e) = entities.get_mut(id) {
            e.set_last_repath_tick(10);
            e.set_path_goal(Some((200.0, 200.0)));
        }

        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(11);
        let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 11);

        // Only 1 tick elapsed, goal unchanged: should NOT repath.
        assert!(!coordinator.can_repath(&entities, id, (200.0, 200.0)));

        // Goal moved materially (> TILE_SIZE): should bypass throttle.
        assert!(
            coordinator.can_repath(&entities, id, (250.0, 250.0)),
            "material goal change should bypass throttle"
        );

        // 3 ticks elapsed, goal unchanged: should now be allowed (MIN_REPATH_TICKS = 3).
        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(13);
        let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 13);
        assert!(
            coordinator.can_repath(&entities, id, (200.0, 200.0)),
            "3+ ticks elapsed should allow repath"
        );
    }

    #[test]
    fn path_failed_is_set_on_unreachable_goal() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        // Place the unit at tile (10, 10).
        let (ux, uy) = map.tile_center(10, 10);
        let id = entities
            .spawn_unit(1, EntityKind::Rifleman, ux, uy)
            .unwrap();

        // Completely surround tile (10, 10) with a ring of 2x2 depots so the unit
        // cannot leave.  Depots centered at tile-centers that keep (10,10) open.
        let ring = [(8.0, 10.0), (12.0, 10.0), (10.0, 8.0), (10.0, 12.0)];
        for &(cx, cy) in &ring {
            let (wx, wy) = map.tile_center(cx as u32, cy as u32);
            entities.spawn_building(1, EntityKind::Depot, wx, wy, true);
        }

        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        // Order the unit to move far away (to tile (30, 30)).
        let (gx, gy) = map.tile_center(30, 30);
        coordinator.order_group_move(&mut entities, 1, &[id], (gx, gy), false);

        // The unit should be in AwaitingPath after the order.
        let e = entities.get(id).unwrap();
        assert_eq!(e.move_phase(), Some(MovePhase::AwaitingPath));

        // Process awaiting paths.
        coordinator.process_awaiting_paths(&mut entities);

        // The unit is fully enclosed, so no route exists → PathFailed.
        let e = entities.get(id).unwrap();
        assert_eq!(e.move_phase(), Some(MovePhase::PathFailed));
        assert!(e.path_is_empty());
    }

    #[test]
    fn process_awaiting_paths_respects_budget() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        // Spawn many units and order them all to move.
        let mut ids = Vec::new();
        for i in 0..10 {
            let x = 32.0 + i as f32 * 32.0;
            let id = entities
                .spawn_unit(1, EntityKind::Rifleman, x, 100.0)
                .unwrap();
            ids.push(id);
        }

        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        // Set budget artificially low (3).
        coordinator.budget = 3;

        // Mark all units as awaiting path with a Move order.
        for &id in &ids {
            if let Some(e) = entities.get_mut(id) {
                e.set_order(Order::move_to(500.0, 500.0));
                e.set_path_goal(Some((500.0, 500.0)));
                e.mark_move_phase(MovePhase::AwaitingPath);
            }
        }

        coordinator.process_awaiting_paths(&mut entities);

        // Count how many moved from AwaitingPath to Moving/PathFailed.
        let mut processed = 0;
        let mut still_waiting = 0;
        for &id in &ids {
            let e = entities.get(id).unwrap();
            match e.move_phase() {
                Some(MovePhase::Moving) | Some(MovePhase::PathFailed) => processed += 1,
                Some(MovePhase::AwaitingPath) => still_waiting += 1,
                _ => {}
            }
        }

        assert_eq!(
            processed, 3,
            "only 3 paths should have been processed with budget=3"
        );
        assert_eq!(still_waiting, 7, "7 units should still be awaiting path");
    }
}
