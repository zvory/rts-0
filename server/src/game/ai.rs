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
use crate::game::entity::{EntityKind, EntityStore, Order};
use crate::game::map::Map;
use crate::game::services::spatial::SpatialIndex;
use crate::game::systems;
use crate::game::PlayerState;
use crate::protocol::{kinds, Command};

// --- Tuning knobs -----------------------------------------------------------

/// Re-plan cadence in ticks. The AI "thinks" this often (≈3×/s at 30 Hz); cheap commands are
/// idempotent enough that acting more often would just churn paths. Decisions are staggered per
/// player so several AIs don't all think on the same tick.
const DECISION_INTERVAL: u32 = 9;
/// Worker count the AI saturates its economy to before it stops queueing more. Kept modest so
/// the (deliberately slow) steel economy isn't entirely consumed by worker supply/cost — the
/// AI needs steel and supply headroom left over to actually field an army.
const TARGET_WORKERS: usize = 8;
/// How many barracks the AI wants (finished + under construction).
const TARGET_BARRACKS: usize = 2;
/// Build a depot when free supply drops below this (and we're not already building one).
const SUPPLY_BUFFER: u32 = 4;
/// Free riflemen that must gather before a wave is committed to attacking. Small so the AI
/// commits attacks within a reasonable time given its slow economy.
const WAVE_SIZE: usize = 4;
/// Max Chebyshev ring (in tiles) searched outward from the base for a build site.
const BUILD_SEARCH_RADIUS: i32 = 16;

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
        // No Industrial Center / nothing left → nothing to do (the match is resolving).
        if !entities.player_alive(self.player) {
            return;
        }

        // Local economy budget. We decrement these as we *decide* to spend so a single think
        // never queues more than the AI can actually afford this tick (commands all apply in
        // order, so without this we'd over-commit on the pre-tick balance).
        let mut steel = me.steel;
        let mut free_supply = me.supply_cap.saturating_sub(me.supply_used);
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
        let mut depot_building = false;

        for e in entities.iter() {
            if e.owner != self.player {
                continue;
            }
            match e.kind {
                EntityKind::Worker => {
                    worker_count += 1;
                    match e.order() {
                        Order::Idle => idle_workers.push(e.id),
                        Order::Gather(_) => gathering_workers.push(e.id),
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
                EntityKind::Depot if e.under_construction() => depot_building = true,
                _ => {}
            }
        }
        let _ = rifleman_count; // surveyed for clarity; waves key off free_riflemen.

        // Workers we may pull onto a build job: prefer truly idle, fall back to a gatherer.
        let mut builder_pool = idle_workers.clone();
        builder_pool.extend(gathering_workers.iter().copied());

        // --- 1. Expand supply with a depot when we're about to choke. ------
        let depot_cost = config::building_stats(EntityKind::Depot)
            .map(|s| s.cost_steel)
            .unwrap_or(50);
        if !depot_building && !supply_capped && free_supply < SUPPLY_BUFFER && steel >= depot_cost {
            if let Some(worker) = builder_pool.pop() {
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
                    steel -= depot_cost;
                    remove_id(&mut idle_workers, worker);
                }
            }
        }

        // --- 2. Build barracks (our rifleman production). --------------------
        let rax_cost = config::building_stats(EntityKind::Barracks)
            .map(|s| s.cost_steel)
            .unwrap_or(100);
        if barracks_total < TARGET_BARRACKS && steel >= rax_cost {
            if let Some(worker) = builder_pool.pop() {
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
                    steel -= rax_cost;
                    remove_id(&mut idle_workers, worker);
                }
            }
        }

        // --- 3. Train workers up to the economy target. -------------------
        let worker_cost = config::unit_stats(EntityKind::Worker)
            .map(|s| s.cost_steel)
            .unwrap_or(50);
        let worker_supply = config::unit_stats(EntityKind::Worker)
            .map(|s| s.supply)
            .unwrap_or(1);
        for industrial_center in idle_industrial_centers {
            if worker_count >= TARGET_WORKERS {
                break;
            }
            if steel < worker_cost || free_supply < worker_supply {
                break;
            }
            out.push((
                self.player,
                Command::Train {
                    building: industrial_center,
                    unit: kinds::WORKER.to_string(),
                },
            ));
            steel -= worker_cost;
            free_supply -= worker_supply;
            worker_count += 1;
        }

        // --- 4. Pump riflemen from each barracks (keep a shallow queue). ---
        let rifleman_cost = config::unit_stats(EntityKind::Rifleman)
            .map(|s| s.cost_steel)
            .unwrap_or(50);
        let rifleman_supply = config::unit_stats(EntityKind::Rifleman)
            .map(|s| s.supply)
            .unwrap_or(1);
        for (rax, queue_len) in barracks {
            // Keep at most one queued behind the in-progress one so we don't lock up steel.
            if queue_len >= 2 {
                continue;
            }
            if steel < rifleman_cost || free_supply < rifleman_supply {
                break;
            }
            out.push((
                self.player,
                Command::Train {
                    building: rax,
                    unit: kinds::RIFLEMAN.to_string(),
                },
            ));
            steel -= rifleman_cost;
            free_supply -= rifleman_supply;
        }

        // --- 5. Send idle workers to mine the nearest steel patch. -------
        for worker in idle_workers {
            if let Some(node) = nearest_steel_node(entities, spatial, worker) {
                out.push((
                    self.player,
                    Command::Gather {
                        units: vec![worker],
                        node,
                    },
                ));
            }
        }

        // --- 6. Commit a wave once enough riflemen are free. --------------
        if free_riflemen.len() >= WAVE_SIZE {
            if let Some((x, y)) = self.nearest_enemy_base(map, entities, players) {
                out.push((
                    self.player,
                    Command::AttackMove {
                        units: free_riflemen,
                        x,
                        y,
                    },
                ));
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
        let (bx, by) = (me.start_tile.0 as i32, me.start_tile.1 as i32);
        for r in 2..=BUILD_SEARCH_RADIUS {
            for dy in -r..=r {
                for dx in -r..=r {
                    // Ring only (Chebyshev distance == r) so we search nearest-first.
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    let (tx, ty) = (bx + dx, by + dy);
                    if tx < 0 || ty < 0 {
                        continue;
                    }
                    let (tx, ty) = (tx as u32, ty as u32);
                    if systems::footprint_placeable(map, entities, spatial, building, tx, ty) {
                        return Some((tx, ty));
                    }
                }
            }
        }
        None
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

/// Nearest non-empty steel node to a worker (by id), or `None` if none remain / worker is gone.
fn nearest_steel_node(entities: &EntityStore, spatial: &SpatialIndex, worker: u32) -> Option<u32> {
    let w = entities.get(worker)?;
    let (wx, wy) = (w.pos_x, w.pos_y);
    let max_radius = config::TILE_SIZE as f32 * 48.0; // generous search radius
    let result = spatial.nearest(
        wx,
        wy,
        max_radius,
        entities,
        |e: &crate::game::entity::Entity| {
            e.kind == EntityKind::Steel && e.remaining().unwrap_or(0) > 0
        },
    );
    result.map(|(id, _)| id)
}

/// Remove the first occurrence of `id` from `v` (used to keep a worker assigned to a build job
/// from also being told to mine in the same think).
fn remove_id(v: &mut Vec<u32>, id: u32) {
    if let Some(pos) = v.iter().position(|&x| x == id) {
        v.swap_remove(pos);
    }
}
