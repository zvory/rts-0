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
/// Free riflemen that must gather before a wave is committed to attacking. Small so the AI
/// commits attacks within a reasonable time given its slow economy.
const WAVE_SIZE: usize = 4;
/// Drives a single AI-controlled player by emitting ordinary commands each think.
///
/// Stateless beyond the player id: every decision is derived fresh from the current world state,
/// which keeps the AI robust to losing units/buildings without bookkeeping to invalidate.
pub(crate) struct AiController {
    player: u32,
}

impl AiController {
    pub(crate) fn new(player: u32) -> Self {
        AiController { player }
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

        // --- 6. Commit a wave once enough riflemen are free. --------------
        if let Some(units) = ai_shared::ready_attack_wave(free_riflemen, WAVE_SIZE, Some) {
            if let Some((x, y)) = self.nearest_enemy_base(map, entities, players) {
                out.push((self.player, Command::AttackMove { units, x, y }));
            }
        }
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
            |tx, ty| systems::footprint_placeable(map, entities, spatial, building, tx, ty),
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
}
