//! A very basic computer opponent. See `DESIGN.md` §8.
//!
//! An [`AiController`] drives one AI-owned player. It is invoked from
//! [`crate::game::Game::tick`] every tick, *before* queued commands are applied, and it pushes
//! ordinary [`Command`]s onto the same pending queue a human client would use. That means the AI
//! has no special powers: its commands run through the identical validation/cost/supply/placement
//! path in `services/commands.rs`, so it can never spend resources it lacks or place buildings illegally —
//! invalid attempts simply fail silently the same way a human's would.
//!
//! Because the controller is server-side (not a network client), it reads the authoritative world
//! state directly rather than a fog-filtered snapshot. Fog is a guard against leaking state to
//! *human* clients over the wire; an internal bot reading full state is not a fog violation. To
//! keep it fair anyway, the AI only ever targets enemy *start tiles*, which are public to everyone
//! via the `start` payload.

use crate::config;
use crate::game::ai_shared;
use crate::game::entity::{EntityKind, EntityStore, Order};
use crate::game::map::Map;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::world_query;
use crate::game::systems;
use crate::game::PlayerState;
use crate::protocol::{kinds, Command};
use std::collections::{BTreeSet, HashSet};

// --- Tuning knobs -----------------------------------------------------------

/// Re-plan cadence in ticks. The AI "thinks" this often (≈3×/s at 30 Hz); cheap commands are
/// idempotent enough that acting more often would just churn paths. Decisions are staggered per
/// player so several AIs don't all think on the same tick.
const DECISION_INTERVAL: u32 = 9;
/// Baseline barracks target (finished + under construction).
const BASE_TARGET_BARRACKS: usize = 2;
/// Once the AI floats at least this much steel, it starts adding more barracks.
const EXTRA_BARRACKS_STEEL_THRESHOLD: u32 = 300;
/// Additional banked steel needed for each barracks beyond the first extra one.
const EXTRA_BARRACKS_STEEL_STEP: u32 = 200;
/// Prevent runaway overbuilding if the AI banks absurdly high.
const MAX_TARGET_BARRACKS: usize = 5;
/// Build a depot when free supply drops below this (and we're not already building one).
const SUPPLY_BUFFER: u32 = 4;
/// Free riflemen stage on a forward rally line before a full wave launches.
const COMBAT_RALLY_TILES_FROM_START: f32 = 8.0;
/// Spacing between neighboring riflemen on the rally line.
const COMBAT_RALLY_SLOT_SPACING_TILES: f32 = 0.75;
/// Once a free rifleman is this much farther forward than the rally line, it should keep
/// pressing toward the enemy base instead of being recycled backward into staging.
const COMBAT_POINT_OF_NO_RETURN_TILES: f32 = 2.0;
/// Initial minimum free riflemen required before the AI launches a rally-line wave.
const BASE_WAVE_SIZE: usize = 3;
/// If the AI cannot assemble its requested wave for this long, fall back to the baseline wave
/// size so it resumes pressuring instead of stalling indefinitely.
const WAVE_STALL_RESET_TICKS: u32 = 360;
/// Drives a single AI-controlled player by emitting ordinary commands each think.
///
/// Most decisions are derived fresh from the current world state. The only persistent planning
/// state is the next desired rifleman wave size and the tick of the last launched wave.
pub(crate) struct AiController {
    player: u32,
    next_wave_size: usize,
    last_wave_launch_tick: u32,
}

impl AiController {
    pub(crate) fn new(player: u32) -> Self {
        AiController {
            player,
            next_wave_size: BASE_WAVE_SIZE,
            last_wave_launch_tick: 0,
        }
    }

    pub(crate) fn player_id(&self) -> u32 {
        self.player
    }

    /// Decide this player's actions for the current tick, pushing any commands onto `out`. A
    /// no-op on most ticks (gated by [`DECISION_INTERVAL`]) and whenever the player is dead.
    pub(crate) fn think(
        &mut self,
        map: &Map,
        entities: &EntityStore,
        spatial: &SpatialIndex,
        players: &[PlayerState],
        tick: u32,
        out: &mut Vec<(u32, Command)>,
    ) {
        // Stagger per player so multiple AIs spread their work across ticks.
        if tick.wrapping_add(self.player) % DECISION_INTERVAL != 0 {
            return;
        }
        let me = match players.iter().find(|p| p.id == self.player) {
            Some(p) => p,
            None => return,
        };
        // No town hall / nothing left → nothing to do (the match is resolving).
        if !entities.player_alive(self.player) {
            return;
        }

        // Local economy budget. We decrement these as we *decide* to spend so a single think
        // never queues more than the AI can actually afford this tick (commands all apply in
        // order, so without this we'd over-commit on the pre-tick balance).
        let mut budget =
            ai_shared::SpendBudget::new(me.steel, me.oil, me.supply_used, me.supply_cap);
        let supply_capped = me.supply_cap >= config::SUPPLY_CAP_MAX;

        // --- Survey the player's holdings in one pass. ---------------------
        let mut idle_workers: Vec<u32> = Vec::new();
        let mut gathering_workers: Vec<u32> = Vec::new();
        let mut worker_count: usize = 0;
        let mut rifleman_count: usize = 0;
        let mut free_riflemen: Vec<u32> = Vec::new();
        // Finished Industrial Centers with an empty production queue (ready to train a worker).
        let mut idle_industrial_centers: Vec<u32> = Vec::new();
        // Finished barracks as (id, queue_len).
        let mut barracks: Vec<(u32, usize)> = Vec::new();
        let mut barracks_total: usize = 0; // finished + under construction
        let mut depot_under_construction = false;
        let mut pending_depot_build = false;

        for e in world_query::owned_units(entities, self.player)
            .chain(world_query::owned_buildings(entities, self.player))
        {
            match e.kind {
                EntityKind::Worker => {
                    worker_count += 1;
                    match e.order() {
                        Order::Idle => idle_workers.push(e.id),
                        Order::Gather(_) => gathering_workers.push(e.id),
                        Order::Build(_) => {
                            if let Some((kind, _, _)) = e.order().build_intent_tile() {
                                match kind {
                                    EntityKind::Depot => pending_depot_build = true,
                                    EntityKind::Barracks => barracks_total += 1,
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                EntityKind::Rifleman => {
                    rifleman_count += 1;
                    if is_free_rifleman(e) {
                        free_riflemen.push(e.id);
                    }
                }
                EntityKind::IndustrialCenter
                    if !e.under_construction() && e.prod_queue().is_empty() =>
                {
                    idle_industrial_centers.push(e.id)
                }
                EntityKind::Barracks => {
                    barracks_total += 1;
                    if !e.under_construction() {
                        barracks.push((e.id, e.prod_queue().len()));
                    }
                }
                EntityKind::Depot if e.under_construction() => depot_under_construction = true,
                _ => {}
            }
        }
        let _ = rifleman_count; // surveyed for clarity; waves key off free_riflemen.
        let depot_in_progress = depot_under_construction || pending_depot_build;
        let target_workers =
            ai_shared::main_base_steel_saturation_target_from_entities(entities, me.start_tile);
        let target_barracks = desired_barracks_target(me.steel);

        // Workers we may pull onto a build job: prefer truly idle, fall back to a gatherer.
        let mut builder_pool = idle_workers.clone();
        builder_pool.extend(gathering_workers.iter().copied());
        let mut reserved_workers = HashSet::new();

        // --- 1. Expand supply with a depot when we're about to choke. ------
        if !depot_in_progress
            && !supply_capped
            && budget.free_supply() < SUPPLY_BUFFER
            && budget.can_afford_building(EntityKind::Depot)
        {
            if let Some(worker) = pop_builder(
                &mut idle_workers,
                &mut gathering_workers,
                &mut builder_pool,
                &mut reserved_workers,
            ) {
                if let Some((tx, ty)) =
                    self.find_build_spot(map, entities, spatial, EntityKind::Depot, me)
                {
                    out.push((
                        self.player,
                        Command::Build {
                            worker,
                            building: kinds::DEPOT.to_string(),
                            tile_x: tx,
                            tile_y: ty,
                        },
                    ));
                    let reserved = budget.reserve_building(EntityKind::Depot);
                    debug_assert!(reserved);
                }
            }
        }

        // --- 2. Build barracks (our rifleman production). --------------------
        if barracks_total < target_barracks && budget.can_afford_building(EntityKind::Barracks) {
            if let Some(worker) = pop_builder(
                &mut idle_workers,
                &mut gathering_workers,
                &mut builder_pool,
                &mut reserved_workers,
            ) {
                if let Some((tx, ty)) =
                    self.find_build_spot(map, entities, spatial, EntityKind::Barracks, me)
                {
                    out.push((
                        self.player,
                        Command::Build {
                            worker,
                            building: kinds::BARRACKS.to_string(),
                            tile_x: tx,
                            tile_y: ty,
                        },
                    ));
                    let reserved = budget.reserve_building(EntityKind::Barracks);
                    debug_assert!(reserved);
                }
            }
        }

        // --- 3. Train workers up to the economy target. -------------------
        for industrial_center in idle_industrial_centers {
            if worker_count >= target_workers {
                break;
            }
            if !budget.can_afford_unit(EntityKind::Worker) {
                break;
            }
            out.push((
                self.player,
                Command::Train {
                    building: industrial_center,
                    unit: kinds::WORKER.to_string(),
                },
            ));
            let reserved = budget.reserve_unit(EntityKind::Worker);
            debug_assert!(reserved);
            worker_count += 1;
        }

        // --- 4. Pump riflemen from each barracks (keep a shallow queue). ---
        for (rax, queue_len) in barracks {
            // Keep at most one queued behind the in-progress one so we don't lock up steel.
            if queue_len >= 2 {
                continue;
            }
            if !budget.can_afford_unit(EntityKind::Rifleman) {
                break;
            }
            out.push((
                self.player,
                Command::Train {
                    building: rax,
                    unit: kinds::RIFLEMAN.to_string(),
                },
            ));
            let reserved = budget.reserve_unit(EntityKind::Rifleman);
            debug_assert!(reserved);
        }

        // --- 5. Send idle workers to distinct steel patches. -------------
        let mut reserved_nodes = occupied_steel_nodes(entities);
        for worker in idle_workers {
            if let Some(node) = nearest_free_steel_node(entities, spatial, worker, &reserved_nodes)
            {
                out.push((
                    self.player,
                    Command::Gather {
                        units: vec![worker],
                        node,
                    },
                ));
                reserved_nodes.insert(node);
            }
        }

        // --- 6. Stage riflemen forward, then launch/continue pressure. ----
        if let Some((enemy_x, enemy_y)) = self.nearest_enemy_base(map, entities, players) {
            let mut staging = Vec::new();
            let mut rally_ready = Vec::new();
            let mut committed = Vec::new();
            let rally_line_cohort = rally_line_cohort(map, entities, self.player, me.start_tile);
            for id in free_riflemen {
                let Some(rifleman) = entities.get(id) else {
                    continue;
                };
                if is_committed_forward(map, me.start_tile, rifleman) {
                    committed.push(id);
                } else if position_is_on_or_adjacent_to_rally_line(
                    map,
                    me.start_tile,
                    (rifleman.pos_x, rifleman.pos_y),
                    rally_line_cohort.len(),
                ) {
                    rally_ready.push(id);
                } else {
                    staging.push(id);
                }
            }
            staging.sort_unstable();
            if !staging.is_empty() {
                let rally_slots = combat_rally_slots(map, me.start_tile, rally_line_cohort.len());
                for id in staging {
                    let Some(slot_index) = rally_line_cohort
                        .iter()
                        .position(|cohort_id| *cohort_id == id)
                    else {
                        continue;
                    };
                    let (x, y) = rally_slots[slot_index];
                    out.push((
                        self.player,
                        Command::Move {
                            units: vec![id],
                            x,
                            y,
                        },
                    ));
                }
            }
            if !committed.is_empty() {
                out.push((
                    self.player,
                    Command::AttackMove {
                        units: committed,
                        x: enemy_x,
                        y: enemy_y,
                    },
                ));
            }

            let wave_size = self.desired_wave_size(tick);
            if rally_ready.len() >= wave_size {
                out.push((
                    self.player,
                    Command::AttackMove {
                        units: rally_ready,
                        x: enemy_x,
                        y: enemy_y,
                    },
                ));
                self.note_wave_launch(tick);
            }
        }
    }

    fn desired_wave_size(&mut self, tick: u32) -> usize {
        if tick.saturating_sub(self.last_wave_launch_tick) >= WAVE_STALL_RESET_TICKS {
            self.next_wave_size = BASE_WAVE_SIZE;
        }
        self.next_wave_size
    }

    fn note_wave_launch(&mut self, tick: u32) {
        self.last_wave_launch_tick = tick;
        self.next_wave_size = self.next_wave_size.saturating_add(1);
    }

    /// Find a placeable footprint for `building` by scanning rings outward from the AI's start
    /// tile. Returns the top-left tile of the first placeable footprint, or `None` if the area is
    /// too congested (caller then simply skips the build this think and retries later).
    fn find_build_spot(
        &self,
        map: &Map,
        entities: &EntityStore,
        spatial: &SpatialIndex,
        building: EntityKind,
        me: &PlayerState,
    ) -> Option<(u32, u32)> {
        // Pre-compute 1-tile margin around all existing buildings so placements keep a gap.
        let mut building_margin: BTreeSet<(u32, u32)> = BTreeSet::new();
        for e in entities.iter() {
            if e.is_building() {
                for (tx, ty) in crate::game::services::occupancy::building_footprint(map, e) {
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let nx = tx as i32 + dx;
                            let ny = ty as i32 + dy;
                            if nx >= 0 && ny >= 0 {
                                building_margin.insert((nx as u32, ny as u32));
                            }
                        }
                    }
                }
            }
        }
        ai_shared::find_build_spot_near_start_with(
            map.size,
            map.size,
            me.start_tile,
            building,
            ai_shared::BuildSearch {
                min_radius: 2,
                max_radius: ai_shared::DEFAULT_BUILD_SEARCH_MAX_RADIUS,
                prefer_away_from_center: false,
            },
            &BTreeSet::new(),
            |tx, ty| {
                if !systems::footprint_placeable(map, entities, spatial, building, tx, ty) {
                    return false;
                }
                let Some(stats) = config::building_stats(building) else {
                    return false;
                };
                for dy in 0..stats.foot_h {
                    for dx in 0..stats.foot_w {
                        if building_margin.contains(&(tx + dx, ty + dy)) {
                            return false;
                        }
                    }
                }
                true
            },
        )
    }

    /// World-pixel center of the nearest *living* enemy's start tile, or `None` if the AI is the
    /// last one standing. Start tiles are public, so targeting them leaks nothing.
    fn nearest_enemy_base(
        &self,
        map: &Map,
        entities: &EntityStore,
        players: &[PlayerState],
    ) -> Option<(f32, f32)> {
        let me = players.iter().find(|p| p.id == self.player)?;
        let (mx, my) = map.tile_center(me.start_tile.0, me.start_tile.1);
        let mut best: Option<(f32, f32, f32)> = None;
        for p in players {
            if p.id == self.player || !entities.player_alive(p.id) {
                continue;
            }
            let (ex, ey) = map.tile_center(p.start_tile.0, p.start_tile.1);
            let d = (ex - mx) * (ex - mx) + (ey - my) * (ey - my);
            if best.map(|(_, _, bd)| d < bd).unwrap_or(true) {
                best = Some((ex, ey, d));
            }
        }
        best.map(|(x, y, _)| (x, y))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FreeRiflemanDisposition {
    Stage,
    RallyReady,
    CommittedForward,
}

fn combat_rally_world(map: &Map, start_tile: (u32, u32)) -> (f32, f32) {
    let start = map.tile_center(start_tile.0, start_tile.1);
    let center = map_center_world(map);
    step_toward_world(
        start,
        center,
        COMBAT_RALLY_TILES_FROM_START * config::TILE_SIZE as f32,
    )
}

fn combat_rally_slots(map: &Map, start_tile: (u32, u32), count: usize) -> Vec<(f32, f32)> {
    if count == 0 {
        return Vec::new();
    }
    let rally = combat_rally_world(map, start_tile);
    let start = map.tile_center(start_tile.0, start_tile.1);
    let center = map_center_world(map);
    let dx = center.0 - start.0;
    let dy = center.1 - start.1;
    let dist = (dx * dx + dy * dy).sqrt();
    let (lx, ly) = if dist <= f32::EPSILON {
        (1.0, 0.0)
    } else {
        (-dy / dist, dx / dist)
    };
    let spacing = COMBAT_RALLY_SLOT_SPACING_TILES * config::TILE_SIZE as f32;
    let center_index = (count as f32 - 1.0) * 0.5;
    (0..count)
        .map(|i| {
            let offset = (i as f32 - center_index) * spacing;
            (rally.0 + lx * offset, rally.1 + ly * offset)
        })
        .collect()
}

fn map_center_world(map: &Map) -> (f32, f32) {
    let size_px = map.size as f32 * config::TILE_SIZE as f32;
    (size_px * 0.5, size_px * 0.5)
}

fn step_toward_world(from: (f32, f32), to: (f32, f32), step: f32) -> (f32, f32) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= f32::EPSILON {
        return to;
    }
    let clamped = step.min(dist);
    (from.0 + dx / dist * clamped, from.1 + dy / dist * clamped)
}

fn classify_free_rifleman(
    map: &Map,
    start_tile: (u32, u32),
    rifleman: &crate::game::entity::Entity,
) -> FreeRiflemanDisposition {
    if is_committed_forward(map, start_tile, rifleman) {
        return FreeRiflemanDisposition::CommittedForward;
    }
    if position_is_on_or_adjacent_to_rally_line(
        map,
        start_tile,
        (rifleman.pos_x, rifleman.pos_y),
        1,
    ) {
        FreeRiflemanDisposition::RallyReady
    } else {
        FreeRiflemanDisposition::Stage
    }
}

fn is_committed_forward(
    map: &Map,
    start_tile: (u32, u32),
    rifleman: &crate::game::entity::Entity,
) -> bool {
    let start = map.tile_center(start_tile.0, start_tile.1);
    let rally = combat_rally_world(map, start_tile);
    let dx = rally.0 - start.0;
    let dy = rally.1 - start.1;
    let rally_dist = (dx * dx + dy * dy).sqrt();
    if rally_dist <= f32::EPSILON {
        return false;
    }
    let ux = dx / rally_dist;
    let uy = dy / rally_dist;
    let progress = (rifleman.pos_x - start.0) * ux + (rifleman.pos_y - start.1) * uy;
    let point_of_no_return =
        rally_dist + COMBAT_POINT_OF_NO_RETURN_TILES * config::TILE_SIZE as f32;
    progress >= point_of_no_return
}

fn position_is_on_or_adjacent_to_rally_line(
    map: &Map,
    start_tile: (u32, u32),
    pos: (f32, f32),
    cohort_len: usize,
) -> bool {
    let rally = combat_rally_world(map, start_tile);
    let start = map.tile_center(start_tile.0, start_tile.1);
    let center = map_center_world(map);
    let dx = center.0 - start.0;
    let dy = center.1 - start.1;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= f32::EPSILON {
        return true;
    }
    let forward_x = dx / dist;
    let forward_y = dy / dist;
    let lateral_x = -forward_y;
    let lateral_y = forward_x;
    let from_rally_x = pos.0 - rally.0;
    let from_rally_y = pos.1 - rally.1;
    let forward_error = (from_rally_x * forward_x + from_rally_y * forward_y).abs();
    let lateral_offset = (from_rally_x * lateral_x + from_rally_y * lateral_y).abs();
    let half_span = if cohort_len <= 1 {
        0.0
    } else {
        (cohort_len as f32 - 1.0) * 0.5 * COMBAT_RALLY_SLOT_SPACING_TILES * config::TILE_SIZE as f32
    };
    forward_error <= config::TILE_SIZE as f32
        && lateral_offset <= half_span + config::TILE_SIZE as f32
}

fn rally_line_cohort(
    map: &Map,
    entities: &EntityStore,
    player: u32,
    start_tile: (u32, u32),
) -> Vec<u32> {
    let mut cohort: Vec<u32> = entities
        .iter()
        .filter(|e| e.owner == player && e.kind == EntityKind::Rifleman)
        .filter(|e| is_rally_line_member(map, start_tile, e))
        .map(|e| e.id)
        .collect();
    cohort.sort_unstable();
    cohort
}

fn is_rally_line_member(
    map: &Map,
    start_tile: (u32, u32),
    rifleman: &crate::game::entity::Entity,
) -> bool {
    if classify_free_rifleman(map, start_tile, rifleman)
        != FreeRiflemanDisposition::CommittedForward
    {
        return true;
    }
    match rifleman.order() {
        Order::Move(_) => rifleman
            .path_goal()
            .map(|goal| goal_is_on_rally_line(map, start_tile, goal))
            .unwrap_or(false),
        _ => false,
    }
}

fn goal_is_on_rally_line(map: &Map, start_tile: (u32, u32), goal: (f32, f32)) -> bool {
    let rally = combat_rally_world(map, start_tile);
    let start = map.tile_center(start_tile.0, start_tile.1);
    let center = map_center_world(map);
    let dx = center.0 - start.0;
    let dy = center.1 - start.1;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= f32::EPSILON {
        return false;
    }
    let forward_x = dx / dist;
    let forward_y = dy / dist;
    let lateral_x = -forward_y;
    let lateral_y = forward_x;
    let from_rally_x = goal.0 - rally.0;
    let from_rally_y = goal.1 - rally.1;
    let forward_error = (from_rally_x * forward_x + from_rally_y * forward_y).abs();
    let lateral_offset = (from_rally_x * lateral_x + from_rally_y * lateral_y).abs();
    forward_error <= config::TILE_SIZE as f32
        && lateral_offset <= COMBAT_RALLY_SLOT_SPACING_TILES * config::TILE_SIZE as f32 * 8.0
}

/// A rifleman available to join a wave: idle, or one whose attack-move finished (no path, no
/// target) so it's standing around and should regroup with the next push.
fn is_free_rifleman(e: &crate::game::entity::Entity) -> bool {
    match e.order() {
        Order::Idle => true,
        Order::AttackMove(_) => e.path_is_empty() && e.target_id().is_none(),
        _ => false,
    }
}

/// The AI starts at two barracks, then scales production when it floats steel.
fn desired_barracks_target(steel: u32) -> usize {
    let extra = steel
        .checked_sub(EXTRA_BARRACKS_STEEL_THRESHOLD + 1)
        .map(|over| 1 + (over / EXTRA_BARRACKS_STEEL_STEP) as usize)
        .unwrap_or(0);
    (BASE_TARGET_BARRACKS + extra).min(MAX_TARGET_BARRACKS)
}

/// Steel patches already held by actively-harvesting workers.
fn occupied_steel_nodes(entities: &EntityStore) -> HashSet<u32> {
    entities
        .iter()
        .filter(|e| e.kind == EntityKind::Worker)
        .filter_map(|e| e.order().gather_node())
        .filter(|&node| world_query::node_holder(entities, node).is_some())
        .collect()
}

/// Nearest non-empty steel node to a worker (by id) that has not already been reserved this
/// think, or `None` if none remain / worker is gone.
fn nearest_free_steel_node(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    worker: u32,
    reserved_nodes: &HashSet<u32>,
) -> Option<u32> {
    let w = entities.get(worker)?;
    spatial
        .nearest(
            w.pos_x,
            w.pos_y,
            world_query::default_resource_search_radius_px(),
            entities,
            |e| {
                e.kind == EntityKind::Steel
                    && e.remaining().unwrap_or(0) > 0
                    && !reserved_nodes.contains(&e.id)
            },
        )
        .map(|(id, _)| id)
}

/// Remove the first occurrence of `id` from `v` (used to keep a worker assigned to a build job
/// from also being told to mine in the same think).
fn remove_id(v: &mut Vec<u32>, id: u32) {
    if let Some(pos) = v.iter().position(|&x| x == id) {
        v.swap_remove(pos);
    }
}

/// Reserve one worker for a build decision, preferring idle workers over active gatherers and
/// keeping every local worker list in sync so later decisions in the same think cannot reuse it.
fn pop_builder(
    idle_workers: &mut Vec<u32>,
    gathering_workers: &mut Vec<u32>,
    builder_pool: &mut Vec<u32>,
    reserved_workers: &mut HashSet<u32>,
) -> Option<u32> {
    let worker = idle_workers.pop().or_else(|| gathering_workers.pop())?;
    if !reserved_workers.insert(worker) {
        return None;
    }
    remove_id(builder_pool, worker);
    remove_id(idle_workers, worker);
    remove_id(gathering_workers, worker);
    Some(worker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::Order;
    use crate::protocol::Command;

    #[test]
    fn pop_builder_prefers_idle_and_does_not_reuse_workers() {
        let mut idle_workers = vec![11, 12];
        let mut gathering_workers = vec![21];
        let mut builder_pool = vec![11, 12, 21];
        let mut reserved = HashSet::new();

        let first = pop_builder(
            &mut idle_workers,
            &mut gathering_workers,
            &mut builder_pool,
            &mut reserved,
        );
        let second = pop_builder(
            &mut idle_workers,
            &mut gathering_workers,
            &mut builder_pool,
            &mut reserved,
        );
        let third = pop_builder(
            &mut idle_workers,
            &mut gathering_workers,
            &mut builder_pool,
            &mut reserved,
        );
        let fourth = pop_builder(
            &mut idle_workers,
            &mut gathering_workers,
            &mut builder_pool,
            &mut reserved,
        );

        assert_eq!(first, Some(12));
        assert_eq!(second, Some(11));
        assert_eq!(third, Some(21));
        assert_eq!(fourth, None);
        assert!(builder_pool.is_empty());
        assert_eq!(reserved.len(), 3);
    }

    #[test]
    fn idle_workers_pick_distinct_steel_nodes() {
        let mut entities = EntityStore::default();
        let worker_a = entities
            .spawn_unit(1, EntityKind::Worker, 0.0, 0.0)
            .unwrap();
        let worker_b = entities
            .spawn_unit(1, EntityKind::Worker, 8.0, 0.0)
            .unwrap();
        let node_a = entities.spawn_node(EntityKind::Steel, 64.0, 0.0).unwrap();
        let node_b = entities.spawn_node(EntityKind::Steel, 96.0, 0.0).unwrap();
        let spatial = SpatialIndex::build(&entities, config::TILE_SIZE);
        let mut reserved = HashSet::new();

        let pick_a = nearest_free_steel_node(&entities, &spatial, worker_a, &reserved);
        assert_eq!(pick_a, Some(node_a));
        reserved.insert(node_a);

        let pick_b = nearest_free_steel_node(&entities, &spatial, worker_b, &reserved);
        assert_eq!(pick_b, Some(node_b));
    }

    #[test]
    fn pending_depot_build_blocks_repeat_supply_depot_plan() {
        let mut entities = EntityStore::default();
        let worker = entities
            .spawn_unit(2, EntityKind::Worker, 0.0, 0.0)
            .unwrap();
        if let Some(e) = entities.get_mut(worker) {
            e.set_order(Order::build(EntityKind::Depot, 5, 6));
        }
        let spatial = SpatialIndex::build(&entities, config::TILE_SIZE);
        let mut ai = AiController::new(2);
        let players = vec![PlayerState {
            id: 2,
            name: "Computer".into(),
            color: "#000".into(),
            start_tile: (10, 10),
            steel: 999,
            oil: 0,
            supply_used: 8,
            supply_cap: 10,
        }];
        let map = Map::generate(2, 1234);
        let mut out = Vec::new();

        ai.think(&map, &entities, &spatial, &players, 7, &mut out);

        assert!(
            !out.iter().any(|(_, cmd)| matches!(cmd, Command::Build { building, .. } if building == crate::protocol::kinds::DEPOT)),
            "AI should treat a worker's pending depot build intent as supply already in progress"
        );
    }

    #[test]
    fn main_base_miner_target_counts_only_nearby_nonempty_steel() {
        let mut entities = EntityStore::default();
        let (hx, hy) = (
            10.5 * config::TILE_SIZE as f32,
            20.5 * config::TILE_SIZE as f32,
        );
        let in_range = (config::IC_RESOURCE_MAX_DIST_TILES - 0.25) * config::TILE_SIZE as f32;
        let out_of_range = (config::IC_RESOURCE_MAX_DIST_TILES + 2.0) * config::TILE_SIZE as f32;

        entities
            .spawn_node(EntityKind::Steel, hx + in_range, hy)
            .unwrap();
        entities
            .spawn_node(EntityKind::Steel, hx - in_range, hy)
            .unwrap();
        entities
            .spawn_node(EntityKind::Oil, hx, hy + in_range)
            .unwrap();
        entities
            .spawn_node(EntityKind::Steel, hx, hy + out_of_range)
            .unwrap();

        let depleted = entities
            .spawn_node(EntityKind::Steel, hx, hy - in_range)
            .unwrap();
        if let Some(node) = entities.get_mut(depleted) {
            if let Some(resource) = node.resource_node.as_mut() {
                resource.remaining = 0;
            }
        }

        let me = PlayerState {
            id: 2,
            name: "Computer".into(),
            color: "#000".into(),
            start_tile: (10, 20),
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
        };

        assert_eq!(
            ai_shared::main_base_steel_saturation_target_from_entities(&entities, me.start_tile),
            2
        );
    }

    #[test]
    fn barracks_target_scales_with_banked_steel() {
        assert_eq!(desired_barracks_target(0), 2);
        assert_eq!(desired_barracks_target(300), 2);
        assert_eq!(desired_barracks_target(301), 3);
        assert_eq!(desired_barracks_target(500), 3);
        assert_eq!(desired_barracks_target(501), 4);
        assert_eq!(desired_barracks_target(2_000), 5);
    }

    #[test]
    fn wave_size_escalates_after_launches() {
        let mut ai = AiController::new(2);

        assert_eq!(ai.player_id(), 2);
        assert_eq!(ai.desired_wave_size(0), 3);
        ai.note_wave_launch(90);
        assert_eq!(ai.desired_wave_size(99), 4);
        ai.note_wave_launch(180);
        assert_eq!(ai.desired_wave_size(189), 5);
    }

    #[test]
    fn wave_size_resets_after_stall() {
        let mut ai = AiController::new(2);

        ai.note_wave_launch(90);
        ai.note_wave_launch(180);
        ai.note_wave_launch(270);
        assert_eq!(ai.desired_wave_size(270 + WAVE_STALL_RESET_TICKS - 1), 6);
        assert_eq!(ai.desired_wave_size(270 + WAVE_STALL_RESET_TICKS), 3);
    }

    #[test]
    fn wave_size_has_no_cap() {
        let mut ai = AiController::new(2);

        for tick in [90, 180, 270, 360, 450, 540, 630, 720] {
            ai.note_wave_launch(tick);
        }

        assert_eq!(ai.desired_wave_size(729), 11);
    }

    #[test]
    fn classify_free_rifleman_splits_stage_ready_and_committed() {
        let map = Map::generate(2, 1234);
        let start_tile = (8, 8);
        let start = map.tile_center(start_tile.0, start_tile.1);
        let center = map_center_world(&map);
        let rally = combat_rally_world(&map, start_tile);
        let stage_pos = step_toward_world(start, center, 6.0 * config::TILE_SIZE as f32);
        let committed_pos = step_toward_world(
            start,
            center,
            (COMBAT_RALLY_TILES_FROM_START + COMBAT_POINT_OF_NO_RETURN_TILES + 1.0)
                * config::TILE_SIZE as f32,
        );

        let stage = crate::game::entity::Entity::new_unit(
            2,
            EntityKind::Rifleman,
            stage_pos.0,
            stage_pos.1,
        )
        .unwrap();
        let ready =
            crate::game::entity::Entity::new_unit(2, EntityKind::Rifleman, rally.0, rally.1)
                .unwrap();
        let committed = crate::game::entity::Entity::new_unit(
            2,
            EntityKind::Rifleman,
            committed_pos.0,
            committed_pos.1,
        )
        .unwrap();

        assert_eq!(
            classify_free_rifleman(&map, start_tile, &stage),
            FreeRiflemanDisposition::Stage
        );
        assert_eq!(
            classify_free_rifleman(&map, start_tile, &ready),
            FreeRiflemanDisposition::RallyReady
        );
        assert_eq!(
            classify_free_rifleman(&map, start_tile, &committed),
            FreeRiflemanDisposition::CommittedForward
        );
    }

    #[test]
    fn committed_free_rifleman_keeps_pressing_enemy_base() {
        let map = Map::generate(2, 1234);
        let mut entities = EntityStore::default();
        let ai_start = (8, 8);
        let enemy_start = (56, 56);
        let ai_base = map.tile_center(ai_start.0, ai_start.1);
        let enemy_base = map.tile_center(enemy_start.0, enemy_start.1);
        entities
            .spawn_building(2, EntityKind::IndustrialCenter, ai_base.0, ai_base.1, true)
            .unwrap();
        entities
            .spawn_building(
                1,
                EntityKind::IndustrialCenter,
                enemy_base.0,
                enemy_base.1,
                true,
            )
            .unwrap();
        let committed_pos = step_toward_world(
            ai_base,
            map_center_world(&map),
            (COMBAT_RALLY_TILES_FROM_START + COMBAT_POINT_OF_NO_RETURN_TILES + 1.0)
                * config::TILE_SIZE as f32,
        );
        entities
            .spawn_unit(2, EntityKind::Rifleman, committed_pos.0, committed_pos.1)
            .unwrap();
        let spatial = SpatialIndex::build(&entities, config::TILE_SIZE);
        let mut ai = AiController::new(2);
        let players = vec![
            PlayerState {
                id: 1,
                name: "Enemy".into(),
                color: "#fff".into(),
                start_tile: enemy_start,
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 0,
            },
            PlayerState {
                id: 2,
                name: "Computer".into(),
                color: "#000".into(),
                start_tile: ai_start,
                steel: 0,
                oil: 0,
                supply_used: 1,
                supply_cap: 10,
            },
        ];
        let mut out = Vec::new();

        ai.think(&map, &entities, &spatial, &players, 7, &mut out);

        assert!(out.iter().any(|(player, cmd)| {
            *player == 2
                && matches!(
                    cmd,
                    Command::AttackMove { units, x, y }
                        if units.len() == 1
                            && *x == enemy_base.0
                            && *y == enemy_base.1
                )
        }));
    }

    #[test]
    fn rally_wave_launch_sends_all_ready_riflemen() {
        let map = Map::generate(2, 1234);
        let mut entities = EntityStore::default();
        let ai_start = (8, 8);
        let enemy_start = (56, 56);
        let ai_base = map.tile_center(ai_start.0, ai_start.1);
        let enemy_base = map.tile_center(enemy_start.0, enemy_start.1);
        entities
            .spawn_building(2, EntityKind::IndustrialCenter, ai_base.0, ai_base.1, true)
            .unwrap();
        entities
            .spawn_building(
                1,
                EntityKind::IndustrialCenter,
                enemy_base.0,
                enemy_base.1,
                true,
            )
            .unwrap();
        let rally = combat_rally_world(&map, ai_start);
        for offset in [0.0_f32, 6.0, 12.0, 18.0] {
            entities
                .spawn_unit(2, EntityKind::Rifleman, rally.0 + offset, rally.1 + offset)
                .unwrap();
        }
        let spatial = SpatialIndex::build(&entities, config::TILE_SIZE);
        let mut ai = AiController::new(2);
        let players = vec![
            PlayerState {
                id: 1,
                name: "Enemy".into(),
                color: "#fff".into(),
                start_tile: enemy_start,
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 0,
            },
            PlayerState {
                id: 2,
                name: "Computer".into(),
                color: "#000".into(),
                start_tile: ai_start,
                steel: 0,
                oil: 0,
                supply_used: 4,
                supply_cap: 10,
            },
        ];
        let mut out = Vec::new();

        ai.think(&map, &entities, &spatial, &players, 7, &mut out);

        assert!(out.iter().any(|(player, cmd)| {
            *player == 2
                && matches!(
                    cmd,
                    Command::AttackMove { units, x, y }
                        if units.len() == 4
                            && *x == enemy_base.0
                            && *y == enemy_base.1
                )
        }));
    }

    #[test]
    fn rally_wave_launch_includes_riflemen_adjacent_to_the_line() {
        let map = Map::generate(2, 1234);
        let mut entities = EntityStore::default();
        let ai_start = (8, 8);
        let enemy_start = (56, 56);
        let ai_base = map.tile_center(ai_start.0, ai_start.1);
        let enemy_base = map.tile_center(enemy_start.0, enemy_start.1);
        entities
            .spawn_building(2, EntityKind::IndustrialCenter, ai_base.0, ai_base.1, true)
            .unwrap();
        entities
            .spawn_building(
                1,
                EntityKind::IndustrialCenter,
                enemy_base.0,
                enemy_base.1,
                true,
            )
            .unwrap();
        let rally = combat_rally_world(&map, ai_start);
        let start = map.tile_center(ai_start.0, ai_start.1);
        let center = map_center_world(&map);
        let dx = center.0 - start.0;
        let dy = center.1 - start.1;
        let dist = (dx * dx + dy * dy).sqrt();
        let (lateral_x, lateral_y) = if dist <= f32::EPSILON {
            (1.0, 0.0)
        } else {
            (-dy / dist, dx / dist)
        };

        entities
            .spawn_unit(2, EntityKind::Rifleman, rally.0, rally.1)
            .unwrap();
        entities
            .spawn_unit(2, EntityKind::Rifleman, rally.0 + 8.0, rally.1)
            .unwrap();
        entities
            .spawn_unit(
                2,
                EntityKind::Rifleman,
                rally.0 + lateral_x * config::TILE_SIZE as f32,
                rally.1 + lateral_y * config::TILE_SIZE as f32,
            )
            .unwrap();

        let spatial = SpatialIndex::build(&entities, config::TILE_SIZE);
        let mut ai = AiController::new(2);
        let players = vec![
            PlayerState {
                id: 1,
                name: "Enemy".into(),
                color: "#fff".into(),
                start_tile: enemy_start,
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 0,
            },
            PlayerState {
                id: 2,
                name: "Computer".into(),
                color: "#000".into(),
                start_tile: ai_start,
                steel: 0,
                oil: 0,
                supply_used: 3,
                supply_cap: 10,
            },
        ];
        let mut out = Vec::new();

        ai.think(&map, &entities, &spatial, &players, 7, &mut out);

        assert!(out.iter().any(|(player, cmd)| {
            *player == 2
                && matches!(
                    cmd,
                    Command::AttackMove { units, x, y }
                        if units.len() == 3
                            && *x == enemy_base.0
                            && *y == enemy_base.1
                )
        }));
    }

    #[test]
    fn staging_riflemen_spread_across_rally_line_slots() {
        let map = Map::generate(2, 1234);
        let mut entities = EntityStore::default();
        let ai_start = (8, 8);
        let enemy_start = (56, 56);
        let ai_base = map.tile_center(ai_start.0, ai_start.1);
        let enemy_base = map.tile_center(enemy_start.0, enemy_start.1);
        entities
            .spawn_building(2, EntityKind::IndustrialCenter, ai_base.0, ai_base.1, true)
            .unwrap();
        entities
            .spawn_building(
                1,
                EntityKind::IndustrialCenter,
                enemy_base.0,
                enemy_base.1,
                true,
            )
            .unwrap();
        for x in [ai_base.0, ai_base.0 + 8.0, ai_base.0 + 16.0] {
            entities
                .spawn_unit(2, EntityKind::Rifleman, x, ai_base.1)
                .unwrap();
        }
        let spatial = SpatialIndex::build(&entities, config::TILE_SIZE);
        let mut ai = AiController::new(2);
        let players = vec![
            PlayerState {
                id: 1,
                name: "Enemy".into(),
                color: "#fff".into(),
                start_tile: enemy_start,
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 0,
            },
            PlayerState {
                id: 2,
                name: "Computer".into(),
                color: "#000".into(),
                start_tile: ai_start,
                steel: 0,
                oil: 0,
                supply_used: 3,
                supply_cap: 10,
            },
        ];
        let mut out = Vec::new();

        ai.think(&map, &entities, &spatial, &players, 7, &mut out);

        let move_targets: Vec<(f32, f32)> = out
            .iter()
            .filter_map(|(player, cmd)| match cmd {
                Command::Move { units, x, y } if *player == 2 && units.len() == 1 => Some((*x, *y)),
                _ => None,
            })
            .collect();
        assert_eq!(move_targets.len(), 3);
        let unique_targets: std::collections::BTreeSet<(i32, i32)> = move_targets
            .iter()
            .map(|(x, y)| (x.round() as i32, y.round() as i32))
            .collect();
        assert_eq!(unique_targets.len(), 3);
    }

    #[test]
    fn staging_slots_include_riflemen_already_moving_to_the_line() {
        let map = Map::generate(2, 1234);
        let mut entities = EntityStore::default();
        let ai_start = (8, 8);
        let enemy_start = (56, 56);
        let ai_base = map.tile_center(ai_start.0, ai_start.1);
        let enemy_base = map.tile_center(enemy_start.0, enemy_start.1);
        entities
            .spawn_building(2, EntityKind::IndustrialCenter, ai_base.0, ai_base.1, true)
            .unwrap();
        entities
            .spawn_building(
                1,
                EntityKind::IndustrialCenter,
                enemy_base.0,
                enemy_base.1,
                true,
            )
            .unwrap();
        let first = entities
            .spawn_unit(2, EntityKind::Rifleman, ai_base.0, ai_base.1)
            .unwrap();
        let second = entities
            .spawn_unit(2, EntityKind::Rifleman, ai_base.0 + 8.0, ai_base.1)
            .unwrap();
        let slots = combat_rally_slots(&map, ai_start, 2);
        if let Some(rifleman) = entities.get_mut(first) {
            rifleman.set_order(Order::move_to(slots[0].0, slots[0].1));
            rifleman.set_path_goal(Some(slots[0]));
        }
        let spatial = SpatialIndex::build(&entities, config::TILE_SIZE);
        let mut ai = AiController::new(2);
        let players = vec![
            PlayerState {
                id: 1,
                name: "Enemy".into(),
                color: "#fff".into(),
                start_tile: enemy_start,
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 0,
            },
            PlayerState {
                id: 2,
                name: "Computer".into(),
                color: "#000".into(),
                start_tile: ai_start,
                steel: 0,
                oil: 0,
                supply_used: 2,
                supply_cap: 10,
            },
        ];
        let mut out = Vec::new();

        ai.think(&map, &entities, &spatial, &players, 7, &mut out);

        let move_targets: Vec<(u32, f32, f32)> = out
            .iter()
            .filter_map(|(player, cmd)| match cmd {
                Command::Move { units, x, y } if *player == 2 && units.len() == 1 => {
                    Some((units[0], *x, *y))
                }
                _ => None,
            })
            .collect();
        assert_eq!(move_targets.len(), 1);
        assert_eq!(move_targets[0].0, second);
        assert_eq!(move_targets[0].1, slots[1].0);
        assert_eq!(move_targets[0].2, slots[1].1);
    }
}
