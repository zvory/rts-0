//! The authoritative game simulation. See `docs/design/server-sim.md` for the public API contract.
//!
//! [`Game`] is the single seam between the simulation and the networking/lobby layer. The
//! networking layer calls ONLY the methods in §3.1; everything else here is private detail.
//!
//! The simulation is fixed-rate: each [`Game::tick`] drains queued commands, advances every
//! system in a deterministic order, and recomputes per-player fog. Snapshots are pulled
//! separately via [`Game::snapshot_for`], fog-filtered so a player only ever sees neutral /
//! enemy entities on tiles they currently see.

pub(crate) mod ability;
mod ai;
pub(crate) mod ai_core;
pub(crate) mod ai_shared;
pub(crate) mod command;
mod commands;
pub(crate) mod entity;
pub(crate) mod fog;
mod invariants;
pub(crate) mod map;
mod pathfinding;
mod replay;
mod scoring;
pub mod selfplay;
pub(crate) mod services;
mod setup;
pub(crate) mod smoke;
mod snapshot;
mod systems;

use std::collections::HashMap;

use crate::config;
use crate::protocol::{
    Event, MapInfo, PlayerResourceSnapshot, PlayerScore, PlayerStart, ResourceDelta, ResourceNode,
    Snapshot, StartPayload,
};
use crate::rules::{economy as economy_rules, projection};
use serde::{Deserialize, Serialize};

use ai::{AiController, AiThinkContext};
use entity::{EntityKind, EntityStore};
use fog::{Fog, LingeringSightSource};
use map::Map;
use rand::rngs::SmallRng;
use rand::SeedableRng;
use replay::CommandLogEntry;
use smoke::SmokeCloudStore;

pub use crate::game::command::SimCommand;

#[cfg(test)]
pub(crate) const FULL_AI_TESTS_ENV: &str = "RTS_FULL_AI_TESTS";
#[cfg(test)]
pub(crate) const SELFPLAY_FULL_ENV: &str = "RTS_SELFPLAY_FULL";

#[cfg(test)]
fn env_flag_enabled(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
pub(crate) fn full_ai_tests_enabled() -> bool {
    env_flag_enabled(FULL_AI_TESTS_ENV) || env_flag_enabled(SELFPLAY_FULL_ENV)
}

#[cfg(test)]
pub(crate) fn skip_unless_full_ai(test_name: &str) -> bool {
    if full_ai_tests_enabled() {
        false
    } else {
        eprintln!("skipping {test_name}; set {FULL_AI_TESTS_ENV}=1 to run full AI coverage");
        true
    }
}

/// Lobby-supplied identity for a player joining a match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerInit {
    pub id: u32,
    pub name: String,
    pub color: String,
    /// When true this player is a computer opponent: it has no socket and is driven by an
    /// internal [`AiController`] instead of receiving snapshots / sending commands.
    pub is_ai: bool,
}

/// Per-player economy and bookkeeping carried for the whole match. Visible to `systems` (the
/// only other module that mutates economy), but not part of the public API.
pub(crate) struct PlayerState {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) color: String,
    pub(crate) start_tile: (u32, u32),
    pub(crate) steel: u32,
    pub(crate) oil: u32,
    /// Supply currently consumed by living + in-production units.
    pub(crate) supply_used: u32,
    /// Supply provided by completed City Centres and Depots, capped at `SUPPLY_CAP_MAX`.
    pub(crate) supply_cap: u32,
    pub(crate) is_ai: bool,
    pub(crate) score: ScoreState,
}

/// Per-player score-screen counters. Values are accumulated from authoritative entity lifecycle
/// events, not inferred from fog-filtered snapshots.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScoreState {
    unit_score: u32,
    structure_score: u32,
    units_killed: u32,
    units_lost: u32,
    buildings_killed: u32,
    buildings_lost: u32,
}

#[derive(Clone, Copy)]
enum AiProfileSelection {
    Default,
    Random,
}

/// The authoritative match state.
pub struct Game {
    map: Map,
    entities: EntityStore,
    fog: Fog,
    players: Vec<PlayerState>,
    /// One controller per AI-owned player. Driven at the top of [`tick`] to enqueue commands;
    /// empty for an all-human match.
    ai: Vec<AiController>,
    /// Commands received this tick window, drained at the start of [`tick`]. Each carries the
    /// issuing player so ownership can be validated on apply.
    pending: Vec<(u32, SimCommand)>,
    /// Authoritative commands stamped with the tick where they were applied. Includes AI commands
    /// because they are emitted into the same pending queue before command application.
    command_log: Vec<CommandLogEntry>,
    tick: u32,
    /// Post-tick spatial index, rebuilt every tick after all systems run so [`snapshot_for`]
    /// can use it for interest filtering without rebuilding.
    spatial: services::spatial::SpatialIndex,
    /// Persistent pathfinding service with an LRU cache for verified paths.
    pathing: services::pathing::PathingService,
    /// Five-second death-vision sources used only when building fog-filtered snapshots.
    lingering_sight: Vec<LingeringSightSource>,
    /// Neutral smoke clouds that block authoritative fog and combat LOS without being entities.
    smokes: SmokeCloudStore,
    /// Match seed retained for replay metadata/API compatibility. The current hardcoded map
    /// ignores it until lobby map selection or randomized maps are reintroduced.
    seed: u32,
    /// Starting steel granted to each player at match start. Retained so replay artifacts can
    /// faithfully recreate debug-mode economy.
    starting_steel: u32,
    /// Starting oil granted to each player at match start. See [`Game::starting_steel`].
    starting_oil: u32,
    /// True for lobby "Debug mode" matches; enables owner-only movement path diagnostics in
    /// snapshots even when the server binary is built in release mode.
    debug_path_overlays: bool,
    pub(crate) rng: SmallRng,
}

impl Game {
    /// Advance the simulation by one tick and return per-player transient events.
    ///
    /// Ordered per `docs/design/server-sim.md`: drain+apply commands → movement → combat → gather →
    /// production+spawn → construction → deaths → recompute supply → recompute fog. The whole
    /// method is panic-free: every entity lookup is fallible and stale ids are ignored.
    pub fn tick(&mut self) -> Vec<(u32, Vec<Event>)> {
        self.tick_inner(None)
    }

    pub fn tick_with_perf(
        &mut self,
        perf: Option<&mut crate::perf::TickPerf>,
    ) -> Vec<(u32, Vec<Event>)> {
        self.tick_inner(perf)
    }

    fn tick_inner(
        &mut self,
        mut perf: Option<&mut crate::perf::TickPerf>,
    ) -> Vec<(u32, Vec<Event>)> {
        self.tick = self.tick.wrapping_add(1);
        self.pathing.advance_tick(self.tick);
        self.smokes.retain_active(self.tick);

        // Per-player event buckets, accumulated by the systems below.
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        for p in &self.players {
            events.entry(p.id).or_default();
        }

        // Let each AI player decide its actions first, appending ordinary commands to the same
        // pending queue a human client feeds. They are validated on apply just like any client
        // command — the AI gets no special authority over the simulation. Disjoint field borrows
        // (`self.ai` mutably, the rest shared) keep this lock-free.
        let pending = crate::perf::timed(perf.as_deref_mut(), "ai_think", || {
            let mut pending = std::mem::take(&mut self.pending);
            for controller in self.ai.iter_mut() {
                controller.think(
                    AiThinkContext {
                        map: &self.map,
                        entities: &self.entities,
                        fog: &self.fog,
                        spatial: &self.spatial,
                        players: &self.players,
                        tick: self.tick,
                    },
                    &mut pending,
                );
            }
            pending
        });
        crate::perf::timed(perf.as_deref_mut(), "record_commands", || {
            self.record_commands_for_tick(&pending);
        });

        // Run every per-tick system in order. `run_tick` takes split borrows of the map,
        // entity store, player economy, and the event buckets, so it can mutate resources and
        // entities together without locks.
        self.spatial = systems::run_tick(
            &self.map,
            &mut self.entities,
            &mut self.players,
            &self.fog,
            &mut self.pathing,
            &mut self.rng,
            &mut self.lingering_sight,
            &mut self.smokes,
            pending,
            &mut events,
            self.tick,
            perf.as_deref_mut(),
        );

        // Live fog last, from the post-systems world state. Lingering death vision is layered
        // only when snapshots are projected so it cannot validate commands or combat targeting.
        self.lingering_sight
            .retain(|source| source.is_active_at(self.tick));
        let ids: Vec<u32> = self.players.iter().map(|p| p.id).collect();
        crate::perf::timed(perf.as_deref_mut(), "fog_recompute", || {
            self.fog
                .recompute_with_smoke(&ids, &self.entities, &self.map, &self.smokes);
        });

        // In debug builds, assert that the world state is internally consistent.
        // Panics here mean a system violated a documented invariant.
        #[cfg(debug_assertions)]
        crate::perf::timed(perf, "debug_invariants", || {
            self.assert_invariants();
        });

        // Return events in a stable order (by player id) for determinism.
        let mut out: Vec<(u32, Vec<Event>)> = events.into_iter().collect();
        out.sort_by_key(|(pid, _)| *pid);
        out
    }

    pub fn current_tick(&self) -> u32 {
        self.tick
    }

    pub fn perf_entity_counts(&self) -> crate::perf::EntityCounts {
        let mut counts = crate::perf::EntityCounts::default();
        for entity in self.entities.iter() {
            counts.entities += 1;
            if entity.is_unit() {
                counts.units += 1;
            } else if entity.is_building() {
                counts.buildings += 1;
            } else if entity.is_node() {
                counts.resources += 1;
            }
        }
        counts
    }

    /// Player ids that are not yet defeated. Human players are defeated when they lose all
    /// buildings; AI players are also defeated when they have no units left.
    pub fn alive_players(&self) -> Vec<u32> {
        self.players
            .iter()
            .filter(|p| {
                let has_building = services::world_query::owned_buildings(&self.entities, p.id)
                    .next()
                    .is_some();
                if !has_building {
                    return false;
                }
                if p.is_ai {
                    services::world_query::owned_units(&self.entities, p.id)
                        .next()
                        .is_some()
                } else {
                    true
                }
            })
            .map(|p| p.id)
            .collect()
    }

    /// Remove every entity owned by `player` (e.g. on disconnect) so the match can resolve.
    pub fn eliminate(&mut self, player: u32) {
        let doomed: Vec<u32> = services::world_query::owned_units(&self.entities, player)
            .chain(services::world_query::owned_buildings(
                &self.entities,
                player,
            ))
            .map(|e| e.id)
            .collect();
        for id in doomed {
            if let Some(entity) = self.entities.remove(id) {
                if let Some(p) = self.players.iter_mut().find(|p| p.id == entity.owner) {
                    p.record_entity_lost(entity.kind);
                }
            }
        }
        if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
            p.supply_used = 0;
            p.supply_cap = 0;
        }
        self.lingering_sight
            .retain(|source| source.owner() != player);
        // Recompute fog so the now-entity-less player's visibility goes dark immediately;
        // otherwise a stale grid would keep leaking neutral/enemy entities into their snapshots.
        let ids: Vec<u32> = self.players.iter().map(|p| p.id).collect();
        self.fog
            .recompute_with_smoke(&ids, &self.entities, &self.map, &self.smokes);
    }

    pub fn tick_count(&self) -> u32 {
        self.tick
    }

    #[allow(dead_code)]
    #[cfg(any(test, debug_assertions))]
    pub(crate) fn spawn_smoke_cloud_for_test(&mut self, x: f32, y: f32) -> Option<u32> {
        let (x, y) = SmokeCloudStore::clamp_point_to_map(&self.map, x, y)?;
        let id = self.smokes.spawn(
            x,
            y,
            config::SMOKE_CLOUD_RADIUS_TILES,
            config::SMOKE_CLOUD_DURATION_TICKS,
            self.tick,
        )?;
        let ids: Vec<u32> = self.players.iter().map(|p| p.id).collect();
        self.fog
            .recompute_with_smoke(&ids, &self.entities, &self.map, &self.smokes);
        Some(id)
    }

    /// Authoritative commands applied so far, in exact application order.
    #[allow(dead_code)]
    pub fn command_log(&self) -> &[CommandLogEntry] {
        &self.command_log
    }

    /// Reconstruct the `PlayerInit` list this game was created from, so a crash/invariant
    /// failure can persist a replayable artifact.
    pub fn player_inits(&self) -> Vec<PlayerInit> {
        self.players
            .iter()
            .map(|p| PlayerInit {
                id: p.id,
                name: p.name.clone(),
                color: p.color.clone(),
                is_ai: p.is_ai,
            })
            .collect()
    }

    // --- internal helpers ------------------------------------------------------
}

#[cfg(test)]
mod tests;
