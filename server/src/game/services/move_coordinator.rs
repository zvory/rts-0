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
use crate::game::entity::{
    uses_oriented_vehicle_body, EntityKind, EntityStore, MovePhase, Order, WeaponSetup,
};
use crate::game::map::Map;
use crate::game::pathfinding::{self, Passability};
use crate::game::services::geometry::{
    building_rect_for_entity, unit_bodies_intersect, unit_body, unit_body_for_entity,
    unit_body_with_facing, RectBody, UnitBody,
};
use crate::game::services::interact_range_for_kind;
use crate::game::services::occupancy::{
    building_footprint, footprint_center, footprint_tiles, Occupancy,
};
use crate::game::services::pathing::{
    simplify_reverse_waypoints_with_limit, PathRequest, PathingService, RouteShape,
};
use crate::game::services::standability;

/// Maximum number of fresh A* path requests serviced in a single tick. Beyond this,
/// remaining `AwaitingPath` units stay queued for the next tick.
const MAX_REQUESTS_PER_TICK: usize = 64;

/// Minimum ticks between repaths for a single unit. Prevents chase/gather spam.
const MIN_REPATH_TICKS: u32 = 3;

/// If the goal moves by more than this many world pixels, bypass the repath throttle.
const MATERIAL_GOAL_DELTA_PX: f32 = config::TILE_SIZE as f32;

const FORMATION_NEAR_DISTANCE_PX: f32 = config::TILE_SIZE as f32 * 4.0;
const FORMATION_FAR_DISTANCE_PX: f32 = config::TILE_SIZE as f32 * 18.0;
const FORMATION_MAX_OFFSET_PX: f32 = config::TILE_SIZE as f32 * 4.0;
const SPAWN_PREFERRED_GAP_UNIT_FRACTION: f32 = 0.10;
const SCOUT_CAR_ROUTE_SIMPLIFY_MAX_SEGMENT_PX: f32 = config::TILE_SIZE as f32 * 3.0;

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
        let units: Vec<FormationUnit> = ids
            .iter()
            .filter_map(|&id| {
                let e = entities.get(id)?;
                (e.is_unit() && e.owner == player).then_some(FormationUnit {
                    id,
                    kind: e.kind,
                    pos: (e.pos_x, e.pos_y),
                })
            })
            .collect();
        if units.is_empty() {
            return;
        }
        let goals = formation_goals(self.map, self.occ, &units, goal);

        for (unit, g) in units.iter().zip(goals.iter()) {
            entities.release_miner(unit.id);
            let Some(e) = entities.get_mut(unit.id) else {
                continue;
            };
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
            begin_deployed_weapon_teardown(e);
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
            // An explicit attack order is not necessarily a move command for a deployed weapon:
            // it may be able to slew and fire immediately. Combat requests a chase path only
            // if the target is actually out of range, after teardown if needed.
            request_initial_path = !requires_weapon_setup(e.kind);
            let (px, py) = (e.pos_x, e.pos_y);
            e.reset_stuck(px, py);
        }
        if request_initial_path {
            self.request_path(entities, id, (tx, ty), false);
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
        self.request_path(entities, id, (nx, ny), false);
    }

    /// Issue a build order: record the placement intent on the worker and walk it to an outside
    /// staging tile. No building is spawned and no resources are deducted here; that happens on
    /// arrival in the construction system.
    pub fn order_build(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        kind: EntityKind,
        tile_x: u32,
        tile_y: u32,
    ) -> bool {
        entities.release_miner(id);
        if let Some(e) = entities.get_mut(id) {
            e.set_order(Order::build(kind, tile_x, tile_y));
            e.set_target_id(None);
            e.set_path(Vec::new());
            e.reset_gather_state();
            let (px, py) = (e.pos_x, e.pos_y);
            e.reset_stuck(px, py);
        }
        if self.request_build_path(entities, id, kind, tile_x, tile_y) {
            return true;
        }
        for goal in build_staging_goals(self.map, self.occ, entities, id, kind, tile_x, tile_y) {
            if self.request_exact_path_to_build_goal(entities, id, goal) {
                return true;
            }
        }
        if let Some(e) = entities.get_mut(id) {
            e.clear_orders();
        }
        false
    }

    // -------------------------------------------------------------------
    // Tick-scoped bulk processing
    // -------------------------------------------------------------------

    /// Process all units currently in `MovePhase::AwaitingPath` in deterministic entity-id
    /// order, assigning paths up to the tick budget. Units that can't be serviced this tick
    /// remain `AwaitingPath`; units that fail to get any route are marked `PathFailed`.
    pub fn process_awaiting_paths(&mut self, entities: &mut EntityStore) {
        let waiting: Vec<u32> = entities
            .iter()
            .filter(|e| e.is_unit() && e.move_phase() == Some(MovePhase::AwaitingPath))
            .map(|e| e.id)
            .collect();

        for id in waiting {
            if self.budget == 0 {
                break;
            }
            let goal = match entities.get(id).and_then(|e| e.path_goal()) {
                Some(g) => g,
                None => continue,
            };
            self.request_path(entities, id, goal, true);
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
        self.request_path(entities, id, target_pos, false)
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
        self.request_path(entities, id, node_pos, false)
    }

    // -------------------------------------------------------------------
    // Spawn search
    // -------------------------------------------------------------------

    /// Find a spawn point near a building using a deterministic outward search.
    /// Returns `None` when no legal body-clearance point exists.
    ///
    /// Prefer points with a small clearance gap from the producing building so spawned units have
    /// room to move away. If no such point exists, fall back to the first legal ring so tight maps
    /// do not block production. When `rally` is `Some`, candidate ties within a ring favor the
    /// point closest to the rally so units still exit the side of the building facing it.
    pub fn find_spawn_point(
        &self,
        entities: &EntityStore,
        building: u32,
        spawned_kind: EntityKind,
        rally: Option<(f32, f32)>,
    ) -> Option<(f32, f32)> {
        let building = entities.get(building)?;
        config::building_stats(building.kind)?;
        let spawned_stats = config::unit_stats(spawned_kind)?;
        let building_rect = building_rect_for_entity(self.map, building)?;
        let preferred_gap = spawned_stats.radius * SPAWN_PREFERRED_GAP_UNIT_FRACTION;
        let footprint = building_footprint(self.map, building);
        let min_x = footprint.iter().map(|(x, _)| *x).min()? as i32;
        let max_x = footprint.iter().map(|(x, _)| *x).max()? as i32;
        let min_y = footprint.iter().map(|(_, y)| *y).min()? as i32;
        let max_y = footprint.iter().map(|(_, y)| *y).max()? as i32;

        // Search outward in rings from the actual building footprint edge.
        let mut fallback: Option<(f32, (f32, f32))> = None;
        for r in 1i32..=6 {
            let mut ring_best: Option<(f32, (f32, f32))> = None;
            let mut preferred_ring_best: Option<(f32, (f32, f32))> = None;
            for ty in (min_y - r)..=(max_y + r) {
                for tx in (min_x - r)..=(max_x + r) {
                    if tx > min_x - r && tx < max_x + r && ty > min_y - r && ty < max_y + r {
                        continue;
                    }
                    if !self.map.in_bounds(tx, ty) {
                        continue;
                    }
                    let (cx, cy) = self.map.tile_center(tx as u32, ty as u32);
                    if !standability::unit_spawn_standable(
                        self.map,
                        self.occ,
                        entities,
                        spawned_kind,
                        cx,
                        cy,
                    ) {
                        continue;
                    }
                    let score = rally.map_or(0.0, |(rx, ry)| (cx - rx).powi(2) + (cy - ry).powi(2));
                    if ring_best.is_none_or(|(best_score, _)| score < best_score) {
                        ring_best = Some((score, (cx, cy)));
                    }
                    if spawn_gap_from_building(spawned_kind, cx, cy, building_rect)
                        .is_some_and(|gap| gap >= preferred_gap)
                        && preferred_ring_best.is_none_or(|(best_score, _)| score < best_score)
                    {
                        preferred_ring_best = Some((score, (cx, cy)));
                    }
                }
            }
            if fallback.is_none() {
                fallback = ring_best;
            }
            if let Some((_, point)) = preferred_ring_best {
                return Some(point);
            }
        }

        fallback.map(|(_, point)| point)
    }

    /// Prefer spawning oriented vehicles already facing the rally, but only if that body
    /// orientation is legal at the chosen spawn point.
    pub fn rally_spawn_facing(
        &self,
        entities: &EntityStore,
        spawned_kind: EntityKind,
        spawn: (f32, f32),
        rally: (f32, f32),
    ) -> Option<f32> {
        if !uses_oriented_vehicle_body(spawned_kind) {
            return None;
        }

        let dx = rally.0 - spawn.0;
        let dy = rally.1 - spawn.1;
        let facing = dy.atan2(dx);
        if !facing.is_finite()
            || !standability::unit_static_standable_with_facing(
                self.map,
                self.occ,
                spawned_kind,
                spawn.0,
                spawn.1,
                facing,
            )
        {
            return None;
        }

        let body = unit_body_with_facing(spawned_kind, spawn.0, spawn.1, facing)?;
        entities
            .iter()
            .all(|e| {
                e.hp == 0
                    || !e.is_unit()
                    || unit_body_for_entity(e)
                        .is_none_or(|existing| !unit_bodies_intersect(body, existing))
            })
            .then_some(facing)
    }

    // -------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------

    /// Direct path request without throttle check. Updates budget, entity path, and phase.
    fn request_path(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        goal: (f32, f32),
        smooth_static_segments: bool,
    ) -> bool {
        let ((sx, sy), kind, start_pos) = match entities.get(id) {
            Some(e) => (
                self.map.tile_of(e.pos_x, e.pos_y),
                e.kind,
                (e.pos_x, e.pos_y),
            ),
            None => return false,
        };
        let (gx, gy) = self.map.tile_of(goal.0, goal.1);
        let radius_tiles = config::unit_radius_tiles(kind);
        let route_shape = if smooth_static_segments && uses_oriented_vehicle_body(kind) {
            RouteShape::ScoutCarClearance
        } else {
            RouteShape::Normal
        };
        let req = PathRequest {
            kind,
            start: (sx as i32, sy as i32),
            goal: (gx as i32, gy as i32),
            radius_tiles,
            route_shape,
            budget: None,
        };
        let mut waypoints = self.pathing.request(self.map, self.occ, req);

        // Snap the final waypoint to the exact requested goal for precise arrival.
        if !waypoints.is_empty() {
            waypoints[0] = goal;
            if route_shape == RouteShape::ScoutCarClearance {
                waypoints = simplify_reverse_waypoints_with_limit(
                    self.map,
                    self.occ,
                    kind,
                    start_pos,
                    waypoints,
                    SCOUT_CAR_ROUTE_SIMPLIFY_MAX_SEGMENT_PX,
                );
            }
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

    fn request_build_path(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        kind: EntityKind,
        tile_x: u32,
        tile_y: u32,
    ) -> bool {
        let footprint = footprint_tiles(kind, tile_x, tile_y);
        if footprint.is_empty() {
            return false;
        }
        let footprint_set: std::collections::BTreeSet<(u32, u32)> =
            footprint.iter().copied().collect();
        if let Some(goal) = current_staging_goal(self.map, entities, id, kind, &footprint_set) {
            set_entity_path(entities, id, Vec::new(), goal, self.tick);
            return true;
        }

        let approach_goal = self.map.tile_center(tile_x, tile_y);
        let Some(tile_path) = self.request_exact_tile_path(entities, id, approach_goal) else {
            return false;
        };
        let Some(staging_index) = tile_path.iter().rposition(|(tx, ty)| {
            *tx >= 0 && *ty >= 0 && !footprint_set.contains(&(*tx as u32, *ty as u32))
        }) else {
            return false;
        };
        let staging_tile = tile_path[staging_index];
        let goal = self
            .map
            .tile_center(staging_tile.0 as u32, staging_tile.1 as u32);
        if !build_staging_goal_in_range(self.map, kind, tile_x, tile_y, goal) {
            return false;
        }
        let trimmed = tile_path[..=staging_index].to_vec();
        let waypoints = pathfinding::to_world_waypoints(&trimmed);
        set_entity_path(entities, id, waypoints, goal, self.tick);
        true
    }

    fn request_exact_path_to_build_goal(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        goal: (f32, f32),
    ) -> bool {
        let Some(tile_path) = self.request_exact_tile_path(entities, id, goal) else {
            return false;
        };
        let waypoints = pathfinding::to_world_waypoints(&tile_path);
        set_entity_path(entities, id, waypoints, goal, self.tick);
        true
    }

    fn request_exact_tile_path(
        &mut self,
        entities: &EntityStore,
        id: u32,
        goal: (f32, f32),
    ) -> Option<Vec<(i32, i32)>> {
        if self.budget == 0 {
            return None;
        }
        let (unit_kind, sx, sy) = match entities.get(id) {
            Some(e) if e.is_unit() => {
                let (sx, sy) = self.map.tile_of(e.pos_x, e.pos_y);
                (e.kind, sx, sy)
            }
            _ => return None,
        };
        let (gx, gy) = self.map.tile_of(goal.0, goal.1);
        let radius_tiles = config::unit_radius_tiles(unit_kind);
        let req = PathRequest {
            kind: unit_kind,
            start: (sx as i32, sy as i32),
            goal: (gx as i32, gy as i32),
            radius_tiles,
            route_shape: RouteShape::Normal,
            budget: None,
        };
        let tile_path = self.pathing.request_tile_path(self.map, self.occ, req);
        self.budget = self.budget.saturating_sub(1);
        if tile_path.last().copied() == Some((gx as i32, gy as i32)) {
            Some(tile_path)
        } else {
            None
        }
    }
}

fn spawn_gap_from_building(
    spawned_kind: EntityKind,
    x: f32,
    y: f32,
    building_rect: RectBody,
) -> Option<f32> {
    let body = unit_body(spawned_kind, x, y)?;
    Some(unit_body_rect_gap(body, building_rect))
}

fn unit_body_rect_gap(body: UnitBody, rect: RectBody) -> f32 {
    match body {
        UnitBody::Circle(circle) => {
            let nearest_x = circle.x.clamp(rect.min_x, rect.max_x);
            let nearest_y = circle.y.clamp(rect.min_y, rect.max_y);
            let dx = circle.x - nearest_x;
            let dy = circle.y - nearest_y;
            ((dx * dx + dy * dy).sqrt() - circle.radius).max(0.0)
        }
        UnitBody::OrientedCapsule(_) | UnitBody::OrientedBox(_) => {
            let aabb = body.aabb();
            let dx = if aabb.max_x < rect.min_x {
                rect.min_x - aabb.max_x
            } else if rect.max_x < aabb.min_x {
                aabb.min_x - rect.max_x
            } else {
                0.0
            };
            let dy = if aabb.max_y < rect.min_y {
                rect.min_y - aabb.max_y
            } else if rect.max_y < aabb.min_y {
                aabb.min_y - rect.max_y
            } else {
                0.0
            };
            (dx * dx + dy * dy).sqrt()
        }
    }
}

fn set_entity_path(
    entities: &mut EntityStore,
    id: u32,
    path: Vec<(f32, f32)>,
    goal: (f32, f32),
    tick: u32,
) {
    if let Some(e) = entities.get_mut(id) {
        e.set_path(path);
        e.set_last_repath_tick(tick);
        e.set_path_goal(Some(goal));
    }
}

fn current_staging_goal(
    map: &Map,
    entities: &EntityStore,
    id: u32,
    kind: EntityKind,
    footprint: &std::collections::BTreeSet<(u32, u32)>,
) -> Option<(f32, f32)> {
    let worker = entities.get(id)?;
    let tile = map.tile_of(worker.pos_x, worker.pos_y);
    if footprint.contains(&tile) {
        return None;
    }
    let &(tile_x, tile_y) = footprint.iter().min()?;
    let goal = (worker.pos_x, worker.pos_y);
    build_staging_goal_in_range(map, kind, tile_x, tile_y, goal).then_some(goal)
}

fn build_staging_goal_in_range(
    map: &Map,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
    goal: (f32, f32),
) -> bool {
    let (cx, cy) = footprint_center(map, kind, tile_x, tile_y);
    let dx = goal.0 - cx;
    let dy = goal.1 - cy;
    dx * dx + dy * dy <= interact_range_for_kind(kind).powi(2)
}

fn begin_deployed_weapon_teardown(e: &mut crate::game::entity::Entity) {
    if !requires_weapon_setup(e.kind) {
        return;
    }
    if !matches!(e.weapon_setup(), WeaponSetup::Packed) {
        let ticks = match e.kind {
            EntityKind::AtTeam => config::AT_TEAM_SETUP_TICKS,
            _ => config::MACHINE_GUNNER_SETUP_TICKS,
        };
        e.set_weapon_setup(WeaponSetup::TearingDown { ticks });
    }
}

fn requires_weapon_setup(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::MachineGunner | EntityKind::AtTeam)
}

// ---------------------------------------------------------------------------
// Goal spreading
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct FormationUnit {
    id: u32,
    kind: EntityKind,
    pos: (f32, f32),
}

/// Spread unit goals around the requested anchor tile. Returns one goal world point per unit
/// in the same order as `ids`.
fn spread_goals(
    map: &Map,
    occ: &Occupancy,
    units: &[FormationUnit],
    anchor: (u32, u32),
) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(units.len());
    let mut taken: Vec<(u32, u32)> = Vec::new();

    for unit in units {
        if let Some(tile) = find_unique_tile_near(map, occ, unit, anchor, &taken) {
            taken.push(tile);
            out.push(map.tile_center(tile.0, tile.1));
        } else {
            out.push(unit.pos);
        }
    }

    out
}

/// Assign formation-aware path goals in the same order as `units`.
fn formation_goals(
    map: &Map,
    occ: &Occupancy,
    units: &[FormationUnit],
    goal: (f32, f32),
) -> Vec<(f32, f32)> {
    if units.len() <= 1 {
        let anchor = map.tile_of(goal.0, goal.1);
        return spread_goals(map, occ, units, anchor);
    }

    let inv_count = 1.0 / units.len() as f32;
    let centroid = units.iter().fold((0.0f32, 0.0f32), |acc, unit| {
        (
            acc.0 + unit.pos.0 * inv_count,
            acc.1 + unit.pos.1 * inv_count,
        )
    });
    let dx = goal.0 - centroid.0;
    let dy = goal.1 - centroid.1;
    let move_distance = (dx * dx + dy * dy).sqrt();
    let formation_scale = formation_scale_for_distance(move_distance);
    let max = map.world_size_px() - 1.0;
    let mut out = Vec::with_capacity(units.len());
    let mut taken: Vec<(u32, u32)> = Vec::new();

    for unit in units {
        let offset = clamp_offset(
            unit.pos.0 - centroid.0,
            unit.pos.1 - centroid.1,
            FORMATION_MAX_OFFSET_PX,
        );
        let desired = (
            (goal.0 + offset.0 * formation_scale).clamp(0.0, max),
            (goal.1 + offset.1 * formation_scale).clamp(0.0, max),
        );
        let anchor = map.tile_of(desired.0, desired.1);
        if let Some(tile) = find_unique_tile_near(map, occ, unit, anchor, &taken) {
            taken.push(tile);
            out.push(map.tile_center(tile.0, tile.1));
        } else {
            out.push(unit.pos);
        }
    }

    out
}

fn formation_scale_for_distance(distance: f32) -> f32 {
    let t = ((distance - FORMATION_NEAR_DISTANCE_PX)
        / (FORMATION_FAR_DISTANCE_PX - FORMATION_NEAR_DISTANCE_PX))
        .clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn clamp_offset(dx: f32, dy: f32, max_len: f32) -> (f32, f32) {
    let len = (dx * dx + dy * dy).sqrt();
    if len <= max_len || len <= f32::EPSILON {
        return (dx, dy);
    }
    let scale = max_len / len;
    (dx * scale, dy * scale)
}

/// Search outward from `anchor` in deterministic ring order and return the first body-standable
/// tile not already in `taken`.
fn find_unique_tile_near(
    map: &Map,
    occ: &Occupancy,
    unit: &FormationUnit,
    anchor: (u32, u32),
    taken: &[(u32, u32)],
) -> Option<(u32, u32)> {
    // Try the anchor first.
    if is_free_goal(map, occ, unit, anchor, taken) {
        return Some(anchor);
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
                if is_free_goal(map, occ, unit, t, taken) {
                    return Some(t);
                }
            }
        }
    }

    None
}

fn is_free_goal(
    map: &Map,
    occ: &Occupancy,
    unit: &FormationUnit,
    tile: (u32, u32),
    taken: &[(u32, u32)],
) -> bool {
    if !map.is_passable(tile.0 as i32, tile.1 as i32) {
        return false;
    }
    if !occ.passable(tile.0 as i32, tile.1 as i32) {
        return false;
    }
    if taken.contains(&tile) {
        return false;
    }
    let center = map.tile_center(tile.0, tile.1);
    let facing = formation_goal_facing(unit, center);
    standability::unit_static_standable_with_facing(map, occ, unit.kind, center.0, center.1, facing)
}

fn formation_goal_facing(unit: &FormationUnit, center: (f32, f32)) -> f32 {
    if !uses_oriented_vehicle_body(unit.kind) {
        return 0.0;
    }
    let dx = center.0 - unit.pos.0;
    let dy = center.1 - unit.pos.1;
    let dist2 = dx * dx + dy * dy;
    if !dist2.is_finite() || dist2 <= 1.0e-4 {
        0.0
    } else {
        dy.atan2(dx)
    }
}

/// Pick a walk target outside a build footprint.
///
/// Construction starts when the worker is close enough to the footprint center, so walking to an
/// outside perimeter tile keeps the builder from ending up inside the completed building.
/// Returns `None` when no outside staging tile is available.
#[cfg(test)]
pub(crate) fn build_staging_goal(
    map: &Map,
    occ: &Occupancy,
    entities: &EntityStore,
    worker: u32,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> Option<(f32, f32)> {
    build_staging_goals(map, occ, entities, worker, kind, tile_x, tile_y)
        .into_iter()
        .next()
}

fn build_staging_goals(
    map: &Map,
    occ: &Occupancy,
    entities: &EntityStore,
    worker: u32,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> Vec<(f32, f32)> {
    let Some(worker) = entities.get(worker) else {
        return Vec::new();
    };
    let footprint = footprint_tiles(kind, tile_x, tile_y);
    let Some(stats) = config::building_stats(kind) else {
        return Vec::new();
    };
    if footprint.is_empty() {
        return Vec::new();
    }
    let footprint_set: std::collections::BTreeSet<(u32, u32)> = footprint.iter().copied().collect();
    let min_x = tile_x as i32;
    let min_y = tile_y as i32;
    let Some(max_x) = tile_x.checked_add(stats.foot_w.saturating_sub(1)) else {
        return Vec::new();
    };
    let Some(max_y) = tile_y.checked_add(stats.foot_h.saturating_sub(1)) else {
        return Vec::new();
    };
    let max_x = max_x as i32;
    let max_y = max_y as i32;
    let mut candidates = Vec::new();

    // Search outward from the footprint, then order candidates by ring and worker distance.
    for r in 1i32..=6 {
        for ty in (min_y - r)..=(max_y + r) {
            for tx in (min_x - r)..=(max_x + r) {
                if tx > min_x - r && tx < max_x + r && ty > min_y - r && ty < max_y + r {
                    continue;
                }
                if !map.in_bounds(tx, ty) {
                    continue;
                }
                let tile = (tx as u32, ty as u32);
                if footprint_set.contains(&tile) {
                    continue;
                }
                if !map.is_passable(tx, ty) {
                    continue;
                }
                if !occ.passable(tx, ty) {
                    continue;
                }
                let center = map.tile_center(tile.0, tile.1);
                if !build_staging_goal_in_range(map, kind, tile_x, tile_y, center) {
                    continue;
                }
                let dist2 = {
                    let dx = worker.pos_x - center.0;
                    let dy = worker.pos_y - center.1;
                    dx * dx + dy * dy
                };
                candidates.push((r, dist2, tile));
            }
        }
    }
    candidates.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    candidates
        .into_iter()
        .map(|(_, _, tile)| map.tile_center(tile.0, tile.1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore, MovePhase, Order};
    use crate::game::map::Map;
    use crate::game::services::occupancy::Occupancy;
    use crate::protocol::terrain;

    fn formation_unit(id: u32, map: &Map, tile: (u32, u32)) -> FormationUnit {
        formation_unit_kind(id, EntityKind::Rifleman, map, tile)
    }

    fn formation_unit_kind(
        id: u32,
        kind: EntityKind,
        map: &Map,
        tile: (u32, u32),
    ) -> FormationUnit {
        FormationUnit {
            id,
            kind,
            pos: map.tile_center(tile.0, tile.1),
        }
    }

    fn square_formation(map: &Map) -> Vec<FormationUnit> {
        vec![
            formation_unit(1, map, (8, 63)),
            formation_unit(2, map, (12, 63)),
            formation_unit(3, map, (8, 67)),
            formation_unit(4, map, (12, 67)),
        ]
    }

    fn offset_from(point: (f32, f32), origin: (f32, f32)) -> (f32, f32) {
        (point.0 - origin.0, point.1 - origin.1)
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.01,
            "expected {actual:.2} to be close to {expected:.2}"
        );
    }

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![],
            expansion_sites: vec![],
        }
    }

    fn impassable_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::WATER; (size * size) as usize],
            starts: vec![],
            expansion_sites: vec![],
        }
    }

    fn set_passable(map: &mut Map, tx: u32, ty: u32) {
        map.terrain[(ty * map.size + tx) as usize] = terrain::GRASS;
    }

    #[test]
    fn near_group_move_compacts_goals_near_click() {
        let map = Map::generate(1, 0x1234_5678);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let units = square_formation(&map);
        let click = map.tile_center(11, 65);

        let goals = formation_goals(&map, &occ, &units, click);

        assert_eq!(goals.len(), units.len());
        for goal in goals {
            let dx = goal.0 - click.0;
            let dy = goal.1 - click.1;
            let dist = (dx * dx + dy * dy).sqrt();
            assert!(
                dist <= config::TILE_SIZE as f32 * 1.5,
                "near goals should stay clustered around the click, got {goal:?}"
            );
        }
    }

    #[test]
    fn far_group_move_preserves_world_offsets() {
        let map = Map::generate(1, 0x1234_5678);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let units = square_formation(&map);
        let click = map.tile_center(30, 65);

        let goals = formation_goals(&map, &occ, &units, click);

        let ts = config::TILE_SIZE as f32;
        let expected = [
            (-2.0 * ts, -2.0 * ts),
            (2.0 * ts, -2.0 * ts),
            (-2.0 * ts, 2.0 * ts),
            (2.0 * ts, 2.0 * ts),
        ];
        for (goal, expected_offset) in goals.iter().zip(expected) {
            let actual = offset_from(*goal, click);
            assert_close(actual.0, expected_offset.0);
            assert_close(actual.1, expected_offset.1);
        }
    }

    #[test]
    fn far_scattered_group_move_caps_preserved_offsets() {
        let map = flat_map(80);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let units = vec![
            formation_unit(1, &map, (5, 20)),
            formation_unit(2, &map, (45, 20)),
        ];
        let click = map.tile_center(60, 50);

        let goals = formation_goals(&map, &occ, &units, click);

        let ts = config::TILE_SIZE as f32;
        let expected = [(-4.0 * ts, 0.0), (4.0 * ts, 0.0)];
        for (goal, expected_offset) in goals.iter().zip(expected) {
            let actual = offset_from(*goal, click);
            assert_close(actual.0, expected_offset.0);
            assert_close(actual.1, expected_offset.1);
        }
    }

    #[test]
    fn medium_group_move_blends_offsets() {
        let map = Map::generate(1, 0x1234_5678);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let units = square_formation(&map);
        let click = map.tile_center(21, 65);

        let goals = formation_goals(&map, &occ, &units, click);

        let ts = config::TILE_SIZE as f32;
        let expected = [(-ts, -ts), (ts, -ts), (-ts, ts), (ts, ts)];
        for (goal, expected_offset) in goals.iter().zip(expected) {
            let actual = offset_from(*goal, click);
            assert_close(actual.0, expected_offset.0);
            assert_close(actual.1, expected_offset.1);
        }
    }

    #[test]
    fn formation_goals_are_unique_when_tiles_are_free() {
        let map = Map::generate(1, 0x1234_5678);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let units = square_formation(&map);
        let click = map.tile_center(11, 65);

        let first = formation_goals(&map, &occ, &units, click);
        let second = formation_goals(&map, &occ, &units, click);

        assert_eq!(
            first, second,
            "formation assignment should be deterministic"
        );
        let mut seen = std::collections::HashSet::new();
        for goal in first {
            let tile = map.tile_of(goal.0, goal.1);
            assert!(
                seen.insert(tile),
                "duplicate goal tile {tile:?} for multi-unit formation"
            );
        }
    }

    #[test]
    fn blocked_formation_slot_falls_back_to_nearby_passable_tile() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        let blocked_tile = (28, 63);
        let blocked_center = map.tile_center(blocked_tile.0, blocked_tile.1);
        entities
            .spawn_building(
                1,
                EntityKind::Depot,
                blocked_center.0,
                blocked_center.1,
                true,
            )
            .unwrap();
        let occ = Occupancy::build(&map, &entities);
        let units = square_formation(&map);
        let click = map.tile_center(30, 65);

        let goals = formation_goals(&map, &occ, &units, click);
        let first_tile = map.tile_of(goals[0].0, goals[0].1);

        assert_ne!(
            first_tile, blocked_tile,
            "blocked desired formation slot should not be assigned"
        );
        assert_eq!(
            first_tile,
            (29, 62),
            "nearby fallback should use deterministic ring order"
        );
        assert!(map.is_passable(first_tile.0 as i32, first_tile.1 as i32));
        assert!(occ.passable(first_tile.0 as i32, first_tile.1 as i32));
    }

    #[test]
    fn formation_goal_accepts_side_on_tank_tile_center_near_building() {
        let mut map = Map::generate(1, 0x1234_5678);
        for tile in &mut map.terrain {
            *tile = crate::protocol::terrain::GRASS;
        }
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Depot, 10, 10);
        entities
            .spawn_building(1, EntityKind::Depot, bx, by, true)
            .expect("depot should spawn");
        let occ = Occupancy::build(&map, &entities);
        let units = vec![formation_unit_kind(1, EntityKind::Tank, &map, (12, 6))];
        let adjacent_tile_goal = map.tile_center(12, 10);

        let goals = formation_goals(&map, &occ, &units, adjacent_tile_goal);
        let goal = goals[0];

        assert_eq!(
            map.tile_of(goal.0, goal.1),
            (12, 10),
            "side-on tank hull should allow adjacent tile-center formation slots"
        );
        let facing = formation_goal_facing(&units[0], goal);
        assert!(
            standability::unit_static_standable_with_facing(
                &map,
                &occ,
                EntityKind::Tank,
                goal.0,
                goal.1,
                facing,
            ),
            "assigned formation goal must be body-standable"
        );
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

        let (sx, sy) = coordinator
            .find_spawn_point(&entities, b_id, EntityKind::Tank, None)
            .expect("spawn point should exist");

        let (stx, sty) = map.tile_of(sx, sy);
        let footprint = building_footprint(&map, entities.get(b_id).unwrap());

        assert!(
            !footprint.contains(&(stx, sty)),
            "spawn tile ({stx},{sty}) is inside the barracks footprint {footprint:?}"
        );

        assert!(map.is_passable(stx as i32, sty as i32));
    }

    #[test]
    fn tank_spawn_point_keeps_clear_of_top_map_edge() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        let (bx, by) = map.tile_center(3, 0);
        let b_id = entities
            .spawn_building(1, EntityKind::Factory, bx, by, true)
            .unwrap();
        let occ = Occupancy::build(&map, &entities);

        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        let (sx, sy) = coordinator
            .find_spawn_point(&entities, b_id, EntityKind::Tank, None)
            .expect("spawn point should exist");

        assert!(
            standability::unit_spawn_standable(&map, &occ, &entities, EntityKind::Tank, sx, sy,),
            "tank spawn point clips the top map edge"
        );
    }

    #[test]
    fn tank_spawn_point_keeps_clear_of_adjacent_building() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        let (fx, fy) = map.tile_center(16, 16);
        let factory_id = entities
            .spawn_building(1, EntityKind::Factory, fx, fy, true)
            .unwrap();
        let (nx, ny) = map.tile_center(20, 16);
        entities
            .spawn_building(1, EntityKind::Depot, nx, ny, true)
            .unwrap();
        let occ = Occupancy::build(&map, &entities);

        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        let (sx, sy) = coordinator
            .find_spawn_point(&entities, factory_id, EntityKind::Tank, None)
            .expect("spawn point should exist");

        assert!(
            standability::unit_spawn_standable(&map, &occ, &entities, EntityKind::Tank, sx, sy,),
            "tank spawn point is too close to the adjacent building"
        );
    }

    #[test]
    fn tank_spawn_point_prefers_gap_from_producer_when_available() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (fx, fy) = footprint_center(&map, EntityKind::Factory, 10, 10);
        let factory_id = entities
            .spawn_building(1, EntityKind::Factory, fx, fy, true)
            .expect("factory should spawn");
        let occ = Occupancy::build(&map, &entities);

        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        let (sx, sy) = coordinator
            .find_spawn_point(&entities, factory_id, EntityKind::Tank, None)
            .expect("spawn point should exist");
        let factory = entities.get(factory_id).expect("factory");
        let rect = building_rect_for_entity(&map, factory).expect("factory rect");
        let gap = spawn_gap_from_building(EntityKind::Tank, sx, sy, rect).expect("tank body");
        let preferred = config::unit_stats(EntityKind::Tank)
            .expect("tank stats")
            .radius
            * SPAWN_PREFERRED_GAP_UNIT_FRACTION;

        assert!(
            gap >= preferred,
            "tank spawn should prefer at least {preferred:.2}px of building clearance, got {gap:.2}px"
        );
    }

    #[test]
    fn tank_spawn_point_falls_back_to_tight_exit_when_no_gap_candidate_exists() {
        let mut map = impassable_map(12);
        let mut entities = EntityStore::new();
        let (fx, fy) = footprint_center(&map, EntityKind::Factory, 4, 4);
        let factory_id = entities
            .spawn_building(1, EntityKind::Factory, fx, fy, true)
            .expect("factory should spawn");
        for tx in 4..=6 {
            set_passable(&mut map, tx, 3);
        }
        let occ = Occupancy::build(&map, &entities);

        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        let (sx, sy) = coordinator
            .find_spawn_point(&entities, factory_id, EntityKind::Tank, None)
            .expect("tight spawn point should still be allowed");
        let factory = entities.get(factory_id).expect("factory");
        let rect = building_rect_for_entity(&map, factory).expect("factory rect");
        let gap = spawn_gap_from_building(EntityKind::Tank, sx, sy, rect).expect("tank body");
        let preferred = config::unit_stats(EntityKind::Tank)
            .expect("tank stats")
            .radius
            * SPAWN_PREFERRED_GAP_UNIT_FRACTION;

        assert_eq!(
            map.tile_of(sx, sy),
            (5, 3),
            "only the tight tile-center exit should be legal"
        );
        assert!(
            gap < preferred,
            "test setup should force fallback to a sub-preferred gap, got {gap:.2}px"
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
    fn build_staging_goal_prefers_outside_tile_for_worker_inside_footprint() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        let (wx, wy) = map.tile_center(10, 10);
        let worker = entities.spawn_unit(1, EntityKind::Worker, wx, wy).unwrap();
        let occ = Occupancy::build(&map, &entities);

        let goal = build_staging_goal(&map, &occ, &entities, worker, EntityKind::Depot, 9, 9)
            .expect("worker should be able to stage outside the footprint");
        let (tx, ty) = map.tile_of(goal.0, goal.1);
        assert!(
            !(9..=10).contains(&tx) || !(9..=10).contains(&ty),
            "staging goal must be outside the 2x2 depot footprint, got ({tx},{ty})"
        );
    }

    #[test]
    fn build_staging_goal_uses_outside_tile_for_worker_approaching_footprint() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        let (wx, wy) = map.tile_center(10, 10);
        let worker = entities.spawn_unit(1, EntityKind::Worker, wx, wy).unwrap();
        let occ = Occupancy::build(&map, &entities);

        let goal = build_staging_goal(
            &map,
            &occ,
            &entities,
            worker,
            EntityKind::CityCentre,
            20,
            20,
        )
        .expect("worker should be able to stage outside the footprint");
        let tile = map.tile_of(goal.0, goal.1);
        let footprint: std::collections::BTreeSet<(u32, u32)> =
            footprint_tiles(EntityKind::CityCentre, 20, 20)
                .into_iter()
                .collect();
        assert!(
            !footprint.contains(&tile),
            "staging goal must be outside the 3x3 City Centre footprint, got {tile:?}"
        );
    }

    #[test]
    fn build_order_fails_when_worker_cannot_escape_placement_area() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        let (wx, wy) = map.tile_center(10, 10);
        let worker = entities.spawn_unit(1, EntityKind::Worker, wx, wy).unwrap();
        // Box the worker in with depots so it cannot path out of the target area.
        for &(tx, ty) in &[(8, 10), (12, 10), (10, 8), (10, 12)] {
            let (px, py) = map.tile_center(tx, ty);
            entities
                .spawn_building(1, EntityKind::Depot, px, py, true)
                .unwrap();
        }
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        let ok = coordinator.order_build(&mut entities, worker, EntityKind::Depot, 9, 9);
        assert!(!ok, "build order should fail when the worker cannot escape");
        let e = entities.get(worker).unwrap();
        assert!(
            matches!(e.order(), Order::Idle),
            "failed build should clear the worker order"
        );
    }

    #[test]
    fn build_order_accepts_long_expansion_route_to_outside_staging() {
        let map = Map::generate(2, 0);
        let mut entities = EntityStore::new();
        let (wx, wy) = map.tile_center(10, 85);
        let worker = entities.spawn_unit(1, EntityKind::Worker, wx, wy).unwrap();
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        let ok = coordinator.order_build(&mut entities, worker, EntityKind::CityCentre, 48, 70);

        assert!(
            ok,
            "expansion City Centre build order should find a staged route"
        );
        let e = entities.get(worker).unwrap();
        let goal = e.path_goal().expect("build order should set a path goal");
        let goal_tile = map.tile_of(goal.0, goal.1);
        let footprint: std::collections::BTreeSet<(u32, u32)> =
            footprint_tiles(EntityKind::CityCentre, 48, 70)
                .into_iter()
                .collect();
        assert!(
            !footprint.contains(&goal_tile),
            "build path goal should stop outside the expansion City Centre footprint"
        );
        assert!(
            build_staging_goal_in_range(&map, EntityKind::CityCentre, 48, 70, goal),
            "outside staging goal should still be close enough to start construction"
        );
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

    #[test]
    fn request_path_snaps_final_waypoint_to_exact_goal() {
        let map = Map::generate(1, 0x1234_5678);
        let mut entities = EntityStore::new();
        let start = map.tile_center(10, 10);
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
            .expect("unit should spawn");
        let goal_tile_center = map.tile_center(20, 13);
        let exact_goal = (goal_tile_center.0 + 7.25, goal_tile_center.1 - 5.5);
        let occ = Occupancy::build(&map, &entities);

        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        assert!(
            coordinator.request_path(&mut entities, unit, exact_goal, true),
            "fixture path should be found"
        );
        let unit = entities.get(unit).expect("unit should still exist");
        let path = &unit
            .movement
            .as_ref()
            .expect("unit should have movement")
            .path;
        assert_eq!(
            path.first().copied(),
            Some(exact_goal),
            "paths are reverse-ordered, so index 0 must remain the exact requested final goal"
        );
        assert_eq!(unit.path_goal(), Some(exact_goal));
    }

    #[test]
    fn smooth_vehicle_paths_use_clearance_route_shape() {
        let map = Map {
            size: 40,
            terrain: vec![crate::protocol::terrain::GRASS; 40 * 40],
            starts: vec![],
            expansion_sites: vec![],
        };
        for kind in [EntityKind::ScoutCar, EntityKind::Tank, EntityKind::AtTeam] {
            let mut entities = EntityStore::new();
            let start = map.tile_center(10, 10);
            let unit = entities
                .spawn_unit(1, kind, start.0, start.1)
                .expect("unit should spawn");
            let goal_tile = (24, 18);
            let goal = map.tile_center(goal_tile.0, goal_tile.1);
            let occ = Occupancy::build(&map, &entities);

            let mut pathing = PathingService::new(8_192, 256);
            pathing.advance_tick(1);
            {
                let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
                assert!(
                    coordinator.request_path(&mut entities, unit, goal, true),
                    "fixture path should be found for {kind:?}"
                );
            }

            assert!(
                pathing.cache_contains(
                    kind,
                    (10, 10),
                    (goal_tile.0 as i32, goal_tile.1 as i32),
                    config::unit_radius_tiles(kind),
                    RouteShape::ScoutCarClearance
                ),
                "{kind:?} movement path should use the clearance-aware vehicle route shape"
            );
        }
    }

    #[test]
    fn request_chase_path_keeps_tile_guided_interaction_route() {
        let map = Map {
            size: 40,
            terrain: vec![crate::protocol::terrain::GRASS; 40 * 40],
            starts: vec![],
            expansion_sites: vec![],
        };
        let mut entities = EntityStore::new();
        let start = map.tile_center(10, 10);
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
            .expect("unit should spawn");
        let goal = map.tile_center(24, 16);
        let occ = Occupancy::build(&map, &entities);

        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(MIN_REPATH_TICKS);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, MIN_REPATH_TICKS);

        assert!(
            coordinator.request_chase_path(&mut entities, unit, goal),
            "fixture chase path should be found"
        );
        let unit = entities.get(unit).expect("unit should still exist");
        let path = &unit
            .movement
            .as_ref()
            .expect("unit should have movement")
            .path;
        assert!(
            path.len() > 1,
            "chase and other interaction paths should keep intermediate tile waypoints"
        );
        assert_eq!(
            path.first().copied(),
            Some(goal),
            "chase still snaps the final reverse waypoint to the interaction goal"
        );
    }
}
