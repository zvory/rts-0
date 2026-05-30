//! Test-only API-driven self-play harness.
//!
//! This deliberately drives the public [`Game`] seam (`enqueue`, `tick`, `snapshot_for`) instead
//! of reaching into simulation internals. The scripted players behave like deterministic API
//! clients: observe a fog-filtered snapshot, issue ordinary commands, and let the authoritative
//! simulation validate every action.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::replay::{replay_commands, EventLogEntry, PlayerSnapshot, ReplayOutcome};
use super::{Game, PlayerInit};
use crate::config;
use crate::game::entity::EntityKind;
use crate::protocol::{
    kinds, states, terrain, Command, EntityView, Event, MapInfo, Snapshot, StartPayload,
};

/// Parse an `EntityView` wire kind into its internal enum.
fn kind_of(e: &EntityView) -> Option<EntityKind> {
    e.kind.parse().ok()
}

/// Convenience: check whether an `EntityView` has a given internal kind.
fn is_kind(e: &EntityView, kind: EntityKind) -> bool {
    e.kind == kind.to_protocol_str()
}

const MAX_TICKS: u32 = 9_600;
const MAX_STALL_TICKS: u32 = 1_800;
const SAMPLE_EVERY_TICKS: u32 = 30;
const THINK_INTERVAL: u32 = 6;
const ATTACK_REISSUE_TICKS: u32 = 120;
const RESOURCE_SANITY_LIMIT: u32 = 1_000_000;
const ECONOMY_TARGET_WORKERS: usize = 8;

trait ScriptedPlayer: Send {
    fn player_id(&self) -> u32;
    fn name(&self) -> &'static str;
    fn commands(&mut self, view: PlayerView<'_>) -> Vec<Command>;
}

pub(crate) struct LiveSelfPlay {
    players: Vec<PlayerInit>,
    scripts: Vec<Box<dyn ScriptedPlayer>>,
}

impl LiveSelfPlay {
    pub(crate) fn default_match() -> Self {
        let players = vec![
            PlayerInit {
                id: 1,
                name: "Alpha Script".to_string(),
                color: "#6f8fa8".to_string(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                name: "Bravo Script".to_string(),
                color: "#b2775f".to_string(),
                is_ai: false,
            },
        ];
        let scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
            Box::new(BuildTechAttackScript::new(players[0].id)),
            Box::new(BuildTechAttackScript::new(players[1].id)),
        ];
        Self { players, scripts }
    }

    pub(crate) fn players(&self) -> &[PlayerInit] {
        &self.players
    }

    pub(crate) fn enqueue_for_tick(&mut self, game: &mut Game) {
        let tick = game.tick_count();
        let start = game.start_payload();
        let mut commands = Vec::new();
        for script in &mut self.scripts {
            let player_id = script.player_id();
            let snapshot = game.snapshot_for(player_id);
            let view = PlayerView {
                player_id,
                tick,
                start: &start,
                snapshot: &snapshot,
            };
            for command in script.commands(view) {
                commands.push((player_id, command));
            }
        }
        for (player_id, command) in commands {
            game.enqueue(player_id, command);
        }
    }
}

#[derive(Clone, Deserialize)]
pub(crate) struct ReplayArtifact {
    pub(crate) replay_commands: Vec<super::replay::CommandLogEntry>,
    pub(crate) players: Vec<PlayerInit>,
}

pub(crate) struct ReplayDriver {
    commands: Vec<super::replay::CommandLogEntry>,
    next: usize,
}

impl ReplayDriver {
    pub(crate) fn from_artifact(artifact: ReplayArtifact) -> (Vec<PlayerInit>, Self) {
        (
            artifact.players,
            Self {
                commands: artifact.replay_commands,
                next: 0,
            },
        )
    }

    pub(crate) fn enqueue_for_tick(&mut self, game: &mut Game) {
        let next_tick = game.tick_count().saturating_add(1);
        while let Some(entry) = self.commands.get(self.next) {
            if entry.tick != next_tick {
                break;
            }
            game.enqueue(entry.player_id, entry.command.clone());
            self.next += 1;
        }
    }
}

#[derive(Clone, Copy)]
struct PlayerView<'a> {
    player_id: u32,
    tick: u32,
    start: &'a StartPayload,
    snapshot: &'a Snapshot,
}

struct BuildTechAttackScript {
    player_id: u32,
    target_workers: usize,
    oil_workers: usize,
    target_barracks: usize,
    attack_size: usize,
    assigned_oil_workers: BTreeSet<u32>,
    initial_gather_sent: bool,
    last_attack_tick: u32,
    last_oil_assignment_tick: u32,
    /// Outstanding Build orders we've issued: worker_id → (building kind, target tile).
    /// Under the "reserve on arrival" model the building doesn't appear in the snapshot
    /// until the worker arrives at the site, so without this we'd keep re-issuing the
    /// same build every think tick. Entries are pruned when the worker leaves the BUILD
    /// state.
    pending_builds: BTreeMap<u32, (EntityKind, u32, u32, u32)>,
    /// Tiles where a Build order failed at arrival (worker went idle without a building
    /// appearing). Used to bias `find_build_spot` away from spots the server has
    /// rejected so the script doesn't loop on the same deterministic tile forever.
    failed_build_spots: HashMap<EntityKind, BTreeSet<(u32, u32)>>,
}

const FAILED_SPOTS_CAP: usize = 16;
/// Force a pending build to be treated as failed if the building still hasn't appeared
/// after this many ticks. Covers cases where the worker gets stuck pathing to the site
/// (e.g. blocked by hard collision in a tight corner) and never leaves BUILD state —
/// without this the entry would pin the spot forever.
const PENDING_BUILD_WATCHDOG_TICKS: u32 = 300;

impl BuildTechAttackScript {
    fn new(player_id: u32) -> Self {
        BuildTechAttackScript {
            player_id,
            target_workers: 10,
            oil_workers: 2,
            target_barracks: 1,
            attack_size: 4,
            assigned_oil_workers: BTreeSet::new(),
            initial_gather_sent: false,
            last_attack_tick: 0,
            last_oil_assignment_tick: 0,
            pending_builds: BTreeMap::new(),
            failed_build_spots: HashMap::new(),
        }
    }

    fn should_think(&self, tick: u32) -> bool {
        tick == 0 || tick.wrapping_add(self.player_id) % THINK_INTERVAL == 0
    }
}

impl ScriptedPlayer for BuildTechAttackScript {
    fn player_id(&self) -> u32 {
        self.player_id
    }

    fn name(&self) -> &'static str {
        "build-tech-attack"
    }

    fn commands(&mut self, view: PlayerView<'_>) -> Vec<Command> {
        if !self.should_think(view.tick) {
            return Vec::new();
        }

        let own: Vec<&EntityView> = view
            .snapshot
            .entities
            .iter()
            .filter(|e| e.owner == view.player_id)
            .collect();
        let workers: Vec<&EntityView> = own
            .iter()
            .copied()
            .filter(|e| is_kind(e, EntityKind::Worker))
            .collect();

        // Drop pending-build entries whose worker is no longer in BUILD state — either the
        // worker arrived and the building was spawned (it will appear in `own`), or the
        // arrival failed/order was cleared and the worker went idle. For dropped entries,
        // distinguish success (building of that kind now overlaps the target tile) from
        // failure (no such building) and record failed tiles so future picks avoid them.
        let mut dropped: Vec<(EntityKind, u32, u32)> = Vec::new();
        let now = view.tick;
        self.pending_builds
            .retain(|wid, (kind, tx, ty, issued_tick)| {
                let in_build = workers
                    .iter()
                    .any(|w| w.id == *wid && w.state == states::BUILD);
                let watchdog_expired =
                    now.saturating_sub(*issued_tick) >= PENDING_BUILD_WATCHDOG_TICKS;
                let keep = in_build && !watchdog_expired;
                if !keep {
                    dropped.push((*kind, *tx, *ty));
                }
                keep
            });
        for (kind, tx, ty) in dropped {
            let succeeded = own.iter().any(|e| {
                is_kind(e, kind) && building_footprint_tiles(&view.start.map, e).contains(&(tx, ty))
            });
            if succeeded {
                self.failed_build_spots.remove(&kind);
            } else {
                let set = self.failed_build_spots.entry(kind).or_default();
                set.insert((tx, ty));
                if set.len() > FAILED_SPOTS_CAP {
                    set.clear();
                }
            }
        }

        let mut idle_workers: Vec<u32> = workers
            .iter()
            .filter(|e| e.state == states::IDLE)
            .map(|e| e.id)
            .collect();
        idle_workers.sort_unstable();
        let mut builder_workers: Vec<u32> = idle_workers
            .iter()
            .copied()
            .filter(|id| !self.assigned_oil_workers.contains(id))
            .collect();
        builder_workers.extend(
            workers
                .iter()
                .filter(|e| e.state != states::IDLE && e.state != states::BUILD)
                .filter(|e| !self.assigned_oil_workers.contains(&e.id))
                .map(|e| e.id),
        );

        let industrial_centers: Vec<&EntityView> = own
            .iter()
            .copied()
            .filter(|e| is_kind(e, EntityKind::IndustrialCenter) && is_complete(e))
            .collect();
        let barracks: Vec<&EntityView> = own
            .iter()
            .copied()
            .filter(|e| is_kind(e, EntityKind::Barracks))
            .collect();
        let complete_barracks: Vec<&EntityView> = barracks
            .iter()
            .copied()
            .filter(|e| is_complete(e))
            .collect();
        let tank_factories: Vec<&EntityView> = own
            .iter()
            .copied()
            .filter(|e| is_kind(e, EntityKind::TankFactory))
            .collect();
        let complete_tank_factories: Vec<&EntityView> = tank_factories
            .iter()
            .copied()
            .filter(|e| is_complete(e))
            .collect();
        let pending_count = |kind: EntityKind| -> usize {
            self.pending_builds
                .values()
                .filter(|(k, _, _, _)| *k == kind)
                .count()
        };
        let depot_count = own.iter().filter(|e| is_kind(e, EntityKind::Depot)).count()
            + pending_count(EntityKind::Depot);
        let depot_under_construction = own
            .iter()
            .any(|e| is_kind(e, EntityKind::Depot) && !is_complete(e))
            || pending_count(EntityKind::Depot) > 0;
        let barracks_count = barracks.len() + pending_count(EntityKind::Barracks);
        let tank_factory_count = tank_factories.len() + pending_count(EntityKind::TankFactory);
        let tank_count = own.iter().filter(|e| is_kind(e, EntityKind::Tank)).count();

        let mut steel = view.snapshot.steel;
        let mut oil = view.snapshot.oil;
        let mut free_supply = view
            .snapshot
            .supply_cap
            .saturating_sub(view.snapshot.supply_used);
        let mut reserved_workers = HashSet::new();
        let mut out = Vec::new();

        // Build tech before spending the early economy on extra workers.
        let needs_barracks = complete_barracks.len() < self.target_barracks
            && barracks_count < self.target_barracks + 1;
        if needs_barracks {
            if let Some(cmd) = self.build_if_affordable(
                view,
                EntityKind::Barracks,
                &mut steel,
                &builder_workers,
                &mut reserved_workers,
            ) {
                out.push(cmd);
            }
        }

        let wants_depot = !depot_under_construction
            && (view.snapshot.supply_cap < config::INDUSTRIAL_CENTER_SUPPLY + config::DEPOT_SUPPLY
                || free_supply <= 4
                || (depot_count == 0 && !complete_barracks.is_empty()));
        if wants_depot {
            if let Some(cmd) = self.build_if_affordable(
                view,
                EntityKind::Depot,
                &mut steel,
                &builder_workers,
                &mut reserved_workers,
            ) {
                out.push(cmd);
            }
        }

        let needs_tank_factory = !complete_barracks.is_empty() && tank_factory_count == 0;
        if needs_tank_factory {
            if let Some(cmd) = self.build_if_affordable(
                view,
                EntityKind::TankFactory,
                &mut steel,
                &builder_workers,
                &mut reserved_workers,
            ) {
                out.push(cmd);
            }
        }

        for industrial_center in industrial_centers {
            if workers.len() >= self.target_workers {
                break;
            }
            if production_queue_len(industrial_center) > 0 {
                continue;
            }
            let Some(stats) = config::unit_stats(EntityKind::Worker) else {
                continue;
            };
            if steel < stats.cost_steel || oil < stats.cost_oil || free_supply < stats.supply {
                break;
            }
            out.push(Command::Train {
                building: industrial_center.id,
                unit: kinds::WORKER.to_string(),
            });
            steel -= stats.cost_steel;
            oil -= stats.cost_oil;
            free_supply -= stats.supply;
        }

        let saving_for_first_tank =
            needs_tank_factory || (tank_count == 0 && !complete_tank_factories.is_empty());
        if !saving_for_first_tank {
            for rax in complete_barracks {
                if production_queue_len(rax) > 0 {
                    continue;
                }

                let Some(stats) = config::unit_stats(EntityKind::Rifleman) else {
                    continue;
                };
                if steel < stats.cost_steel || oil < stats.cost_oil || free_supply < stats.supply {
                    continue;
                }
                out.push(Command::Train {
                    building: rax.id,
                    unit: kinds::RIFLEMAN.to_string(),
                });
                steel -= stats.cost_steel;
                oil -= stats.cost_oil;
                free_supply -= stats.supply;
            }
        }

        for factory in complete_tank_factories {
            if tank_count > 0 || production_queue_len(factory) > 0 {
                continue;
            }
            let Some(stats) = config::unit_stats(EntityKind::Tank) else {
                continue;
            };
            if steel < stats.cost_steel || oil < stats.cost_oil || free_supply < stats.supply {
                continue;
            }
            out.push(Command::Train {
                building: factory.id,
                unit: kinds::TANK.to_string(),
            });
            steel -= stats.cost_steel;
            oil -= stats.cost_oil;
            free_supply -= stats.supply;
        }

        self.assign_workers(view, &workers, &reserved_workers, &mut out);

        let combat_units: Vec<u32> = own
            .iter()
            .filter(|e| is_kind(e, EntityKind::Rifleman) || is_kind(e, EntityKind::Tank))
            .map(|e| e.id)
            .collect();
        let has_tech_unit = tank_count > 0;
        let has_army = combat_units.len() >= self.attack_size;
        let attack_due = view.tick.saturating_sub(self.last_attack_tick) >= ATTACK_REISSUE_TICKS;
        if has_army && has_tech_unit && attack_due {
            let (x, y) = combat_rendezvous_world(view);
            out.push(Command::AttackMove {
                units: combat_units,
                x,
                y,
            });
            self.last_attack_tick = view.tick;
        }

        out
    }
}

impl BuildTechAttackScript {
    fn build_if_affordable(
        &mut self,
        view: PlayerView<'_>,
        building: EntityKind,
        steel: &mut u32,
        idle_workers: &[u32],
        reserved_workers: &mut HashSet<u32>,
    ) -> Option<Command> {
        // Local steel debit is a conservative reservation, not a mirror of server state:
        // under the "reserve on arrival" build model the server only deducts when the
        // worker arrives at the site. The script over-reserves locally to avoid
        // double-spending across decision ticks.
        let stats = config::building_stats(building)?;
        if *steel < stats.cost_steel {
            return None;
        }
        let worker = idle_workers
            .iter()
            .copied()
            .find(|id| !reserved_workers.contains(id))?;
        let start = own_start_tile(view.start, view.player_id)?;
        let empty = BTreeSet::new();
        let skip = self.failed_build_spots.get(&building).unwrap_or(&empty);
        let (tile_x, tile_y) =
            find_build_spot(&view.start.map, view.snapshot, start, building, skip)?;
        reserved_workers.insert(worker);
        self.pending_builds
            .insert(worker, (building, tile_x, tile_y, view.tick));
        *steel -= stats.cost_steel;
        Some(Command::Build {
            worker,
            building: building.to_protocol_str().to_string(),
            tile_x,
            tile_y,
        })
    }

    fn assign_workers(
        &mut self,
        view: PlayerView<'_>,
        workers: &[&EntityView],
        reserved_workers: &HashSet<u32>,
        out: &mut Vec<Command>,
    ) {
        let steel_nodes: Vec<&EntityView> = view
            .snapshot
            .entities
            .iter()
            .filter(|e| {
                e.owner == 0 && is_kind(e, EntityKind::Steel) && e.remaining.unwrap_or(0) > 0
            })
            .collect();
        let oil_nodes: Vec<&EntityView> = view
            .snapshot
            .entities
            .iter()
            .filter(|e| e.owner == 0 && is_kind(e, EntityKind::Oil) && e.remaining.unwrap_or(0) > 0)
            .collect();
        if steel_nodes.is_empty() && oil_nodes.is_empty() {
            return;
        }

        let mut sorted_workers: Vec<&EntityView> = workers.to_vec();
        sorted_workers.sort_by_key(|w| w.id);

        let can_assign_oil = self.assigned_oil_workers.len() < self.oil_workers
            && view.tick > 0
            && view.tick.saturating_sub(self.last_oil_assignment_tick) >= 90
            && !oil_nodes.is_empty()
            && view
                .snapshot
                .entities
                .iter()
                .any(|e| e.owner == view.player_id && is_kind(e, EntityKind::Barracks));
        let mut assigned_nodes: HashSet<u32> = HashSet::new();
        if can_assign_oil {
            let mut assigned = 0usize;
            for worker in &sorted_workers {
                if assigned >= self.oil_workers {
                    break;
                }
                if reserved_workers.contains(&worker.id) {
                    continue;
                }
                if worker.state == states::BUILD {
                    continue;
                }
                if worker.latched_node.is_some() {
                    continue;
                }
                if let Some(node) = nearest_unassigned_node(worker, &oil_nodes, &assigned_nodes) {
                    out.push(Command::Gather {
                        units: vec![worker.id],
                        node,
                    });
                    self.assigned_oil_workers.insert(worker.id);
                    assigned += 1;
                    assigned_nodes.insert(node);
                }
            }
            self.last_oil_assignment_tick = view.tick;
        }

        for worker in sorted_workers {
            if reserved_workers.contains(&worker.id) {
                continue;
            }
            if self.assigned_oil_workers.contains(&worker.id) {
                continue;
            }
            if self.initial_gather_sent && worker.state != states::IDLE {
                continue;
            }
            if worker.latched_node.is_some() {
                continue;
            }
            if let Some(node) = nearest_unassigned_node(worker, &steel_nodes, &assigned_nodes) {
                out.push(Command::Gather {
                    units: vec![worker.id],
                    node,
                });
                assigned_nodes.insert(node);
            }
        }
        self.initial_gather_sent = true;
    }
}

struct EconomyScript {
    player_id: u32,
    target_workers: usize,
    initial_gather_sent: bool,
}

impl EconomyScript {
    fn new(player_id: u32) -> Self {
        EconomyScript {
            player_id,
            target_workers: ECONOMY_TARGET_WORKERS,
            initial_gather_sent: false,
        }
    }

    fn should_think(&self, tick: u32) -> bool {
        tick == 0 || tick.wrapping_add(self.player_id) % THINK_INTERVAL == 0
    }
}

impl ScriptedPlayer for EconomyScript {
    fn player_id(&self) -> u32 {
        self.player_id
    }

    fn name(&self) -> &'static str {
        "economy"
    }

    fn commands(&mut self, view: PlayerView<'_>) -> Vec<Command> {
        if !self.should_think(view.tick) {
            return Vec::new();
        }

        let own: Vec<&EntityView> = view
            .snapshot
            .entities
            .iter()
            .filter(|e| e.owner == view.player_id)
            .collect();
        let workers: Vec<&EntityView> = own
            .iter()
            .copied()
            .filter(|e| is_kind(e, EntityKind::Worker))
            .collect();
        let industrial_centers: Vec<&EntityView> = own
            .iter()
            .copied()
            .filter(|e| is_kind(e, EntityKind::IndustrialCenter) && is_complete(e))
            .collect();

        let mut builder_workers: Vec<u32> = workers
            .iter()
            .filter(|e| e.state == states::IDLE)
            .map(|e| e.id)
            .collect();
        builder_workers.sort_unstable();
        builder_workers.extend(
            workers
                .iter()
                .filter(|e| e.state != states::IDLE && e.state != states::BUILD)
                .map(|e| e.id),
        );

        let depot_under_construction = own
            .iter()
            .any(|e| is_kind(e, EntityKind::Depot) && !is_complete(e));
        let mut steel = view.snapshot.steel;
        let mut oil = view.snapshot.oil;
        let mut free_supply = view
            .snapshot
            .supply_cap
            .saturating_sub(view.snapshot.supply_used);
        let mut reserved_workers = HashSet::new();
        let mut out = Vec::new();

        if !depot_under_construction
            && free_supply <= 2
            && view.snapshot.supply_cap < config::SUPPLY_CAP_MAX
        {
            if let Some(cmd) = build_near_own_start_if_affordable(
                view,
                EntityKind::Depot,
                &mut steel,
                &builder_workers,
                &mut reserved_workers,
            ) {
                out.push(cmd);
            }
        }

        for industrial_center in industrial_centers {
            if workers.len() >= self.target_workers {
                break;
            }
            if production_queue_len(industrial_center) > 0 {
                continue;
            }
            let Some(stats) = config::unit_stats(EntityKind::Worker) else {
                continue;
            };
            if steel < stats.cost_steel || oil < stats.cost_oil || free_supply < stats.supply {
                break;
            }
            out.push(Command::Train {
                building: industrial_center.id,
                unit: kinds::WORKER.to_string(),
            });
            steel -= stats.cost_steel;
            oil -= stats.cost_oil;
            free_supply -= stats.supply;
        }

        assign_steel_workers(
            view,
            &workers,
            &reserved_workers,
            self.initial_gather_sent,
            &mut out,
        );
        self.initial_gather_sent = true;

        out
    }
}

struct WorkerRushScript {
    player_id: u32,
    target_player_id: u32,
    last_attack_tick: u32,
}

impl WorkerRushScript {
    fn new(player_id: u32, target_player_id: u32) -> Self {
        WorkerRushScript {
            player_id,
            target_player_id,
            last_attack_tick: 0,
        }
    }

    fn should_think(&self, tick: u32) -> bool {
        tick == 0 || tick.wrapping_add(self.player_id) % THINK_INTERVAL == 0
    }
}

impl ScriptedPlayer for WorkerRushScript {
    fn player_id(&self) -> u32 {
        self.player_id
    }

    fn name(&self) -> &'static str {
        "worker-rush"
    }

    fn commands(&mut self, view: PlayerView<'_>) -> Vec<Command> {
        if !self.should_think(view.tick) {
            return Vec::new();
        }
        let workers: Vec<u32> = view
            .snapshot
            .entities
            .iter()
            .filter(|e| e.owner == view.player_id && is_kind(e, EntityKind::Worker))
            .map(|e| e.id)
            .collect();
        if workers.is_empty() {
            return Vec::new();
        }
        let attack_due = view.tick == 0
            || view.tick.saturating_sub(self.last_attack_tick) >= ATTACK_REISSUE_TICKS;
        if !attack_due {
            return Vec::new();
        }
        let Some((x, y)) = player_start_world(view.start, self.target_player_id) else {
            return Vec::new();
        };
        self.last_attack_tick = view.tick;
        vec![Command::AttackMove {
            units: workers,
            x,
            y,
        }]
    }
}

struct BunkerRushScript {
    player_id: u32,
    target_player_id: u32,
    initial_gather_sent: bool,
    last_bunker_attempt_tick: u32,
    scouts: Vec<u32>,
}

impl BunkerRushScript {
    fn new(player_id: u32, target_player_id: u32) -> Self {
        BunkerRushScript {
            player_id,
            target_player_id,
            initial_gather_sent: false,
            last_bunker_attempt_tick: 0,
            scouts: Vec::new(),
        }
    }

    fn should_think(&self, tick: u32) -> bool {
        tick == 0 || tick.wrapping_add(self.player_id) % THINK_INTERVAL == 0
    }
}

impl ScriptedPlayer for BunkerRushScript {
    fn player_id(&self) -> u32 {
        self.player_id
    }

    fn name(&self) -> &'static str {
        "bunker-rush"
    }

    fn commands(&mut self, view: PlayerView<'_>) -> Vec<Command> {
        if !self.should_think(view.tick) {
            return Vec::new();
        }

        let own: Vec<&EntityView> = view
            .snapshot
            .entities
            .iter()
            .filter(|e| e.owner == view.player_id)
            .collect();
        let workers: Vec<&EntityView> = own
            .iter()
            .copied()
            .filter(|e| is_kind(e, EntityKind::Worker))
            .collect();
        let mut builder_workers: Vec<u32> = workers
            .iter()
            .filter(|e| e.state == states::IDLE)
            .map(|e| e.id)
            .collect();
        builder_workers.sort_unstable();
        builder_workers.extend(
            workers
                .iter()
                .filter(|e| e.state != states::IDLE && e.state != states::BUILD)
                .map(|e| e.id),
        );

        let mut steel = view.snapshot.steel;
        let mut reserved_workers = HashSet::new();
        let mut out = Vec::new();

        // Pre-position scouts toward the enemy base so they can start building immediately
        // when we can afford bunkers.
        if let Some((target_x, target_y)) = player_start_world(view.start, self.target_player_id) {
            let mut candidates: Vec<u32> = workers.iter().map(|w| w.id).collect();
            candidates.sort_unstable();
            for worker_id in candidates {
                if self.scouts.contains(&worker_id) {
                    continue;
                }
                if self.scouts.len() >= 2 {
                    break;
                }
                out.push(Command::Move {
                    units: vec![worker_id],
                    x: target_x,
                    y: target_y,
                });
                self.scouts.push(worker_id);
            }
        }
        reserved_workers.extend(&self.scouts);

        let bunker_count = own
            .iter()
            .filter(|e| is_kind(e, EntityKind::Bunker))
            .count();
        let bunker_attempt_due = view.tick == 0
            || view.tick.saturating_sub(self.last_bunker_attempt_tick) >= ATTACK_REISSUE_TICKS;
        if bunker_count < 2 && bunker_attempt_due {
            if let Some(cmd) = self.build_offensive_bunker_if_affordable(
                view,
                &mut steel,
                &builder_workers,
                &mut reserved_workers,
            ) {
                out.push(cmd);
                self.last_bunker_attempt_tick = view.tick;
            }
        }

        assign_steel_workers(
            view,
            &workers,
            &reserved_workers,
            self.initial_gather_sent,
            &mut out,
        );
        self.initial_gather_sent = true;

        out
    }
}

impl BunkerRushScript {
    fn build_offensive_bunker_if_affordable(
        &self,
        view: PlayerView<'_>,
        steel: &mut u32,
        builder_workers: &[u32],
        reserved_workers: &mut HashSet<u32>,
    ) -> Option<Command> {
        let stats = config::building_stats(EntityKind::Bunker)?;
        if *steel < stats.cost_steel {
            return None;
        }
        let worker = builder_workers
            .iter()
            .copied()
            .find(|id| !reserved_workers.contains(id))?;
        let (tile_x, tile_y) =
            find_offensive_bunker_spot(view.start, view.snapshot, self.target_player_id)?;
        reserved_workers.insert(worker);
        *steel -= stats.cost_steel;
        Some(Command::Build {
            worker,
            building: EntityKind::Bunker.to_protocol_str().to_string(),
            tile_x,
            tile_y,
        })
    }
}

struct SelfPlayRunner {
    test_name: &'static str,
    game: Game,
    start: StartPayload,
    player_specs: Vec<PlayerSpec>,
    scripts: Vec<Box<dyn ScriptedPlayer>>,
    commands: Vec<CommandRecord>,
    replay_commands_len: usize,
    events: Vec<EventRecord>,
    event_log: Vec<EventLogEntry>,
    samples: Vec<SnapshotSample>,
    milestones: Milestones,
}

impl SelfPlayRunner {
    fn new(
        test_name: &'static str,
        game: Game,
        start: StartPayload,
        player_specs: Vec<PlayerSpec>,
        scripts: Vec<Box<dyn ScriptedPlayer>>,
    ) -> Self {
        let milestones = Milestones::tech_combat_for_players(player_specs.iter().map(|p| p.id));
        SelfPlayRunner::with_milestones(test_name, game, start, player_specs, scripts, milestones)
    }

    fn with_milestones(
        test_name: &'static str,
        game: Game,
        start: StartPayload,
        player_specs: Vec<PlayerSpec>,
        scripts: Vec<Box<dyn ScriptedPlayer>>,
        milestones: Milestones,
    ) -> Self {
        SelfPlayRunner {
            test_name,
            game,
            start,
            player_specs,
            scripts,
            commands: Vec::new(),
            replay_commands_len: 0,
            events: Vec::new(),
            event_log: Vec::new(),
            samples: Vec::new(),
            milestones,
        }
    }

    fn run(&mut self) -> Result<SelfPlayReport, SelfPlayFailure> {
        let mut last_progress_tick = 0;

        for _ in 0..=MAX_TICKS {
            let tick = self.game.tick_count();
            let snapshots = self.current_snapshots();
            self.validate_snapshots(&snapshots)?;
            if self.record_observations(tick, &snapshots) {
                last_progress_tick = tick;
            }
            if self.milestones.complete() {
                return Ok(SelfPlayReport {
                    ticks: tick,
                    commands: self.commands.len(),
                    replay_commands: self.replay_commands_len,
                });
            }
            if tick >= MAX_TICKS {
                break;
            }
            if tick.saturating_sub(last_progress_tick) > MAX_STALL_TICKS {
                return Err(SelfPlayFailure::new(format!(
                    "self-play stalled for more than {MAX_STALL_TICKS} ticks before all milestones"
                )));
            }

            let mut commands = Vec::new();
            for script in &mut self.scripts {
                let pid = script.player_id();
                let Some(snapshot) = snapshots.get(&pid) else {
                    continue;
                };
                let view = PlayerView {
                    player_id: pid,
                    tick,
                    start: &self.start,
                    snapshot,
                };
                for command in script.commands(view) {
                    commands.push((pid, script.name(), command));
                }
            }

            for (player_id, script, command) in commands {
                self.commands.push(CommandRecord {
                    tick,
                    player_id,
                    script,
                    command: command.clone(),
                });
                self.game.enqueue(player_id, command);
            }

            let tick_events =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.game.tick()))
                    .map_err(|_| SelfPlayFailure::new("Game::tick panicked during self-play"))?;
            self.replay_commands_len = self.game.command_log().len();
            if self.record_events(self.game.tick_count(), tick_events) {
                last_progress_tick = self.game.tick_count();
            }
        }

        Err(SelfPlayFailure::new(format!(
            "self-play did not complete all milestones within {MAX_TICKS} ticks: {}",
            self.milestones.missing_summary()
        )))
    }

    fn current_snapshots(&self) -> BTreeMap<u32, Snapshot> {
        self.player_specs
            .iter()
            .map(|p| (p.id, self.game.snapshot_for(p.id)))
            .collect()
    }

    fn validate_snapshots(
        &self,
        snapshots: &BTreeMap<u32, Snapshot>,
    ) -> Result<(), SelfPlayFailure> {
        for (player_id, snapshot) in snapshots {
            validate_snapshot(*player_id, &self.start.map, snapshot)?;
        }
        Ok(())
    }

    fn record_observations(&mut self, tick: u32, snapshots: &BTreeMap<u32, Snapshot>) -> bool {
        if tick == 0 || tick % SAMPLE_EVERY_TICKS == 0 {
            for (player_id, snapshot) in snapshots {
                self.samples
                    .push(SnapshotSample::from_snapshot(tick, *player_id, snapshot));
            }
        }
        self.milestones.observe_snapshots(snapshots)
    }

    fn record_events(&mut self, tick: u32, tick_events: Vec<(u32, Vec<Event>)>) -> bool {
        let mut progressed = false;
        for (player_id, events) in tick_events {
            for event in events {
                let attacker_kind = match &event {
                    Event::Attack { from, .. } => self.attacker_kind(player_id, *from),
                    Event::Death { .. } | Event::Build { .. } | Event::Notice { .. } => None,
                };
                progressed |= self.milestones.observe_combat_event(
                    player_id,
                    attacker_kind.as_deref(),
                    &event,
                );
                self.event_log.push(EventLogEntry {
                    tick,
                    player_id,
                    event: event.clone(),
                });
                self.events.push(EventRecord {
                    tick,
                    player_id,
                    event,
                });
            }
        }
        progressed
    }

    fn attacker_kind(&self, player_id: u32, attacker: u32) -> Option<String> {
        self.game
            .snapshot_for(player_id)
            .entities
            .iter()
            .find(|e| e.id == attacker)
            .map(|e| e.kind.clone())
    }

    fn write_failure_artifact(&self, failure: &SelfPlayFailure) -> Result<PathBuf, String> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("selfplay-failures")
            .join(format!(
                "{}-{}-{}",
                self.test_name,
                std::process::id(),
                now_ms
            ));
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let artifact = FailureArtifact {
            test_name: self.test_name,
            failure: failure.reason.clone(),
            start: self.start.clone(),
            players: self.player_specs.clone(),
            milestones: self.milestones.clone(),
            commands: self.commands.clone(),
            replay_commands: self.game.command_log().to_vec(),
            events: self.events.clone(),
            replay_events: self.event_log.clone(),
            samples: self.samples.clone(),
        };
        let json = serde_json::to_vec_pretty(&artifact).map_err(|e| e.to_string())?;
        fs::write(dir.join("replay.json"), json).map_err(|e| e.to_string())?;
        fs::write(dir.join("summary.log"), artifact.summary_log()).map_err(|e| e.to_string())?;
        Ok(dir)
    }
}

#[derive(Debug)]
struct SelfPlayReport {
    ticks: u32,
    commands: usize,
    replay_commands: usize,
}

#[derive(Debug)]
pub(crate) struct SelfPlayFailure {
    reason: String,
}

impl SelfPlayFailure {
    pub(crate) fn new(reason: impl Into<String>) -> Self {
        SelfPlayFailure {
            reason: reason.into(),
        }
    }

    pub(crate) fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Clone, Serialize)]
struct PlayerSpec {
    id: u32,
    name: String,
    color: String,
}

impl From<&PlayerInit> for PlayerSpec {
    fn from(p: &PlayerInit) -> Self {
        PlayerSpec {
            id: p.id,
            name: p.name.clone(),
            color: p.color.clone(),
        }
    }
}

#[derive(Clone, Serialize)]
struct CommandRecord {
    tick: u32,
    player_id: u32,
    script: &'static str,
    command: Command,
}

#[derive(Clone, Serialize)]
struct EventRecord {
    tick: u32,
    player_id: u32,
    event: Event,
}

#[derive(Clone, Serialize)]
struct SnapshotSample {
    tick: u32,
    player_id: u32,
    steel: u32,
    oil: u32,
    supply_used: u32,
    supply_cap: u32,
    own_counts: BTreeMap<String, u32>,
    visible_entities: u32,
    damaged_own_entities: u32,
}

impl SnapshotSample {
    fn from_snapshot(tick: u32, player_id: u32, snapshot: &Snapshot) -> Self {
        let mut own_counts = BTreeMap::new();
        let mut damaged_own_entities = 0;
        for e in snapshot.entities.iter().filter(|e| e.owner == player_id) {
            *own_counts.entry(e.kind.clone()).or_insert(0) += 1;
            if e.hp < e.max_hp {
                damaged_own_entities += 1;
            }
        }

        SnapshotSample {
            tick,
            player_id,
            steel: snapshot.steel,
            oil: snapshot.oil,
            supply_used: snapshot.supply_used,
            supply_cap: snapshot.supply_cap,
            own_counts,
            visible_entities: snapshot.entities.len() as u32,
            damaged_own_entities,
        }
    }
}

#[derive(Clone, Serialize)]
struct Milestones {
    players: BTreeMap<u32, PlayerMilestones>,
    goals: BTreeMap<u32, PlayerMilestoneGoal>,
    combat_goal: CombatGoal,
    attack_events: u32,
    death_events: u32,
    attack_events_by_player: BTreeMap<u32, u32>,
    worker_attack_events_by_player: BTreeMap<u32, u32>,
    bunker_attack_events_by_player: BTreeMap<u32, u32>,
}

impl Milestones {
    fn tech_combat_for_players(ids: impl Iterator<Item = u32>) -> Self {
        Milestones::with_goals(
            ids.map(|id| (id, PlayerMilestoneGoal::tech_combat())),
            CombatGoal::any_combat(),
        )
    }

    fn with_goals(
        goals: impl IntoIterator<Item = (u32, PlayerMilestoneGoal)>,
        combat_goal: CombatGoal,
    ) -> Self {
        let goals: BTreeMap<u32, PlayerMilestoneGoal> = goals.into_iter().collect();
        Milestones {
            players: goals
                .keys()
                .copied()
                .map(|id| (id, PlayerMilestones::default()))
                .collect(),
            goals,
            combat_goal,
            attack_events: 0,
            death_events: 0,
            attack_events_by_player: BTreeMap::new(),
            worker_attack_events_by_player: BTreeMap::new(),
            bunker_attack_events_by_player: BTreeMap::new(),
        }
    }

    fn observe_snapshots(&mut self, snapshots: &BTreeMap<u32, Snapshot>) -> bool {
        let mut changed = false;
        for (player_id, snapshot) in snapshots {
            if let Some(player) = self.players.get_mut(player_id) {
                changed |= player.observe(*player_id, snapshot);
            }
        }
        changed
    }

    fn observe_combat_event(
        &mut self,
        player_id: u32,
        attacker_kind: Option<&str>,
        event: &Event,
    ) -> bool {
        match event {
            Event::Attack { .. } => {
                self.attack_events += 1;
                *self.attack_events_by_player.entry(player_id).or_default() += 1;
                match attacker_kind {
                    Some(kinds::WORKER) => {
                        *self
                            .worker_attack_events_by_player
                            .entry(player_id)
                            .or_default() += 1;
                    }
                    Some(kinds::BUNKER) => {
                        *self
                            .bunker_attack_events_by_player
                            .entry(player_id)
                            .or_default() += 1;
                    }
                    _ => {}
                }
                true
            }
            Event::Death { .. } => {
                self.death_events += 1;
                true
            }
            Event::Build { .. } | Event::Notice { .. } => false,
        }
    }

    fn complete(&self) -> bool {
        let players_complete = self
            .goals
            .iter()
            .all(|(player_id, goal)| self.players[player_id].complete_for(goal));
        players_complete && self.combat_goal.complete(self)
    }

    fn missing_summary(&self) -> String {
        let mut missing = Vec::new();
        for (player_id, goal) in &self.goals {
            if let Some(player) = self.players.get(player_id) {
                for item in player.missing_for(goal) {
                    missing.push(format!("p{player_id}:{item}"));
                }
            }
        }
        for item in self.combat_goal.missing(self) {
            missing.push(item);
        }
        missing.join(", ")
    }
}

#[derive(Clone, Default, Serialize)]
struct CombatGoal {
    require_any_combat: bool,
    min_attacks_by_player: BTreeMap<u32, u32>,
    min_worker_attacks_by_player: BTreeMap<u32, u32>,
    min_bunker_attacks_by_player: BTreeMap<u32, u32>,
}

impl CombatGoal {
    fn any_combat() -> Self {
        CombatGoal {
            require_any_combat: true,
            ..CombatGoal::default()
        }
    }

    fn worker_attack_by(player_id: u32) -> Self {
        CombatGoal {
            min_worker_attacks_by_player: BTreeMap::from([(player_id, 1)]),
            ..CombatGoal::default()
        }
    }

    fn bunker_attack_by(player_id: u32) -> Self {
        CombatGoal {
            min_bunker_attacks_by_player: BTreeMap::from([(player_id, 1)]),
            ..CombatGoal::default()
        }
    }

    fn complete(&self, milestones: &Milestones) -> bool {
        if self.require_any_combat && milestones.attack_events == 0 && milestones.death_events == 0
        {
            return false;
        }
        for (player_id, required) in &self.min_attacks_by_player {
            if milestones
                .attack_events_by_player
                .get(player_id)
                .copied()
                .unwrap_or(0)
                < *required
            {
                return false;
            }
        }
        for (player_id, required) in &self.min_worker_attacks_by_player {
            if milestones
                .worker_attack_events_by_player
                .get(player_id)
                .copied()
                .unwrap_or(0)
                < *required
            {
                return false;
            }
        }
        for (player_id, required) in &self.min_bunker_attacks_by_player {
            if milestones
                .bunker_attack_events_by_player
                .get(player_id)
                .copied()
                .unwrap_or(0)
                < *required
            {
                return false;
            }
        }
        true
    }

    fn missing(&self, milestones: &Milestones) -> Vec<String> {
        let mut out = Vec::new();
        if self.require_any_combat && milestones.attack_events == 0 && milestones.death_events == 0
        {
            out.push("combat-event".to_string());
        }
        for (player_id, required) in &self.min_attacks_by_player {
            let seen = milestones
                .attack_events_by_player
                .get(player_id)
                .copied()
                .unwrap_or(0);
            if seen < *required {
                out.push(format!("p{player_id}:attack-events>={required}"));
            }
        }
        for (player_id, required) in &self.min_worker_attacks_by_player {
            let seen = milestones
                .worker_attack_events_by_player
                .get(player_id)
                .copied()
                .unwrap_or(0);
            if seen < *required {
                out.push(format!("p{player_id}:worker-attacks>={required}"));
            }
        }
        for (player_id, required) in &self.min_bunker_attacks_by_player {
            let seen = milestones
                .bunker_attack_events_by_player
                .get(player_id)
                .copied()
                .unwrap_or(0);
            if seen < *required {
                out.push(format!("p{player_id}:bunker-attacks>={required}"));
            }
        }
        out
    }
}

#[derive(Clone, Default, Serialize)]
struct PlayerMilestoneGoal {
    require_gathering: bool,
    require_oil: bool,
    require_depot_supply: bool,
    require_barracks_complete: bool,
    require_rifleman: bool,
    require_tank: bool,
    require_bunker_complete: bool,
    require_damage_taken: bool,
    min_workers: u32,
    min_bunkers: u32,
}

impl PlayerMilestoneGoal {
    fn tech_combat() -> Self {
        PlayerMilestoneGoal {
            require_gathering: true,
            require_oil: true,
            require_depot_supply: true,
            require_barracks_complete: true,
            require_rifleman: true,
            require_tank: true,
            ..PlayerMilestoneGoal::default()
        }
    }

    fn bunker_rush() -> Self {
        PlayerMilestoneGoal {
            require_gathering: true,
            require_bunker_complete: true,
            min_bunkers: 1,
            ..PlayerMilestoneGoal::default()
        }
    }

    fn damaged_economy() -> Self {
        PlayerMilestoneGoal {
            require_gathering: true,
            require_damage_taken: true,
            min_workers: config::STARTING_WORKERS + 2,
            ..PlayerMilestoneGoal::default()
        }
    }
}

#[derive(Clone, Default, PartialEq, Serialize)]
struct PlayerMilestones {
    saw_gathering: bool,
    oil_gathered: bool,
    depot_started: bool,
    barracks_started: bool,
    barracks_complete: bool,
    bunker_started: bool,
    bunker_complete: bool,
    rifleman_trained: bool,
    tank_trained: bool,
    damage_taken: bool,
    max_workers: u32,
    max_steel: u32,
    max_oil: u32,
    max_supply_cap: u32,
    max_riflemen: u32,
    max_tanks: u32,
    max_bunkers: u32,
}

impl PlayerMilestones {
    fn observe(&mut self, player_id: u32, snapshot: &Snapshot) -> bool {
        let before = self.clone();
        let mut workers = 0;
        let mut riflemen = 0;
        let mut tanks = 0;
        let mut bunkers = 0;
        for e in snapshot.entities.iter().filter(|e| e.owner == player_id) {
            let Some(k) = kind_of(e) else { continue };
            match k {
                EntityKind::Worker => {
                    workers += 1;
                    if e.state == states::GATHER || e.latched_node.is_some() {
                        self.saw_gathering = true;
                    }
                }
                EntityKind::Rifleman => riflemen += 1,
                EntityKind::Tank => tanks += 1,
                EntityKind::Depot => self.depot_started = true,
                EntityKind::Barracks => {
                    self.barracks_started = true;
                    if is_complete(e) {
                        self.barracks_complete = true;
                    }
                }
                EntityKind::Bunker => {
                    self.bunker_started = true;
                    if is_complete(e) {
                        bunkers += 1;
                        self.bunker_complete = true;
                    }
                }
                _ => {}
            }
            if e.hp < e.max_hp {
                self.damage_taken = true;
            }
        }
        self.oil_gathered |= snapshot.oil > 0;
        self.max_workers = self.max_workers.max(workers);
        self.max_steel = self.max_steel.max(snapshot.steel);
        self.max_oil = self.max_oil.max(snapshot.oil);
        self.max_supply_cap = self.max_supply_cap.max(snapshot.supply_cap);
        self.max_riflemen = self.max_riflemen.max(riflemen);
        self.max_tanks = self.max_tanks.max(tanks);
        self.max_bunkers = self.max_bunkers.max(bunkers);
        self.rifleman_trained |= riflemen > 0;
        self.tank_trained |= tanks > 0;
        before != *self
    }

    fn complete_for(&self, goal: &PlayerMilestoneGoal) -> bool {
        self.missing_for(goal).is_empty()
    }

    fn missing_for(&self, goal: &PlayerMilestoneGoal) -> Vec<String> {
        let mut out = Vec::new();
        if goal.require_gathering && !self.saw_gathering {
            out.push("economy-gather".to_string());
        }
        if goal.require_oil && !self.oil_gathered {
            out.push("oil-gather".to_string());
        }
        if goal.require_depot_supply
            && (!self.depot_started || self.max_supply_cap <= config::INDUSTRIAL_CENTER_SUPPLY)
        {
            out.push("depot-supply".to_string());
        }
        if goal.require_barracks_complete && !self.barracks_complete {
            out.push("barracks".to_string());
        }
        if goal.require_rifleman && !self.rifleman_trained {
            out.push("rifleman".to_string());
        }
        if goal.require_tank && !self.tank_trained {
            out.push("tank".to_string());
        }
        if goal.require_bunker_complete && !self.bunker_complete {
            out.push("bunker".to_string());
        }
        if goal.require_damage_taken && !self.damage_taken {
            out.push("damage-taken".to_string());
        }
        if self.max_workers < goal.min_workers {
            out.push(format!("workers>={}", goal.min_workers));
        }
        if self.max_bunkers < goal.min_bunkers {
            out.push(format!("bunkers>={}", goal.min_bunkers));
        }
        out
    }
}

#[derive(Serialize)]
struct FailureArtifact {
    test_name: &'static str,
    failure: String,
    start: StartPayload,
    players: Vec<PlayerSpec>,
    milestones: Milestones,
    commands: Vec<CommandRecord>,
    replay_commands: Vec<super::replay::CommandLogEntry>,
    events: Vec<EventRecord>,
    replay_events: Vec<EventLogEntry>,
    samples: Vec<SnapshotSample>,
}

impl FailureArtifact {
    fn summary_log(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("test: {}\n", self.test_name));
        out.push_str(&format!("failure: {}\n", self.failure));
        out.push_str(&format!("commands: {}\n", self.commands.len()));
        out.push_str(&format!(
            "replay commands: {}\n",
            self.replay_commands.len()
        ));
        out.push_str(&format!("events: {}\n", self.events.len()));
        out.push_str(&format!("missing: {}\n", self.milestones.missing_summary()));
        if let Some(last) = self.samples.last() {
            out.push_str(&format!("last sample tick: {}\n", last.tick));
        }
        out
    }
}

fn replay_outcome_for(
    game: &Game,
    players: &[PlayerInit],
) -> Result<ReplayOutcome, SelfPlayFailure> {
    replay_commands(players, game.command_log(), game.tick_count())
        .map_err(|e| SelfPlayFailure::new(format!("replay failed: {e}")))
}

fn live_outcome_for(
    game: &Game,
    players: &[PlayerInit],
    events: &[EventLogEntry],
) -> ReplayOutcome {
    ReplayOutcome {
        ticks: game.tick_count(),
        events: events.to_vec(),
        final_snapshots: players
            .iter()
            .map(|p| PlayerSnapshot {
                player_id: p.id,
                snapshot: game.snapshot_for(p.id),
            })
            .collect(),
    }
}

pub(crate) fn assert_replay_matches_live(
    game: &Game,
    players: &[PlayerInit],
    events: &[EventLogEntry],
) -> Result<(), SelfPlayFailure> {
    let live = live_outcome_for(game, players, events);
    let replay = replay_outcome_for(game, players)?;
    if replay != live {
        return Err(SelfPlayFailure::new(format!(
            "deterministic replay diverged from the live command-log run: {}",
            first_replay_diff(&live, &replay)
        )));
    }
    Ok(())
}

fn first_replay_diff(live: &ReplayOutcome, replay: &ReplayOutcome) -> String {
    if live.ticks != replay.ticks {
        return format!("ticks live={} replay={}", live.ticks, replay.ticks);
    }
    if live.events != replay.events {
        for (idx, (live_event, replay_event)) in live.events.iter().zip(&replay.events).enumerate()
        {
            if live_event != replay_event {
                return format!("event {idx} differs: live={live_event:?} replay={replay_event:?}");
            }
        }
        return format!(
            "events live={} replay={}",
            live.events.len(),
            replay.events.len()
        );
    }
    for (live_view, replay_view) in live.final_snapshots.iter().zip(&replay.final_snapshots) {
        if live_view.player_id != replay_view.player_id {
            return format!(
                "snapshot player order live={} replay={}",
                live_view.player_id, replay_view.player_id
            );
        }
        if live_view.snapshot != replay_view.snapshot {
            return format!(
                "snapshot mismatch for player {}: live_entities={} replay_entities={} live_resources={}/{} replay_resources={}/{} live_supply={}/{} replay_supply={}/{}",
                live_view.player_id,
                live_view.snapshot.entities.len(),
                replay_view.snapshot.entities.len(),
                live_view.snapshot.steel,
                live_view.snapshot.oil,
                replay_view.snapshot.steel,
                replay_view.snapshot.oil,
                live_view.snapshot.supply_used,
                live_view.snapshot.supply_cap,
                replay_view.snapshot.supply_used,
                replay_view.snapshot.supply_cap
            );
        }
    }
    format!(
        "snapshot counts live={} replay={}",
        live.final_snapshots.len(),
        replay.final_snapshots.len()
    )
}

fn validate_snapshot(
    player_id: u32,
    map: &MapInfo,
    snapshot: &Snapshot,
) -> Result<(), SelfPlayFailure> {
    if snapshot.supply_used > snapshot.supply_cap {
        return Err(SelfPlayFailure::new(format!(
            "player {player_id} has invalid supply {}/{}",
            snapshot.supply_used, snapshot.supply_cap
        )));
    }
    if snapshot.supply_cap > config::SUPPLY_CAP_MAX {
        return Err(SelfPlayFailure::new(format!(
            "player {player_id} exceeded supply cap max: {}",
            snapshot.supply_cap
        )));
    }
    if snapshot.steel > RESOURCE_SANITY_LIMIT || snapshot.oil > RESOURCE_SANITY_LIMIT {
        return Err(SelfPlayFailure::new(format!(
            "player {player_id} resources look invalid: steel={} oil={}",
            snapshot.steel, snapshot.oil
        )));
    }

    let mut ids = HashSet::new();
    let world = map.width as f32 * map.tile_size as f32;
    for entity in &snapshot.entities {
        if !ids.insert(entity.id) {
            return Err(SelfPlayFailure::new(format!(
                "player {player_id} snapshot has duplicate entity id {}",
                entity.id
            )));
        }
        if !known_kind(&entity.kind) {
            return Err(SelfPlayFailure::new(format!(
                "player {player_id} saw unknown entity kind {}",
                entity.kind
            )));
        }
        if entity.hp > entity.max_hp {
            return Err(SelfPlayFailure::new(format!(
                "player {player_id} saw entity {} with hp {}/{}",
                entity.id, entity.hp, entity.max_hp
            )));
        }
        if !entity.x.is_finite()
            || !entity.y.is_finite()
            || entity.x < 0.0
            || entity.y < 0.0
            || entity.x >= world
            || entity.y >= world
        {
            return Err(SelfPlayFailure::new(format!(
                "player {player_id} saw entity {} out of bounds at {},{}",
                entity.id, entity.x, entity.y
            )));
        }
        if let Some(progress) = entity.prod_progress {
            if !(0.0..=1.0).contains(&progress) || !progress.is_finite() {
                return Err(SelfPlayFailure::new(format!(
                    "player {player_id} saw invalid production progress {progress}"
                )));
            }
        }
        if let Some(progress) = entity.build_progress {
            if !(0.0..=1.0).contains(&progress) || !progress.is_finite() {
                return Err(SelfPlayFailure::new(format!(
                    "player {player_id} saw invalid build progress {progress}"
                )));
            }
        }
    }

    Ok(())
}

fn known_kind(kind: &str) -> bool {
    matches!(
        kind,
        kinds::WORKER
            | kinds::RIFLEMAN
            | kinds::MACHINE_GUNNER
            | kinds::AT_TEAM
            | kinds::TANK
            | kinds::INDUSTRIAL_CENTER
            | kinds::DEPOT
            | kinds::BARRACKS
            | kinds::TRAINING_CENTRE
            | kinds::TANK_FACTORY
            | kinds::BUNKER
            | kinds::STEEL
            | kinds::OIL
    )
}

fn is_complete(entity: &EntityView) -> bool {
    entity.build_progress.is_none()
}

fn production_queue_len(entity: &EntityView) -> u32 {
    entity.prod_queue.unwrap_or(0)
}

fn build_near_own_start_if_affordable(
    view: PlayerView<'_>,
    building: EntityKind,
    steel: &mut u32,
    builder_workers: &[u32],
    reserved_workers: &mut HashSet<u32>,
) -> Option<Command> {
    let stats = config::building_stats(building)?;
    if *steel < stats.cost_steel {
        return None;
    }
    let worker = builder_workers
        .iter()
        .copied()
        .find(|id| !reserved_workers.contains(id))?;
    let start = own_start_tile(view.start, view.player_id)?;
    let empty = BTreeSet::new();
    let (tile_x, tile_y) =
        find_build_spot(&view.start.map, view.snapshot, start, building, &empty)?;
    reserved_workers.insert(worker);
    *steel -= stats.cost_steel;
    Some(Command::Build {
        worker,
        building: building.to_protocol_str().to_string(),
        tile_x,
        tile_y,
    })
}

fn assign_steel_workers(
    view: PlayerView<'_>,
    workers: &[&EntityView],
    reserved_workers: &HashSet<u32>,
    initial_gather_sent: bool,
    out: &mut Vec<Command>,
) {
    let steel_nodes: Vec<&EntityView> = view
        .snapshot
        .entities
        .iter()
        .filter(|e| e.owner == 0 && is_kind(e, EntityKind::Steel) && e.remaining.unwrap_or(0) > 0)
        .collect();
    if steel_nodes.is_empty() {
        return;
    }

    let mut assigned_nodes: HashSet<u32> = HashSet::new();
    for w in workers {
        if let Some(node) = w.latched_node {
            assigned_nodes.insert(node);
        }
    }

    let mut sorted_workers = workers.to_vec();
    sorted_workers.sort_by_key(|w| w.id);
    for worker in sorted_workers {
        if reserved_workers.contains(&worker.id) || worker.state == states::BUILD {
            continue;
        }
        if initial_gather_sent && worker.state != states::IDLE {
            continue;
        }
        if worker.latched_node.is_some() {
            continue;
        }
        if let Some(node) = nearest_unassigned_node(worker, &steel_nodes, &assigned_nodes) {
            out.push(Command::Gather {
                units: vec![worker.id],
                node,
            });
            assigned_nodes.insert(node);
        }
    }
}

fn own_start_tile(start: &StartPayload, player_id: u32) -> Option<(u32, u32)> {
    start
        .players
        .iter()
        .find(|p| p.id == player_id)
        .map(|p| (p.start_tile_x, p.start_tile_y))
}

fn player_start_world(start: &StartPayload, player_id: u32) -> Option<(f32, f32)> {
    let (tile_x, tile_y) = own_start_tile(start, player_id)?;
    let ts = start.map.tile_size as f32;
    Some((tile_x as f32 * ts + ts * 0.5, tile_y as f32 * ts + ts * 0.5))
}

fn combat_rendezvous_world(view: PlayerView<'_>) -> (f32, f32) {
    let ts = view.start.map.tile_size as f32;
    (
        view.start.map.width as f32 * ts * 0.5,
        view.start.map.height as f32 * ts * 0.5,
    )
}

fn nearest_unassigned_node(
    worker: &EntityView,
    nodes: &[&EntityView],
    assigned: &HashSet<u32>,
) -> Option<u32> {
    let mut best = None;
    for node in nodes {
        if assigned.contains(&node.id) {
            continue;
        }
        let d = dist2(worker.x, worker.y, node.x, node.y);
        if best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((node.id, d));
        }
    }
    best.map(|(id, _)| id)
}

fn dist2(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

fn find_offensive_bunker_spot(
    start: &StartPayload,
    snapshot: &Snapshot,
    target_player_id: u32,
) -> Option<(u32, u32)> {
    let occupied = occupied_tiles_from_snapshot(&start.map, snapshot);
    let (target_x, target_y) = own_start_tile(start, target_player_id)?;
    let center_x = start.map.width as f32 * 0.5;
    let center_y = start.map.height as f32 * 0.5;
    let away_x = sign_step(target_x as f32 - center_x);
    let away_y = sign_step(target_y as f32 - center_y);
    let target_x = target_x as i32;
    let target_y = target_y as i32;

    let preferred_offsets = [
        (away_x * 7, -away_y),
        (-away_x, away_y * 7),
        (away_x * 7, 0),
        (0, away_y * 7),
        (away_x * 6, -away_y * 2),
        (-away_x * 2, away_y * 6),
    ];
    for (dx, dy) in preferred_offsets {
        if let Some(spot) =
            offensive_build_spot_if_placeable(start, &occupied, target_x + dx, target_y + dy)
        {
            return Some(spot);
        }
    }

    let mut best: Option<(u32, u32, i32, i32)> = None;
    for radius in 4i32..=7 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let dist2_tiles = dx * dx + dy * dy;
                if !(16..=49).contains(&dist2_tiles) {
                    continue;
                }
                let away_score = dx * away_x + dy * away_y;
                if away_score <= 0 {
                    continue;
                }
                let tx = target_x + dx;
                let ty = target_y + dy;
                let Some((tx, ty)) = offensive_build_spot_if_placeable(start, &occupied, tx, ty)
                else {
                    continue;
                };
                let better = best
                    .map(|(_, _, best_score, best_dist)| {
                        away_score > best_score
                            || (away_score == best_score && dist2_tiles < best_dist)
                    })
                    .unwrap_or(true);
                if better {
                    best = Some((tx, ty, away_score, dist2_tiles));
                }
            }
        }
        if let Some((tx, ty, _, _)) = best {
            return Some((tx, ty));
        }
    }
    None
}

fn offensive_build_spot_if_placeable(
    start: &StartPayload,
    occupied: &BTreeSet<(u32, u32)>,
    tile_x: i32,
    tile_y: i32,
) -> Option<(u32, u32)> {
    if tile_x < 0 || tile_y < 0 {
        return None;
    }
    let (tile_x, tile_y) = (tile_x as u32, tile_y as u32);
    footprint_placeable_from_snapshot(&start.map, EntityKind::Bunker, tile_x, tile_y, occupied)
        .then_some((tile_x, tile_y))
}

fn sign_step(value: f32) -> i32 {
    if value < 0.0 {
        -1
    } else if value > 0.0 {
        1
    } else {
        0
    }
}

fn find_build_spot(
    map: &MapInfo,
    snapshot: &Snapshot,
    start: (u32, u32),
    building: EntityKind,
    skip: &BTreeSet<(u32, u32)>,
) -> Option<(u32, u32)> {
    let stats = config::building_stats(building)?;
    let occupied = occupied_tiles_from_snapshot(map, snapshot);

    let map_center = (map.width as f32 * 0.5, map.height as f32 * 0.5);
    let away = (start.0 as f32 - map_center.0, start.1 as f32 - map_center.1);
    let (sx, sy) = (start.0 as i32, start.1 as i32);
    let mut fallback = None;
    for radius in 3i32..=16 {
        let mut best_in_ring: Option<(u32, u32, f32, f32)> = None;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let tx = sx + dx;
                let ty = sy + dy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let (tx, ty) = (tx as u32, ty as u32);
                if skip.contains(&(tx, ty)) {
                    continue;
                }
                if footprint_placeable_from_snapshot(map, building, tx, ty, &occupied) {
                    let center_x = tx as f32 + stats.foot_w as f32 * 0.5;
                    let center_y = ty as f32 + stats.foot_h as f32 * 0.5;
                    let from_start = (center_x - start.0 as f32, center_y - start.1 as f32);
                    let away_score = from_start.0 * away.0 + from_start.1 * away.1;
                    let dist = from_start.0 * from_start.0 + from_start.1 * from_start.1;
                    if fallback.is_none() {
                        fallback = Some((tx, ty));
                    }
                    let better = best_in_ring
                        .map(|(_, _, best_score, best_dist)| {
                            away_score > best_score
                                || (away_score == best_score && dist < best_dist)
                        })
                        .unwrap_or(true);
                    if better {
                        best_in_ring = Some((tx, ty, away_score, dist));
                    }
                }
            }
        }
        if let Some((tx, ty, away_score, _)) = best_in_ring {
            if away_score >= 0.0 {
                return Some((tx, ty));
            }
        }
    }
    fallback
}

fn occupied_tiles_from_snapshot(map: &MapInfo, snapshot: &Snapshot) -> BTreeSet<(u32, u32)> {
    let mut occupied = BTreeSet::new();
    for e in &snapshot.entities {
        if e.owner != 0 && kind_of(e).map(|k| k.is_building()).unwrap_or(false) {
            for tile in building_footprint_tiles(map, e) {
                occupied.insert(tile);
            }
        } else if e.owner == 0 && (is_kind(e, EntityKind::Steel) || is_kind(e, EntityKind::Oil)) {
            occupied.insert(tile_of(map, e.x, e.y));
        }
    }
    occupied
}

fn footprint_placeable_from_snapshot(
    map: &MapInfo,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
    occupied: &BTreeSet<(u32, u32)>,
) -> bool {
    let Some(stats) = config::building_stats(building) else {
        return false;
    };
    for dy in 0..stats.foot_h {
        for dx in 0..stats.foot_w {
            let Some(tx) = tile_x.checked_add(dx) else {
                return false;
            };
            let Some(ty) = tile_y.checked_add(dy) else {
                return false;
            };
            if tx >= map.width || ty >= map.height {
                return false;
            }
            let idx = (ty * map.width + tx) as usize;
            if map.terrain.get(idx).copied() != Some(terrain::GRASS) {
                return false;
            }
            if occupied.contains(&(tx, ty)) {
                return false;
            }
        }
    }
    if !config::trainable_units(building).is_empty() {
        let spawn_x = tile_x + stats.foot_w / 2;
        let Some(spawn_y) = tile_y.checked_add(stats.foot_h) else {
            return false;
        };
        if spawn_x >= map.width || spawn_y >= map.height {
            return false;
        }
        let spawn_idx = (spawn_y * map.width + spawn_x) as usize;
        if map.terrain.get(spawn_idx).copied() != Some(terrain::GRASS) {
            return false;
        }
        if occupied.contains(&(spawn_x, spawn_y)) {
            return false;
        }
    }
    true
}

fn building_footprint_tiles(map: &MapInfo, entity: &EntityView) -> Vec<(u32, u32)> {
    let kind = match kind_of(entity) {
        Some(k) => k,
        None => return Vec::new(),
    };
    let Some(stats) = config::building_stats(kind) else {
        return Vec::new();
    };
    let (cx, cy) = tile_of(map, entity.x, entity.y);
    let ox = stats.foot_w as i32 / 2;
    let oy = stats.foot_h as i32 / 2;
    let mut out = Vec::new();
    for dy in 0..stats.foot_h as i32 {
        for dx in 0..stats.foot_w as i32 {
            let tx = cx as i32 + dx - ox;
            let ty = cy as i32 + dy - oy;
            if tx >= 0 && ty >= 0 {
                out.push((tx as u32, ty as u32));
            }
        }
    }
    out
}

fn tile_of(map: &MapInfo, x: f32, y: f32) -> (u32, u32) {
    let ts = map.tile_size as f32;
    let tx = (x / ts).floor().max(0.0) as u32;
    let ty = (y / ts).floor().max(0.0) as u32;
    (tx.min(map.width - 1), ty.min(map.height - 1))
}

#[test]
fn scripted_self_play_exercises_economy_tech_and_combat() {
    let players = vec![
        PlayerInit {
            id: 1,
            name: "Script Alpha".into(),
            color: "#4cc9f0".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            name: "Script Beta".into(),
            color: "#f72585".into(),
            is_ai: false,
        },
    ];
    let game = Game::new(&players);
    let start = game.start_payload();
    let specs: Vec<PlayerSpec> = players.iter().map(PlayerSpec::from).collect();
    let scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(BuildTechAttackScript::new(1)),
        Box::new(BuildTechAttackScript::new(2)),
    ];
    let mut runner = SelfPlayRunner::new(
        "scripted_self_play_exercises_economy_tech_and_combat",
        game,
        start,
        specs,
        scripts,
    );

    match runner.run() {
        Ok(report) => {
            assert!(report.ticks > 0);
            assert!(report.commands > 0);
            assert_eq!(report.commands, report.replay_commands);
            assert_replay_matches_live(&runner.game, &players, &runner.event_log).unwrap_or_else(
                |failure| {
                    let artifact = runner
                        .write_failure_artifact(&failure)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|e| format!("artifact write failed: {e}"));
                    panic!(
                        "self-play replay failed: {}; artifact: {artifact}",
                        failure.reason
                    );
                },
            );
        }
        Err(failure) => {
            let artifact = runner
                .write_failure_artifact(&failure)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|e| format!("artifact write failed: {e}"));
            panic!("self-play failed: {}; artifact: {artifact}", failure.reason);
        }
    }
}

#[test]
fn scripted_self_play_bunker_rush_vs_economy() {
    let players = vec![
        PlayerInit {
            id: 1,
            name: "Bunker Rush".into(),
            color: "#ff9f1c".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            name: "Economy".into(),
            color: "#2ec4b6".into(),
            is_ai: false,
        },
    ];
    let game = Game::new(&players);
    let start = game.start_payload();
    let specs: Vec<PlayerSpec> = players.iter().map(PlayerSpec::from).collect();
    let scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(BunkerRushScript::new(1, 2)),
        Box::new(EconomyScript::new(2)),
    ];
    let milestones = Milestones::with_goals(
        [
            (1, PlayerMilestoneGoal::bunker_rush()),
            (2, PlayerMilestoneGoal::damaged_economy()),
        ],
        CombatGoal::bunker_attack_by(1),
    );
    let mut runner = SelfPlayRunner::with_milestones(
        "scripted_self_play_bunker_rush_vs_economy",
        game,
        start,
        specs,
        scripts,
        milestones,
    );

    match runner.run() {
        Ok(report) => {
            assert!(report.ticks > 0);
            assert!(report.commands > 0);
            assert_eq!(report.commands, report.replay_commands);
            assert_replay_matches_live(&runner.game, &players, &runner.event_log).unwrap_or_else(
                |failure| {
                    let artifact = runner
                        .write_failure_artifact(&failure)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|e| format!("artifact write failed: {e}"));
                    panic!(
                        "self-play replay failed: {}; artifact: {artifact}",
                        failure.reason
                    );
                },
            );
        }
        Err(failure) => {
            let artifact = runner
                .write_failure_artifact(&failure)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|e| format!("artifact write failed: {e}"));
            panic!("self-play failed: {}; artifact: {artifact}", failure.reason);
        }
    }
}

#[test]
fn scripted_self_play_worker_rush_vs_economy() {
    let players = vec![
        PlayerInit {
            id: 1,
            name: "Worker Rush".into(),
            color: "#e71d36".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            name: "Economy".into(),
            color: "#3a86ff".into(),
            is_ai: false,
        },
    ];
    let game = Game::new(&players);
    let start = game.start_payload();
    let specs: Vec<PlayerSpec> = players.iter().map(PlayerSpec::from).collect();
    let scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(WorkerRushScript::new(1, 2)),
        Box::new(EconomyScript::new(2)),
    ];
    let milestones = Milestones::with_goals(
        [
            (1, PlayerMilestoneGoal::default()),
            (2, PlayerMilestoneGoal::damaged_economy()),
        ],
        CombatGoal::worker_attack_by(1),
    );
    let mut runner = SelfPlayRunner::with_milestones(
        "scripted_self_play_worker_rush_vs_economy",
        game,
        start,
        specs,
        scripts,
        milestones,
    );

    match runner.run() {
        Ok(report) => {
            assert!(report.ticks > 0);
            assert!(report.commands > 0);
            assert_eq!(report.commands, report.replay_commands);
            assert_replay_matches_live(&runner.game, &players, &runner.event_log).unwrap_or_else(
                |failure| {
                    let artifact = runner
                        .write_failure_artifact(&failure)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|e| format!("artifact write failed: {e}"));
                    panic!(
                        "self-play replay failed: {}; artifact: {artifact}",
                        failure.reason
                    );
                },
            );
        }
        Err(failure) => {
            let artifact = runner
                .write_failure_artifact(&failure)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|e| format!("artifact write failed: {e}"));
            panic!("self-play failed: {}; artifact: {artifact}", failure.reason);
        }
    }
}

/// A scripted player that does nothing but send idle workers to mine the nearest steel node.
/// No building, no training, no combat — pure passive mining.
struct MineOnlyScript {
    player_id: u32,
    initial_gather_sent: bool,
}

impl MineOnlyScript {
    fn new(player_id: u32) -> Self {
        MineOnlyScript {
            player_id,
            initial_gather_sent: false,
        }
    }

    fn should_think(&self, tick: u32) -> bool {
        tick == 0 || tick.wrapping_add(self.player_id) % THINK_INTERVAL == 0
    }
}

impl ScriptedPlayer for MineOnlyScript {
    fn player_id(&self) -> u32 {
        self.player_id
    }

    fn name(&self) -> &'static str {
        "mine-only"
    }

    fn commands(&mut self, view: PlayerView<'_>) -> Vec<Command> {
        if !self.should_think(view.tick) {
            return Vec::new();
        }

        let workers: Vec<&EntityView> = view
            .snapshot
            .entities
            .iter()
            .filter(|e| e.owner == view.player_id && is_kind(e, EntityKind::Worker))
            .collect();

        let mut out = Vec::new();
        assign_steel_workers(
            view,
            &workers,
            &HashSet::new(),
            self.initial_gather_sent,
            &mut out,
        );
        self.initial_gather_sent = true;
        out
    }
}

/// Two players mine steel passively for two minutes. With attached mining the steady state
/// has no pathfinding variance, so both players should end with nearly identical steel totals.
#[test]
fn scripted_self_play_mine_only_steel_fairness() {
    const TWO_MINUTES_TICKS: u32 = 2 * 60 * config::TICK_HZ;
    const STEEL_TOLERANCE: u32 = 15;

    let players = vec![
        PlayerInit {
            id: 1,
            name: "Miner A".into(),
            color: "#4cc9f0".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            name: "Miner B".into(),
            color: "#f72585".into(),
            is_ai: false,
        },
    ];
    let mut game = Game::new(&players);
    let start = game.start_payload();

    let mut scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(MineOnlyScript::new(1)),
        Box::new(MineOnlyScript::new(2)),
    ];

    for tick in 0..TWO_MINUTES_TICKS {
        let snapshots: BTreeMap<u32, Snapshot> = players
            .iter()
            .map(|p| (p.id, game.snapshot_for(p.id)))
            .collect();

        let mut commands = Vec::new();
        for script in &mut scripts {
            let pid = script.player_id();
            let Some(snapshot) = snapshots.get(&pid) else {
                continue;
            };
            let view = PlayerView {
                player_id: pid,
                tick,
                start: &start,
                snapshot,
            };
            for command in script.commands(view) {
                commands.push((pid, command));
            }
        }

        for (player_id, command) in commands {
            game.enqueue(player_id, command);
        }

        game.tick();
    }

    let snap_a = game.snapshot_for(1);
    let snap_b = game.snapshot_for(2);

    let diff = snap_a.steel.abs_diff(snap_b.steel);

    assert!(
        diff <= STEEL_TOLERANCE,
        "after two minutes of passive mining, player 1 has {} steel and player 2 has {} steel (diff = {}, tolerance = {})",
        snap_a.steel,
        snap_b.steel,
        diff,
        STEEL_TOLERANCE
    );
}

/// 30-second smoke test for attached-mining fairness. Should be perfectly deterministic.
#[test]
fn mine_only_steel_fairness_30s() {
    const THIRTY_SECONDS_TICKS: u32 = 30 * config::TICK_HZ;
    const STEEL_TOLERANCE: u32 = 2;

    let players = vec![
        PlayerInit {
            id: 1,
            name: "Miner A".into(),
            color: "#4cc9f0".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            name: "Miner B".into(),
            color: "#f72585".into(),
            is_ai: false,
        },
    ];
    let mut game = Game::new(&players);
    let start = game.start_payload();

    let mut scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(MineOnlyScript::new(1)),
        Box::new(MineOnlyScript::new(2)),
    ];

    for tick in 0..THIRTY_SECONDS_TICKS {
        let snapshots: BTreeMap<u32, Snapshot> = players
            .iter()
            .map(|p| (p.id, game.snapshot_for(p.id)))
            .collect();

        let mut commands = Vec::new();
        for script in &mut scripts {
            let pid = script.player_id();
            let Some(snapshot) = snapshots.get(&pid) else {
                continue;
            };
            let view = PlayerView {
                player_id: pid,
                tick,
                start: &start,
                snapshot,
            };
            for command in script.commands(view) {
                commands.push((pid, command));
            }
        }

        for (player_id, command) in commands {
            game.enqueue(player_id, command);
        }

        game.tick();
    }

    let snap_a = game.snapshot_for(1);
    let snap_b = game.snapshot_for(2);

    let diff = snap_a.steel.abs_diff(snap_b.steel);

    assert!(
        diff <= STEEL_TOLERANCE,
        "after 30 seconds of passive mining, player 1 has {} steel and player 2 has {} steel (diff = {}, tolerance = {})",
        snap_a.steel,
        snap_b.steel,
        diff,
        STEEL_TOLERANCE
    );
}

/// Run a scripted match for a fixed number of ticks and return the final game state plus
/// the per-tick snapshots for every player.
#[cfg(test)]
fn run_scripted_ticks(
    players: &[PlayerInit],
    scripts: &mut [Box<dyn ScriptedPlayer>],
    start: &StartPayload,
    game: &mut Game,
    ticks: u32,
) -> Vec<BTreeMap<u32, Snapshot>> {
    let mut history = Vec::with_capacity(ticks as usize);
    for tick in 0..ticks {
        let snapshots: BTreeMap<u32, Snapshot> = players
            .iter()
            .map(|p| (p.id, game.snapshot_for(p.id)))
            .collect();
        history.push(snapshots.clone());

        let mut commands = Vec::new();
        for script in scripts.iter_mut() {
            let pid = script.player_id();
            let Some(snapshot) = snapshots.get(&pid) else {
                continue;
            };
            let view = PlayerView {
                player_id: pid,
                tick,
                start,
                snapshot,
            };
            for command in script.commands(view) {
                commands.push((pid, command));
            }
        }

        for (player_id, command) in commands {
            game.enqueue(player_id, command);
        }

        game.tick();
    }
    history
}

/// Two fresh games with the same scripted players must evolve identically tick-for-tick.
/// This catches any non-determinism that would diverge between process restarts.
#[test]
fn identical_scripted_runs_are_identical() {
    const TICKS: u32 = 600;

    let players = vec![
        PlayerInit {
            id: 1,
            name: "A".into(),
            color: "#4cc9f0".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            name: "B".into(),
            color: "#f72585".into(),
            is_ai: false,
        },
    ];
    let start = Game::new(&players).start_payload();

    let mut game_a = Game::new(&players);
    let mut scripts_a: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(MineOnlyScript::new(1)),
        Box::new(MineOnlyScript::new(2)),
    ];
    let history_a = run_scripted_ticks(&players, &mut scripts_a, &start, &mut game_a, TICKS);

    let mut game_b = Game::new(&players);
    let mut scripts_b: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(MineOnlyScript::new(1)),
        Box::new(MineOnlyScript::new(2)),
    ];
    let history_b = run_scripted_ticks(&players, &mut scripts_b, &start, &mut game_b, TICKS);

    for (tick, (snaps_a, snaps_b)) in history_a.iter().zip(&history_b).enumerate() {
        for p in &players {
            assert_eq!(
                snaps_a[&p.id], snaps_b[&p.id],
                "tick {tick}: player {} snapshots diverged between two fresh runs",
                p.id
            );
        }
    }

    // Command logs must also be identical.
    assert_eq!(
        game_a.command_log(),
        game_b.command_log(),
        "command logs diverged between two fresh runs"
    );
}

/// Two AI opponents produce a deterministic command log; replaying it from a fresh game
/// yields the exact same events and final snapshots.
#[test]
fn ai_vs_ai_replay_is_deterministic() {
    let players = vec![
        PlayerInit {
            id: 1,
            name: "AI Alpha".into(),
            color: "#4cc9f0".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            name: "AI Beta".into(),
            color: "#f72585".into(),
            is_ai: true,
        },
    ];
    let mut game = Game::new(&players);

    let mut event_log = Vec::new();
    for tick in 1..=1200 {
        for (player_id, events) in game.tick() {
            for event in events {
                event_log.push(EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    assert_replay_matches_live(&game, &players, &event_log).unwrap_or_else(|failure| {
        panic!("AI vs AI replay determinism failed: {}", failure.reason);
    });
}
