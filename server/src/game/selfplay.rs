//! Test-only API-driven self-play harness.
//!
//! This deliberately drives the public [`Game`] seam (`enqueue`, `tick`, `snapshot_for`) instead
//! of reaching into simulation internals. The scripted players behave like deterministic API
//! clients: observe a fog-filtered snapshot, issue ordinary commands, and let the authoritative
//! simulation validate every action.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::ai_core::decision::{decide_profile, AiDecisionMemory, AiIntent};
#[cfg(test)]
use super::ai_core::profiles::RIFLE_FLOOD_FAST_ID;
use super::ai_core::profiles::{
    profile_by_id, required_profiles, AiProfile, RIFLE_FLOOD_FULL_SATURATION,
    RIFLE_FLOOD_FULL_SATURATION_ID, STEEL_EXPANSION_TANKS_ID, TECH_TO_TANKS_ID,
};
use super::replay::{replay_commands, EventLogEntry, PlayerSnapshot, ReplayOutcome};
use super::{Game, PlayerInit};
use crate::config;
use crate::game::ai_core::actions::{self, AiActionContext, ResourceAssignmentPolicy, SpendBudget};
use crate::game::ai_core::facts::AiFacts;
use crate::game::ai_core::observation::{AiBuildIntent, AiObservation};
use crate::game::ai_shared;
use crate::game::command::SimCommand as Command;
use crate::game::entity::EntityKind;
#[cfg(test)]
use crate::protocol::PlayerStart;
use crate::protocol::{
    kinds, states, terrain, Command as WireCommand, EntityView, Event, MapInfo, Snapshot,
    StartPayload,
};
use crate::rules;

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
const SELFPLAY_ATTACK_STAGE_SUPPRESSION_TICKS: u32 = 3_600;
const RESOURCE_SANITY_LIMIT: u32 = 1_000_000;
const SELFPLAY_FAILURE_DIR: &str = "selfplay-failures";
const SELFPLAY_ARTIFACT_DIR: &str = "selfplay-artifacts";
const SAVE_REPLAY_ENV: &str = "RTS_SELFPLAY_SAVE_REPLAY";

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
                is_ai: true,
            },
            PlayerInit {
                id: 2,
                name: "Bravo Script".to_string(),
                color: "#b2775f".to_string(),
                is_ai: true,
            },
        ];
        let scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
            Box::new(ProfileBackedScript::new(players[0].id, TECH_TO_TANKS_ID)),
            Box::new(ProfileBackedScript::new(players[1].id, TECH_TO_TANKS_ID)),
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

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ReplayArtifact {
    pub(crate) replay_commands: Vec<super::replay::CommandLogEntry>,
    pub(crate) players: Vec<PlayerInit>,
    #[serde(default)]
    pub(crate) seed: u32,
    /// Starting steel each player began the match with. Defaults to [`config::STARTING_STEEL`]
    /// so legacy replays (recorded before quickstart was persisted) still load.
    #[serde(default = "default_starting_steel")]
    pub(crate) starting_steel: u32,
    /// Starting oil each player began the match with. See [`ReplayArtifact::starting_steel`].
    #[serde(default = "default_starting_oil")]
    pub(crate) starting_oil: u32,
}

fn default_starting_steel() -> u32 {
    config::STARTING_STEEL
}

fn default_starting_oil() -> u32 {
    config::STARTING_OIL
}

pub(crate) struct ReplayDriver {
    commands: Vec<super::replay::CommandLogEntry>,
    next: usize,
    seed: u32,
    starting_steel: u32,
    starting_oil: u32,
}

impl ReplayDriver {
    pub(crate) fn from_artifact(artifact: ReplayArtifact) -> (Vec<PlayerInit>, Self) {
        (
            artifact.players,
            Self {
                commands: artifact.replay_commands,
                next: 0,
                seed: artifact.seed,
                starting_steel: artifact.starting_steel,
                starting_oil: artifact.starting_oil,
            },
        )
    }

    pub(crate) fn seed(&self) -> u32 {
        self.seed
    }

    pub(crate) fn starting_steel(&self) -> u32 {
        self.starting_steel
    }

    pub(crate) fn starting_oil(&self) -> u32 {
        self.starting_oil
    }

    pub(crate) fn enqueue_for_tick(&mut self, game: &mut Game) {
        let next_tick = game.tick_count().saturating_add(1);
        while let Some(entry) = self.commands.get(self.next) {
            if entry.tick != next_tick {
                break;
            }
            game.enqueue(
                entry.player_id,
                Command::from_protocol(entry.command.clone()),
            );
            self.next += 1;
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProfileMatchupOptions {
    pub(crate) profile_a: String,
    pub(crate) profile_b: String,
    pub(crate) seed: u32,
    pub(crate) max_ticks: u32,
    pub(crate) verify_replay: bool,
    pub(crate) save_replay_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileMatchupResult {
    pub(crate) profile_a: String,
    pub(crate) profile_b: String,
    pub(crate) seed: u32,
    pub(crate) max_ticks: u32,
    pub(crate) ticks: u32,
    pub(crate) completed_by_elimination: bool,
    pub(crate) winner: Option<ProfileMatchupWinner>,
    pub(crate) players: Vec<ProfileMatchupPlayerResult>,
    pub(crate) first_damage_tick: Option<u32>,
    pub(crate) attack_events: usize,
    pub(crate) death_events: usize,
    pub(crate) event_count: usize,
    pub(crate) replay_verified: bool,
    pub(crate) replay_artifact: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileMatchupWinner {
    pub(crate) player_id: u32,
    pub(crate) profile: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileMatchupPlayerResult {
    pub(crate) player_id: u32,
    pub(crate) profile: String,
    pub(crate) alive: bool,
    pub(crate) command_count: usize,
    pub(crate) attack_command_count: usize,
    pub(crate) first_attack_command_tick: Option<u32>,
    pub(crate) first_tank_tick: Option<u32>,
    pub(crate) final_counts: BTreeMap<String, u32>,
}

pub(crate) fn available_profile_ids() -> Vec<&'static str> {
    required_profiles()
        .into_iter()
        .map(|profile| profile.id)
        .collect()
}

pub(crate) fn canonical_profile_id(input: &str) -> Option<&'static str> {
    match input {
        "rush" | "fast" => Some("rifle_flood_fast"),
        "saturation" | "full" | "macro" => Some(RIFLE_FLOOD_FULL_SATURATION_ID),
        "tech" | "tanks" => Some(TECH_TO_TANKS_ID),
        "expand" | "expansion" | "steel" | "steel_tanks" => Some(STEEL_EXPANSION_TANKS_ID),
        id => profile_by_id(id).map(|profile| profile.id),
    }
}

pub(crate) fn run_profile_matchup_result(
    options: ProfileMatchupOptions,
) -> Result<ProfileMatchupResult, String> {
    let profile_a = profile_by_id(&options.profile_a)
        .ok_or_else(|| format!("unknown profile: {}", options.profile_a))?;
    let profile_b = profile_by_id(&options.profile_b)
        .ok_or_else(|| format!("unknown profile: {}", options.profile_b))?;
    if options.max_ticks == 0 {
        return Err("max ticks must be greater than zero".to_string());
    }
    if let Some(name) = &options.save_replay_name {
        if !is_safe_artifact_name(name) {
            return Err(format!("unsafe replay artifact name: {name}"));
        }
    }

    let players = vec![
        PlayerInit {
            id: 1,
            name: profile_a.id.to_string(),
            color: "#4cc9f0".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            name: profile_b.id.to_string(),
            color: "#f72585".to_string(),
            is_ai: true,
        },
    ];
    let mut game = Game::new_without_ai_controllers(&players, options.seed);
    let start = game.start_payload();
    let mut scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(ProfileBackedScript::new(1, profile_a.id)),
        Box::new(ProfileBackedScript::new(2, profile_b.id)),
    ];
    let mut event_log = Vec::new();
    let mut first_tank_tick: BTreeMap<u32, u32> = BTreeMap::new();
    let mut first_damage_tick = None;
    let mut attack_events = 0usize;
    let mut death_events = 0usize;

    while game.tick_count() < options.max_ticks {
        let alive = game.alive_players();
        if alive.len() <= 1 {
            break;
        }

        let tick = game.tick_count();
        let mut commands = Vec::new();
        for script in &mut scripts {
            let player_id = script.player_id();
            let snapshot = game.snapshot_for(player_id);
            observe_first_tank_tick(tick, player_id, &snapshot, &mut first_tank_tick);
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

        let tick_events = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.tick()))
            .map_err(|payload| {
                format!(
                    "Game::tick panicked during profile matchup: {}",
                    panic_payload_to_string(&payload)
                )
            })?;
        let event_tick = game.tick_count();
        for player in &players {
            let snapshot = game.snapshot_for(player.id);
            observe_first_tank_tick(event_tick, player.id, &snapshot, &mut first_tank_tick);
        }
        for (player_id, events) in tick_events {
            for event in events {
                match &event {
                    Event::Attack { .. } => {
                        first_damage_tick.get_or_insert(event_tick);
                        attack_events += 1;
                    }
                    Event::Death { .. } => {
                        death_events += 1;
                    }
                    Event::Build { .. } | Event::Notice { .. } => {}
                }
                event_log.push(EventLogEntry {
                    tick: event_tick,
                    player_id,
                    event,
                });
            }
        }
    }

    let replay_verified = if options.verify_replay {
        assert_replay_matches_live(&game, &players, &event_log)
            .map_err(|failure| format!("replay determinism failed: {}", failure.reason))?;
        true
    } else {
        false
    };

    let replay_artifact = match &options.save_replay_name {
        Some(name) => Some(
            write_simple_replay_artifact(name, &game, &players)?
                .display()
                .to_string(),
        ),
        None => None,
    };

    let alive = game.alive_players();
    let winner = if alive.len() == 1 {
        let player_id = alive[0];
        Some(ProfileMatchupWinner {
            player_id,
            profile: if player_id == 1 {
                profile_a.id.to_string()
            } else {
                profile_b.id.to_string()
            },
        })
    } else {
        None
    };
    let final_counts = final_unit_counts(&game, &players);
    let command_stats = command_stats_by_player(game.command_log());
    let players = players
        .iter()
        .map(|player| {
            let stats = command_stats.get(&player.id);
            ProfileMatchupPlayerResult {
                player_id: player.id,
                profile: if player.id == 1 {
                    profile_a.id.to_string()
                } else {
                    profile_b.id.to_string()
                },
                alive: alive.contains(&player.id),
                command_count: stats.map(|s| s.command_count).unwrap_or_default(),
                attack_command_count: stats.map(|s| s.attack_command_count).unwrap_or_default(),
                first_attack_command_tick: stats.and_then(|s| s.first_attack_command_tick),
                first_tank_tick: first_tank_tick.get(&player.id).copied(),
                final_counts: final_counts.get(&player.id).cloned().unwrap_or_default(),
            }
        })
        .collect();

    Ok(ProfileMatchupResult {
        profile_a: profile_a.id.to_string(),
        profile_b: profile_b.id.to_string(),
        seed: options.seed,
        max_ticks: options.max_ticks,
        ticks: game.tick_count(),
        completed_by_elimination: alive.len() <= 1,
        winner,
        players,
        first_damage_tick,
        attack_events,
        death_events,
        event_count: event_log.len(),
        replay_verified,
        replay_artifact,
    })
}

#[derive(Default)]
struct CommandStats {
    command_count: usize,
    attack_command_count: usize,
    first_attack_command_tick: Option<u32>,
}

fn command_stats_by_player(
    commands: &[super::replay::CommandLogEntry],
) -> BTreeMap<u32, CommandStats> {
    let mut stats: BTreeMap<u32, CommandStats> = BTreeMap::new();
    for entry in commands {
        let player = stats.entry(entry.player_id).or_default();
        player.command_count += 1;
        match &entry.command {
            WireCommand::AttackMove { .. } | WireCommand::Attack { .. } => {
                player.attack_command_count += 1;
                player.first_attack_command_tick.get_or_insert(entry.tick);
            }
            WireCommand::Move { .. }
            | WireCommand::Gather { .. }
            | WireCommand::Build { .. }
            | WireCommand::Train { .. }
            | WireCommand::Cancel { .. }
            | WireCommand::Stop { .. }
            | WireCommand::SetRally { .. } => {}
        }
    }
    stats
}

fn observe_first_tank_tick(
    tick: u32,
    player_id: u32,
    snapshot: &Snapshot,
    first_tank_tick: &mut BTreeMap<u32, u32>,
) {
    if first_tank_tick.contains_key(&player_id) {
        return;
    }
    if snapshot
        .entities
        .iter()
        .any(|entity| entity.owner == player_id && entity.kind == kinds::TANK)
    {
        first_tank_tick.insert(player_id, tick);
    }
}

fn write_simple_replay_artifact(
    name: &str,
    game: &Game,
    players: &[PlayerInit],
) -> Result<PathBuf, String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(SELFPLAY_ARTIFACT_DIR)
        .join(name);
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let artifact = ReplayArtifact {
        replay_commands: game.command_log().to_vec(),
        players: players.to_vec(),
        seed: game.seed(),
        starting_steel: game.starting_steel(),
        starting_oil: game.starting_oil(),
    };
    let json = serde_json::to_vec_pretty(&artifact).map_err(|err| err.to_string())?;
    fs::write(dir.join("replay.json"), json).map_err(|err| err.to_string())?;
    Ok(dir)
}

fn panic_payload_to_string(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic without string payload".to_string()
    }
}

#[derive(Clone, Copy)]
struct PlayerView<'a> {
    player_id: u32,
    tick: u32,
    start: &'a StartPayload,
    snapshot: &'a Snapshot,
}

impl PlayerView<'_> {
    fn observation(
        self,
        pending_builds: impl IntoIterator<Item = AiBuildIntent>,
    ) -> Option<AiObservation> {
        AiObservation::from_selfplay_snapshot(
            self.start,
            self.snapshot,
            self.player_id,
            pending_builds,
        )
    }
}

const FAILED_SPOTS_CAP: usize = 16;
/// Force a pending build to be treated as failed after this many ticks without worker movement so
/// stale commands do not suppress future build attempts forever if a worker gets stuck.
const PENDING_BUILD_STALE_TICKS: u32 = 300;
const PENDING_BUILD_PROGRESS_EPS_PX: f32 = 4.0;

#[derive(Clone, Copy)]
struct PendingBuild {
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
    last_x: Option<f32>,
    last_y: Option<f32>,
    last_progress_tick: u32,
}

impl PendingBuild {
    fn observe_worker(&mut self, worker: &EntityView, tick: u32) {
        let Some(last_x) = self.last_x else {
            self.last_x = Some(worker.x);
            self.last_y = Some(worker.y);
            self.last_progress_tick = tick;
            return;
        };
        let last_y = self.last_y.unwrap_or(worker.y);
        let dx = worker.x - last_x;
        let dy = worker.y - last_y;
        if dx * dx + dy * dy >= PENDING_BUILD_PROGRESS_EPS_PX * PENDING_BUILD_PROGRESS_EPS_PX {
            self.last_x = Some(worker.x);
            self.last_y = Some(worker.y);
            self.last_progress_tick = tick;
        }
    }

    fn stale_at(self, tick: u32) -> bool {
        tick.saturating_sub(self.last_progress_tick) >= PENDING_BUILD_STALE_TICKS
    }
}

#[derive(Default)]
struct PendingBuildTracker {
    pending: BTreeMap<u32, PendingBuild>,
    failed_spots: HashMap<EntityKind, BTreeSet<(u32, u32)>>,
}

impl PendingBuildTracker {
    fn observe(&mut self, view: PlayerView<'_>) {
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
        let mut dropped = Vec::new();
        self.pending.retain(|worker_id, pending| {
            let worker = workers
                .iter()
                .copied()
                .find(|w| w.id == *worker_id && w.state == states::BUILD);
            let keep = worker
                .map(|worker| {
                    pending.observe_worker(worker, view.tick);
                    !pending.stale_at(view.tick)
                })
                .unwrap_or(false);
            if !keep {
                dropped.push(*pending);
            }
            keep
        });
        for pending in dropped {
            let succeeded = own.iter().any(|e| {
                is_kind(e, pending.kind)
                    && building_footprint_tiles(&view.start.map, e)
                        .contains(&(pending.tile_x, pending.tile_y))
            });
            if succeeded {
                self.failed_spots.remove(&pending.kind);
            } else {
                let set = self.failed_spots.entry(pending.kind).or_default();
                set.insert((pending.tile_x, pending.tile_y));
                if set.len() > FAILED_SPOTS_CAP {
                    set.clear();
                }
            }
        }
    }

    fn intents(&self) -> Vec<AiBuildIntent> {
        self.pending
            .iter()
            .map(|(worker_id, pending)| {
                AiBuildIntent::to_site(*worker_id, pending.kind, pending.tile_x, pending.tile_y)
            })
            .collect()
    }

    fn record_commands(&mut self, tick: u32, commands: &[Command]) {
        for command in commands {
            let Command::Build {
                worker,
                building,
                tile_x,
                tile_y,
            } = command
            else {
                continue;
            };
            if config::building_stats(*building).is_none() {
                continue;
            }
            self.pending.insert(
                *worker,
                PendingBuild {
                    kind: *building,
                    tile_x: *tile_x,
                    tile_y: *tile_y,
                    last_x: None,
                    last_y: None,
                    last_progress_tick: tick,
                },
            );
        }
    }

    fn failed(&self, kind: EntityKind, tile_x: u32, tile_y: u32) -> bool {
        self.failed_spots
            .get(&kind)
            .map(|spots| spots.contains(&(tile_x, tile_y)))
            .unwrap_or(false)
    }
}

struct ProfileBackedScript {
    player_id: u32,
    profile: &'static AiProfile,
    memory: AiDecisionMemory,
    pending_builds: PendingBuildTracker,
    staged_units: BTreeSet<u32>,
    active_attack_units: BTreeMap<u32, u32>,
    allow_combat_commands: bool,
    script_name: &'static str,
}

impl ProfileBackedScript {
    fn new(player_id: u32, profile_id: &'static str) -> Self {
        Self::with_combat(player_id, profile_id, true, profile_id)
    }

    fn economy_only(player_id: u32) -> Self {
        Self::with_combat(
            player_id,
            RIFLE_FLOOD_FULL_SATURATION_ID,
            false,
            "profile-economy",
        )
    }

    fn with_combat(
        player_id: u32,
        profile_id: &'static str,
        allow_combat_commands: bool,
        script_name: &'static str,
    ) -> Self {
        let profile = profile_by_id(profile_id).unwrap_or(&RIFLE_FLOOD_FULL_SATURATION);
        Self {
            player_id,
            profile,
            memory: AiDecisionMemory::for_profile(profile),
            pending_builds: PendingBuildTracker::default(),
            staged_units: BTreeSet::new(),
            active_attack_units: BTreeMap::new(),
            allow_combat_commands,
            script_name,
        }
    }

    fn should_think(&self, tick: u32) -> bool {
        tick == 0
            || tick
                .wrapping_add(self.player_id)
                .is_multiple_of(THINK_INTERVAL)
    }
}

impl ScriptedPlayer for ProfileBackedScript {
    fn player_id(&self) -> u32 {
        self.player_id
    }

    fn name(&self) -> &'static str {
        self.script_name
    }

    fn commands(&mut self, view: PlayerView<'_>) -> Vec<Command> {
        if !self.should_think(view.tick) {
            return Vec::new();
        }

        self.pending_builds.observe(view);
        let Some(observation) = view.observation(self.pending_builds.intents()) else {
            return Vec::new();
        };
        self.prune_combat_memory(&observation, view.tick);

        let occupied = occupied_tiles_from_snapshot(&view.start.map, view.snapshot);
        let failed_builds = &self.pending_builds;
        let decision = decide_profile(
            &observation,
            self.profile,
            &mut self.memory,
            ai_shared::BuildSearch::default(),
            |building, tile_x, tile_y| {
                !failed_builds.failed(building, tile_x, tile_y)
                    && footprint_placeable_from_snapshot(
                        &view.start.map,
                        view.snapshot,
                        building,
                        tile_x,
                        tile_y,
                        &occupied,
                    )
            },
        );
        debug_assert_eq!(decision.profile_id, self.profile.id);

        let combat_intent_units = combat_intent_units(&decision.intents);
        let mut commands =
            self.filter_repeated_stage_commands(view.tick, &decision.intents, decision.commands);
        if !self.allow_combat_commands {
            commands.retain(|command| !is_combat_command(command, &combat_intent_units));
        }
        self.pending_builds.record_commands(view.tick, &commands);
        commands
    }
}

fn combat_intent_units(intents: &[AiIntent]) -> BTreeSet<u32> {
    let mut units = BTreeSet::new();
    for intent in intents {
        if let AiIntent::Attack { units: attacking } = intent {
            units.extend(attacking.iter().copied());
        }
    }
    units
}

fn is_combat_command(command: &Command, combat_intent_units: &BTreeSet<u32>) -> bool {
    match command {
        Command::Attack { .. } | Command::AttackMove { .. } => true,
        Command::Move { units, .. } => units.iter().any(|id| combat_intent_units.contains(id)),
        Command::Gather { .. }
        | Command::Build { .. }
        | Command::Train { .. }
        | Command::Cancel { .. }
        | Command::Stop { .. }
        | Command::SetRally { .. }
        | Command::Rejected { .. } => false,
    }
}

impl ProfileBackedScript {
    fn prune_combat_memory(&mut self, observation: &AiObservation, tick: u32) {
        let owned: BTreeSet<u32> = observation.owned.iter().map(|entity| entity.id).collect();
        self.staged_units.retain(|id| owned.contains(id));
        let suppress_ticks = self
            .profile
            .attack
            .reissue_cadence_ticks
            .max(SELFPLAY_ATTACK_STAGE_SUPPRESSION_TICKS);
        self.active_attack_units.retain(|id, issued| {
            owned.contains(id) && tick.saturating_sub(*issued) < suppress_ticks
        });
    }

    fn filter_repeated_stage_commands(
        &mut self,
        tick: u32,
        intents: &[AiIntent],
        commands: Vec<Command>,
    ) -> Vec<Command> {
        let mut attacking = BTreeSet::new();
        let mut staging = BTreeSet::new();
        for intent in intents {
            match intent {
                AiIntent::Attack { units } => attacking.extend(units.iter().copied()),
                AiIntent::Stage { units } => staging.extend(units.iter().copied()),
                AiIntent::Move { .. }
                | AiIntent::Build { .. }
                | AiIntent::Train { .. }
                | AiIntent::Gather { .. } => {}
            }
        }
        for id in &attacking {
            self.staged_units.remove(id);
            self.active_attack_units.insert(*id, tick);
        }
        if staging.is_empty() {
            return commands;
        }

        let mut filtered = Vec::new();
        for command in commands {
            match command {
                Command::AttackMove { units, x, y }
                    if units.iter().any(|id| staging.contains(id)) =>
                {
                    let fresh: Vec<u32> = units
                        .into_iter()
                        .filter(|id| !self.staged_units.contains(id))
                        .filter(|id| !self.active_attack_units.contains_key(id))
                        .collect();
                    self.staged_units.extend(fresh.iter().copied());
                    if !fresh.is_empty() {
                        filtered.push(Command::AttackMove { units: fresh, x, y });
                    }
                }
                other => filtered.push(other),
            }
        }
        filtered
    }
}

struct WorkerRushScript {
    player_id: u32,
    target_player_id: u32,
    last_attack_tick: u32,
}

// Intentionally retained as special harness coverage: this is an all-in worker pull, not a normal
// strategy profile. The profile-backed `rifle_flood_fast` covers early rifle pressure separately.
impl WorkerRushScript {
    fn new(player_id: u32, target_player_id: u32) -> Self {
        WorkerRushScript {
            player_id,
            target_player_id,
            last_attack_tick: 0,
        }
    }

    fn should_think(&self, tick: u32) -> bool {
        tick == 0
            || tick
                .wrapping_add(self.player_id)
                .is_multiple_of(THINK_INTERVAL)
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
        let Some(observation) = view.observation([]) else {
            return Vec::new();
        };
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(
            &facts,
            SpendBudget::new(
                view.snapshot.steel,
                view.snapshot.oil,
                view.snapshot.supply_used,
                view.snapshot.supply_cap,
            ),
        );
        self.last_attack_tick = view.tick;
        actions::attack_move_units(&mut actions, workers, x, y);
        actions.into_commands()
    }
}

struct SelfPlayRunner {
    test_name: &'static str,
    max_ticks: u32,
    game: Game,
    start: StartPayload,
    resource_kinds: BTreeMap<u32, EntityKind>,
    player_specs: Vec<PlayerInit>,
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
        player_specs: Vec<PlayerInit>,
        scripts: Vec<Box<dyn ScriptedPlayer>>,
    ) -> Self {
        let milestones = Milestones::tech_combat_for_players(player_specs.iter().map(|p| p.id));
        SelfPlayRunner::with_milestones(test_name, game, start, player_specs, scripts, milestones)
    }

    fn with_milestones(
        test_name: &'static str,
        game: Game,
        start: StartPayload,
        player_specs: Vec<PlayerInit>,
        scripts: Vec<Box<dyn ScriptedPlayer>>,
        milestones: Milestones,
    ) -> Self {
        SelfPlayRunner::with_options(
            test_name,
            MAX_TICKS,
            game,
            start,
            player_specs,
            scripts,
            milestones,
        )
    }

    fn with_options(
        test_name: &'static str,
        max_ticks: u32,
        game: Game,
        start: StartPayload,
        player_specs: Vec<PlayerInit>,
        scripts: Vec<Box<dyn ScriptedPlayer>>,
        milestones: Milestones,
    ) -> Self {
        let resource_kinds = resource_kinds_from_start(&start);
        SelfPlayRunner {
            test_name,
            max_ticks,
            game,
            start,
            resource_kinds,
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

        for _ in 0..=self.max_ticks {
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
            let alive = self.game.alive_players();
            if alive.len() < self.player_specs.len() {
                return Err(SelfPlayFailure::new(format!(
                    "self-play ended by elimination before all milestones: alive={alive:?}; missing={}",
                    self.milestones.missing_summary()
                )));
            }
            if tick >= self.max_ticks {
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

            let mut command_progressed = false;
            for (player_id, script, command) in commands {
                if let Some(wire_command) = command.to_protocol() {
                    self.commands.push(CommandRecord {
                        tick,
                        player_id,
                        script,
                        command: wire_command,
                    });
                }
                command_progressed |= self.milestones.observe_command(tick, player_id, &command);
                self.game.enqueue(player_id, command);
            }
            if command_progressed {
                last_progress_tick = tick;
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
            "self-play did not complete all milestones within {} ticks: {}",
            self.max_ticks,
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
        if tick == 0 || tick.is_multiple_of(SAMPLE_EVERY_TICKS) {
            for (player_id, snapshot) in snapshots {
                self.samples
                    .push(SnapshotSample::from_snapshot(tick, *player_id, snapshot));
            }
        }
        self.milestones
            .observe_snapshots(tick, snapshots, &self.resource_kinds)
    }

    fn record_events(&mut self, tick: u32, tick_events: Vec<(u32, Vec<Event>)>) -> bool {
        let mut progressed = false;
        for (player_id, events) in tick_events {
            for event in events {
                let attacker = match &event {
                    Event::Attack { from, .. } => self.attacker_info(*from),
                    Event::Death { .. } | Event::Build { .. } | Event::Notice { .. } => None,
                };
                progressed |= self
                    .milestones
                    .observe_combat_event(tick, player_id, attacker, &event);
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

    fn attacker_info(&self, attacker: u32) -> Option<AttackerInfo> {
        let viewer = self.player_specs.first()?.id;
        self.game
            .snapshot_full_for(viewer)
            .entities
            .iter()
            .find(|e| e.id == attacker)
            .and_then(|e| {
                kind_of(e).map(|kind| AttackerInfo {
                    owner: e.owner,
                    kind,
                })
            })
    }

    fn write_failure_artifact(&self, failure: &SelfPlayFailure) -> Result<PathBuf, String> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        let dir = self.artifact_root(SELFPLAY_FAILURE_DIR).join(format!(
            "{}-{}-{}",
            self.test_name,
            std::process::id(),
            now_ms
        ));
        let artifact = self.artifact_payload(Some(failure.reason.clone()));
        self.write_artifact_dir(&dir, &artifact)?;
        Ok(dir)
    }

    fn write_success_artifact_if_requested(&self) -> Result<Option<PathBuf>, String> {
        let Some(name) = requested_replay_artifact_name(self.test_name)? else {
            return Ok(None);
        };
        let dir = self.artifact_root(SELFPLAY_ARTIFACT_DIR).join(name);
        let artifact = self.artifact_payload(None);
        self.write_artifact_dir(&dir, &artifact)?;
        Ok(Some(dir))
    }

    fn artifact_payload(&self, failure: Option<String>) -> SelfPlayArtifact {
        SelfPlayArtifact {
            test_name: self.test_name,
            failure,
            start: self.start.clone(),
            players: self.player_specs.clone(),
            milestones: self.milestones.clone(),
            commands: self.commands.clone(),
            replay_commands: self.game.command_log().to_vec(),
            events: self.events.clone(),
            replay_events: self.event_log.clone(),
            samples: self.samples.clone(),
            seed: self.game.seed(),
        }
    }

    fn artifact_root(&self, dir_name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(dir_name)
    }

    fn write_artifact_dir(&self, dir: &Path, artifact: &SelfPlayArtifact) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_vec_pretty(&artifact).map_err(|e| e.to_string())?;
        fs::write(dir.join("replay.json"), json).map_err(|e| e.to_string())?;
        fs::write(dir.join("summary.log"), artifact.summary_log()).map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn requested_replay_artifact_name(test_name: &str) -> Result<Option<String>, String> {
    let Some(raw) = env::var_os(SAVE_REPLAY_ENV) else {
        return Ok(None);
    };
    let raw = raw.to_string_lossy();
    let value = raw.trim();
    if value.is_empty()
        || value.eq_ignore_ascii_case("0")
        || value.eq_ignore_ascii_case("false")
        || value.eq_ignore_ascii_case("no")
    {
        return Ok(None);
    }
    if value.eq_ignore_ascii_case("1")
        || value.eq_ignore_ascii_case("true")
        || value.eq_ignore_ascii_case("yes")
    {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        return Ok(Some(format!("{test_name}-{}-{now_ms}", std::process::id())));
    }
    if is_safe_artifact_name(value) {
        return Ok(Some(value.to_string()));
    }
    Err(format!(
        "{SAVE_REPLAY_ENV} must be 1/true/yes for an auto-generated name or a safe artifact name"
    ))
}

fn resource_kinds_from_start(start: &StartPayload) -> BTreeMap<u32, EntityKind> {
    start
        .map
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .kind
                .parse::<EntityKind>()
                .ok()
                .map(|kind| (resource.id, kind))
        })
        .collect()
}

pub(crate) fn is_safe_artifact_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
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
struct CommandRecord {
    tick: u32,
    player_id: u32,
    script: &'static str,
    command: WireCommand,
}

#[derive(Clone, Serialize)]
struct EventRecord {
    tick: u32,
    player_id: u32,
    event: Event,
}

#[derive(Clone, Copy)]
struct AttackerInfo {
    owner: u32,
    kind: EntityKind,
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
    first_damage_tick: Option<u32>,
    first_damage_tick_by_attacker: BTreeMap<u32, u32>,
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
            first_damage_tick: None,
            first_damage_tick_by_attacker: BTreeMap::new(),
        }
    }

    fn observe_snapshots(
        &mut self,
        tick: u32,
        snapshots: &BTreeMap<u32, Snapshot>,
        resource_kinds: &BTreeMap<u32, EntityKind>,
    ) -> bool {
        let mut changed = false;
        for (player_id, snapshot) in snapshots {
            if let Some(player) = self.players.get_mut(player_id) {
                changed |= player.observe(tick, *player_id, snapshot, resource_kinds);
            }
        }
        changed
    }

    fn observe_command(&mut self, tick: u32, player_id: u32, command: &Command) -> bool {
        let Some(player) = self.players.get_mut(&player_id) else {
            return false;
        };
        let Some(goal) = self.goals.get(&player_id) else {
            return false;
        };
        player.observe_command(tick, goal, command)
    }

    fn observe_combat_event(
        &mut self,
        tick: u32,
        player_id: u32,
        attacker: Option<AttackerInfo>,
        event: &Event,
    ) -> bool {
        let before_damage_tick = self.first_damage_tick;
        let changed = match event {
            Event::Attack { .. } => {
                self.attack_events += 1;
                self.first_damage_tick.get_or_insert(tick);
                if let Some(attacker) = attacker {
                    *self
                        .attack_events_by_player
                        .entry(attacker.owner)
                        .or_default() += 1;
                    self.first_damage_tick_by_attacker
                        .entry(attacker.owner)
                        .or_insert(tick);
                    if attacker.kind == EntityKind::Worker {
                        *self
                            .worker_attack_events_by_player
                            .entry(attacker.owner)
                            .or_default() += 1;
                    }
                } else {
                    *self.attack_events_by_player.entry(player_id).or_default() += 1;
                }
                true
            }
            Event::Death { .. } => {
                self.death_events += 1;
                true
            }
            Event::Build { .. } | Event::Notice { .. } => false,
        };
        changed || before_damage_tick != self.first_damage_tick
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
    require_damage: bool,
    min_attacks_by_player: BTreeMap<u32, u32>,
    min_worker_attacks_by_player: BTreeMap<u32, u32>,
}

impl CombatGoal {
    fn any_combat() -> Self {
        CombatGoal {
            require_any_combat: true,
            ..CombatGoal::default()
        }
    }

    fn damage() -> Self {
        CombatGoal {
            require_damage: true,
            ..CombatGoal::default()
        }
    }

    fn worker_attack_by(player_id: u32) -> Self {
        CombatGoal {
            min_worker_attacks_by_player: BTreeMap::from([(player_id, 1)]),
            ..CombatGoal::default()
        }
    }

    fn complete(&self, milestones: &Milestones) -> bool {
        if self.require_any_combat && milestones.attack_events == 0 && milestones.death_events == 0
        {
            return false;
        }
        if self.require_damage && milestones.first_damage_tick.is_none() {
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
        true
    }

    fn missing(&self, milestones: &Milestones) -> Vec<String> {
        let mut out = Vec::new();
        if self.require_any_combat && milestones.attack_events == 0 && milestones.death_events == 0
        {
            out.push("combat-event".to_string());
        }
        if self.require_damage && milestones.first_damage_tick.is_none() {
            out.push("damage-event".to_string());
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
        out
    }
}

#[derive(Clone, Default, Serialize)]
struct PlayerMilestoneGoal {
    require_gathering: bool,
    require_oil: bool,
    require_oil_worker_assignment: bool,
    require_depot_supply: bool,
    require_barracks_complete: bool,
    require_rifleman: bool,
    require_tank: bool,
    require_damage_taken: bool,
    allow_elimination_before_milestones: bool,
    min_workers: u32,
    min_supply_cap: u32,
    min_attack_command_units: u32,
    min_units_by_kind: BTreeMap<&'static str, u32>,
    min_buildings_by_kind: BTreeMap<&'static str, u32>,
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

    fn damaged_economy() -> Self {
        PlayerMilestoneGoal {
            require_gathering: true,
            require_damage_taken: true,
            min_workers: config::STARTING_WORKERS + 2,
            ..PlayerMilestoneGoal::default()
        }
    }

    fn with_min_workers(mut self, min_workers: u32) -> Self {
        self.min_workers = min_workers;
        self
    }

    fn with_min_supply_cap(mut self, min_supply_cap: u32) -> Self {
        self.min_supply_cap = min_supply_cap;
        self
    }

    fn with_min_attack_command_units(mut self, min_units: u32) -> Self {
        self.min_attack_command_units = min_units;
        self
    }

    fn with_min_units(mut self, kind: &'static str, count: u32) -> Self {
        self.min_units_by_kind.insert(kind, count);
        self
    }

    fn with_min_buildings(mut self, kind: &'static str, count: u32) -> Self {
        self.min_buildings_by_kind.insert(kind, count);
        self
    }

    fn allowing_elimination_before_milestones(mut self) -> Self {
        self.allow_elimination_before_milestones = true;
        self
    }
}

#[derive(Clone, Default, PartialEq, Serialize)]
struct PlayerMilestones {
    saw_owned_entities: bool,
    eliminated: bool,
    saw_gathering: bool,
    oil_gathered: bool,
    oil_worker_assigned: bool,
    depot_started: bool,
    barracks_started: bool,
    barracks_complete: bool,
    rifleman_trained: bool,
    tank_trained: bool,
    damage_taken: bool,
    first_attack_command_tick: Option<u32>,
    first_goal_attack_command_tick: Option<u32>,
    first_tank_tick: Option<u32>,
    first_damage_tick: Option<u32>,
    max_workers: u32,
    max_steel: u32,
    max_oil: u32,
    max_supply_cap: u32,
    max_riflemen: u32,
    max_tanks: u32,
    max_units_by_kind: BTreeMap<String, u32>,
    max_buildings_by_kind: BTreeMap<String, u32>,
}

impl PlayerMilestones {
    fn observe(
        &mut self,
        tick: u32,
        player_id: u32,
        snapshot: &Snapshot,
        resource_kinds: &BTreeMap<u32, EntityKind>,
    ) -> bool {
        let before = self.clone();
        let mut workers = 0;
        let mut riflemen = 0;
        let mut tanks = 0;
        let mut owned_entities = 0;
        let mut owned_buildings = 0;
        let mut units_by_kind = BTreeMap::<String, u32>::new();
        let mut buildings_by_kind = BTreeMap::<String, u32>::new();
        for e in snapshot.entities.iter().filter(|e| e.owner == player_id) {
            owned_entities += 1;
            let Some(k) = kind_of(e) else { continue };
            if k.is_unit() {
                *units_by_kind.entry(e.kind.clone()).or_default() += 1;
            }
            if k.is_building() {
                owned_buildings += 1;
                *buildings_by_kind.entry(e.kind.clone()).or_default() += 1;
            }
            match k {
                EntityKind::Worker => {
                    workers += 1;
                    if e.state == states::GATHER || e.latched_node.is_some() {
                        self.saw_gathering = true;
                    }
                    if e.latched_node
                        .and_then(|node| resource_kinds.get(&node).copied())
                        == Some(EntityKind::Oil)
                    {
                        self.oil_worker_assigned = true;
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
                _ => {}
            }
            if e.hp < e.max_hp {
                self.damage_taken = true;
                self.first_damage_tick.get_or_insert(tick);
            }
        }
        if owned_entities > 0 {
            self.saw_owned_entities = true;
        }
        if self.saw_owned_entities && owned_buildings == 0 {
            self.eliminated = true;
        }
        self.oil_gathered |= snapshot.oil > 0;
        self.max_workers = self.max_workers.max(workers);
        self.max_steel = self.max_steel.max(snapshot.steel);
        self.max_oil = self.max_oil.max(snapshot.oil);
        self.max_supply_cap = self.max_supply_cap.max(snapshot.supply_cap);
        self.max_riflemen = self.max_riflemen.max(riflemen);
        self.max_tanks = self.max_tanks.max(tanks);
        for (kind, count) in units_by_kind {
            self.max_units_by_kind
                .entry(kind)
                .and_modify(|max| *max = (*max).max(count))
                .or_insert(count);
        }
        for (kind, count) in buildings_by_kind {
            self.max_buildings_by_kind
                .entry(kind)
                .and_modify(|max| *max = (*max).max(count))
                .or_insert(count);
        }
        self.rifleman_trained |= riflemen > 0;
        if tanks > 0 {
            self.tank_trained = true;
            self.first_tank_tick.get_or_insert(tick);
        }
        before != *self
    }

    fn observe_command(
        &mut self,
        tick: u32,
        goal: &PlayerMilestoneGoal,
        command: &Command,
    ) -> bool {
        let before = self.clone();
        let attack_units = match command {
            Command::AttackMove { units, .. } | Command::Attack { units, .. } => {
                Some(units.len() as u32)
            }
            Command::Move { units, .. } if self.rifleman_trained => Some(units.len() as u32),
            Command::Move { .. }
            | Command::Gather { .. }
            | Command::Build { .. }
            | Command::Train { .. }
            | Command::Cancel { .. }
            | Command::Stop { .. }
            | Command::SetRally { .. }
            | Command::Rejected { .. } => None,
        };
        if let Some(attack_units) = attack_units {
            self.first_attack_command_tick.get_or_insert(tick);
            if goal.min_attack_command_units > 0 && attack_units >= goal.min_attack_command_units {
                self.first_goal_attack_command_tick.get_or_insert(tick);
            }
        }
        before != *self
    }

    fn complete_for(&self, goal: &PlayerMilestoneGoal) -> bool {
        self.missing_for(goal).is_empty()
    }

    fn missing_for(&self, goal: &PlayerMilestoneGoal) -> Vec<String> {
        if goal.allow_elimination_before_milestones && self.eliminated {
            return Vec::new();
        }

        let mut out = Vec::new();
        if goal.require_gathering && !self.saw_gathering {
            out.push("economy-gather".to_string());
        }
        if goal.require_oil && !self.oil_gathered {
            out.push("oil-gather".to_string());
        }
        if goal.require_oil_worker_assignment && !self.oil_worker_assigned {
            out.push("oil-worker".to_string());
        }
        if goal.require_depot_supply
            && (!self.depot_started || self.max_supply_cap <= config::CITY_CENTRE_SUPPLY)
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
        if goal.require_damage_taken && !self.damage_taken {
            out.push("damage-taken".to_string());
        }
        if self.max_workers < goal.min_workers {
            out.push(format!("workers>={}", goal.min_workers));
        }
        if self.max_supply_cap < goal.min_supply_cap {
            out.push(format!("supply-cap>={}", goal.min_supply_cap));
        }
        if goal.min_attack_command_units > 0 && self.first_goal_attack_command_tick.is_none() {
            out.push(format!(
                "attack-command-units>={}",
                goal.min_attack_command_units
            ));
        }
        for (kind, required) in &goal.min_units_by_kind {
            let seen = self
                .max_units_by_kind
                .get(*kind)
                .copied()
                .unwrap_or_default();
            if seen < *required {
                out.push(format!("{kind}>={required}"));
            }
        }
        for (kind, required) in &goal.min_buildings_by_kind {
            let seen = self
                .max_buildings_by_kind
                .get(*kind)
                .copied()
                .unwrap_or_default();
            if seen < *required {
                out.push(format!("{kind}>={required}"));
            }
        }
        out
    }
}

#[derive(Serialize)]
struct SelfPlayArtifact {
    test_name: &'static str,
    failure: Option<String>,
    start: StartPayload,
    players: Vec<PlayerInit>,
    milestones: Milestones,
    commands: Vec<CommandRecord>,
    replay_commands: Vec<super::replay::CommandLogEntry>,
    events: Vec<EventRecord>,
    replay_events: Vec<EventLogEntry>,
    samples: Vec<SnapshotSample>,
    seed: u32,
}

impl SelfPlayArtifact {
    fn summary_log(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("test: {}\n", self.test_name));
        if let Some(failure) = &self.failure {
            out.push_str(&format!("failure: {failure}\n"));
        } else {
            out.push_str("result: success\n");
        }
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
    replay_commands(
        players,
        game.command_log(),
        game.tick_count(),
        game.seed(),
        game.starting_steel(),
        game.starting_oil(),
    )
    .map_err(|e| SelfPlayFailure::new(format!("replay failed: {e}")))
}

fn finalize_self_play_success(
    runner: &SelfPlayRunner,
    players: &[PlayerInit],
    report: &SelfPlayReport,
) {
    assert!(report.ticks > 0);
    assert!(report.commands > 0);
    assert_eq!(report.commands, report.replay_commands);
    assert_replay_matches_live(&runner.game, players, &runner.event_log).unwrap_or_else(
        |failure| {
            let artifact = runner
                .write_failure_artifact(&failure)
                .map(|p| {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string());
                    format!("/dev/selfplay?replay={name}")
                })
                .unwrap_or_else(|e| format!("artifact write failed: {e}"));
            panic!(
                "self-play replay failed: {}; REPLAY={artifact}",
                failure.reason
            );
        },
    );
    if let Err(err) = runner.write_success_artifact_if_requested() {
        panic!("failed to save self-play replay artifact: {err}");
    }
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
            | kinds::CITY_CENTRE
            | kinds::DEPOT
            | kinds::BARRACKS
            | kinds::TRAINING_CENTRE
            | kinds::FACTORY
            | kinds::STEELWORKS
            | kinds::STEEL
            | kinds::OIL
    )
}

fn is_complete(entity: &EntityView) -> bool {
    entity.build_progress.is_none()
}

fn assign_steel_workers(
    observation: &AiObservation,
    actions: &mut AiActionContext<'_>,
    initial_gather_sent: bool,
) {
    let has_steel = observation
        .resources
        .iter()
        .any(|node| node.kind == EntityKind::Steel && node.remaining > 0);
    if !has_steel {
        return;
    }
    let latched_nodes: BTreeSet<u32> = observation
        .owned
        .iter()
        .filter_map(|worker| worker.latched_node)
        .collect();
    let skipped_workers = BTreeSet::new();
    actions::assign_workers_to_resource(
        actions,
        ResourceAssignmentPolicy {
            workers: &observation.owned,
            resources: &observation.resources,
            resource_kind: EntityKind::Steel,
            candidate_worker_ids: None,
            skip_workers: &skipped_workers,
            pre_reserved_nodes: &latched_nodes,
            idle_only: initial_gather_sent,
            allow_latched_reassignment: false,
            max_assignments: None,
            max_worker_resource_distance_px: None,
        },
    );
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

fn occupied_tiles_from_snapshot(map: &MapInfo, snapshot: &Snapshot) -> BTreeSet<(u32, u32)> {
    let mut occupied = BTreeSet::new();
    for resource in &map.resources {
        if matches!(resource.kind.as_str(), kinds::STEEL | kinds::OIL) {
            occupied.insert(tile_of(map, resource.x, resource.y));
        }
    }
    for e in &snapshot.entities {
        if e.owner != 0 && kind_of(e).map(|k| k.is_building()).unwrap_or(false) {
            for (tx, ty) in building_footprint_tiles(map, e) {
                let Some(kind) = kind_of(e) else {
                    continue;
                };
                let clearance = ai_shared::building_clearance_tiles(kind);
                for dy in -clearance..=clearance {
                    for dx in -clearance..=clearance {
                        let nx = tx as i32 + dx;
                        let ny = ty as i32 + dy;
                        if nx >= 0 && ny >= 0 && (nx as u32) < map.width && (ny as u32) < map.height
                        {
                            occupied.insert((nx as u32, ny as u32));
                        }
                    }
                }
            }
        } else if e.owner == 0 && (is_kind(e, EntityKind::Steel) || is_kind(e, EntityKind::Oil)) {
            occupied.insert(tile_of(map, e.x, e.y));
        }
    }
    occupied
}

fn footprint_placeable_from_snapshot(
    map: &MapInfo,
    snapshot: &Snapshot,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
    occupied: &BTreeSet<(u32, u32)>,
) -> bool {
    let Some(stats) = config::building_stats(building) else {
        return false;
    };
    for e in &snapshot.entities {
        let Some(existing_kind) = kind_of(e) else {
            continue;
        };
        if e.owner == 0 || !existing_kind.is_building() {
            continue;
        }
        let existing_tile = tile_of(map, e.x, e.y);
        let existing_tile_x = existing_tile.0.saturating_sub(
            config::building_stats(existing_kind)
                .map(|building| building.foot_w / 2)
                .unwrap_or(0),
        );
        let existing_tile_y = existing_tile.1.saturating_sub(
            config::building_stats(existing_kind)
                .map(|building| building.foot_h / 2)
                .unwrap_or(0),
        );
        if !ai_shared::footprints_respect_clearance(
            building,
            tile_x,
            tile_y,
            existing_kind,
            existing_tile_x,
            existing_tile_y,
        ) {
            return false;
        }
    }
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
    if !rules::economy::trainable_units(building).is_empty() {
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

struct MatchupPlayerSpec {
    id: u32,
    name: &'static str,
    color: &'static str,
    profile_id: &'static str,
    goal: PlayerMilestoneGoal,
}

struct MatchupConfig {
    artifact_name: &'static str,
    seed: u32,
    max_ticks: u32,
    players: [MatchupPlayerSpec; 2],
    combat_goal: CombatGoal,
    assert_outcome: fn(&Milestones),
}

fn run_profile_matchup(config: MatchupConfig) {
    let players: Vec<PlayerInit> = config
        .players
        .iter()
        .map(|player| PlayerInit {
            id: player.id,
            name: player.name.to_string(),
            color: player.color.to_string(),
            is_ai: true,
        })
        .collect();
    let game = Game::new_without_ai_controllers(&players, config.seed);
    let start = game.start_payload();
    let specs = players.clone();
    let scripts: Vec<Box<dyn ScriptedPlayer>> = config
        .players
        .iter()
        .map(|player| {
            Box::new(ProfileBackedScript::new(player.id, player.profile_id))
                as Box<dyn ScriptedPlayer>
        })
        .collect();
    let milestones = Milestones::with_goals(
        config
            .players
            .iter()
            .map(|player| (player.id, player.goal.clone())),
        config.combat_goal,
    );
    let mut runner = SelfPlayRunner::with_options(
        config.artifact_name,
        config.max_ticks,
        game,
        start,
        specs,
        scripts,
        milestones,
    );

    match runner.run() {
        Ok(report) => {
            (config.assert_outcome)(&runner.milestones);
            finalize_self_play_success(&runner, &players, &report);
        }
        Err(failure) => {
            let artifact = runner
                .write_failure_artifact(&failure)
                .map(|p| {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string());
                    format!("/dev/selfplay?replay={name}")
                })
                .unwrap_or_else(|e| format!("artifact write failed: {e}"));
            panic!("matchup failed: {}; REPLAY={artifact}", failure.reason);
        }
    }
}

fn rifle_flood_fast_goal() -> PlayerMilestoneGoal {
    PlayerMilestoneGoal {
        require_gathering: true,
        require_depot_supply: true,
        require_barracks_complete: true,
        ..PlayerMilestoneGoal::default()
    }
    .with_min_workers(config::STARTING_WORKERS + 1)
    .with_min_supply_cap(config::CITY_CENTRE_SUPPLY + config::DEPOT_SUPPLY)
    .with_min_units(kinds::RIFLEMAN, 1)
    .with_min_attack_command_units(1)
}

fn rifle_flood_full_saturation_goal() -> PlayerMilestoneGoal {
    PlayerMilestoneGoal {
        require_gathering: true,
        require_depot_supply: true,
        require_barracks_complete: true,
        ..PlayerMilestoneGoal::default()
    }
    .with_min_workers(12)
    .with_min_supply_cap(config::CITY_CENTRE_SUPPLY + config::DEPOT_SUPPLY)
    .with_min_units(kinds::RIFLEMAN, 6)
    .with_min_attack_command_units(6)
}

fn rifle_flood_full_saturation_under_proxy_pressure_goal() -> PlayerMilestoneGoal {
    PlayerMilestoneGoal {
        require_gathering: true,
        require_depot_supply: true,
        require_barracks_complete: true,
        ..PlayerMilestoneGoal::default()
    }
    .with_min_workers(config::STARTING_WORKERS + 4)
    .with_min_supply_cap(config::CITY_CENTRE_SUPPLY + config::DEPOT_SUPPLY)
    .with_min_units(kinds::RIFLEMAN, 1)
    .with_min_attack_command_units(1)
}

fn tech_to_tanks_goal() -> PlayerMilestoneGoal {
    PlayerMilestoneGoal {
        require_gathering: true,
        require_oil: true,
        require_oil_worker_assignment: true,
        require_depot_supply: true,
        require_barracks_complete: true,
        require_tank: true,
        ..PlayerMilestoneGoal::default()
    }
    .with_min_workers(8)
    .with_min_supply_cap(config::CITY_CENTRE_SUPPLY + config::DEPOT_SUPPLY)
    .with_min_buildings(kinds::TRAINING_CENTRE, 1)
    .with_min_buildings(kinds::FACTORY, 1)
    .with_min_buildings(kinds::STEELWORKS, 1)
    .with_min_units(kinds::TANK, 1)
}

fn tech_to_tanks_under_pressure_goal() -> PlayerMilestoneGoal {
    PlayerMilestoneGoal {
        require_gathering: true,
        require_oil: true,
        require_oil_worker_assignment: true,
        require_depot_supply: true,
        require_barracks_complete: true,
        ..PlayerMilestoneGoal::default()
    }
    .with_min_workers(8)
    .with_min_supply_cap(config::CITY_CENTRE_SUPPLY + config::DEPOT_SUPPLY)
    .with_min_buildings(kinds::TRAINING_CENTRE, 1)
    .with_min_buildings(kinds::FACTORY, 1)
    .with_min_buildings(kinds::STEELWORKS, 1)
    .allowing_elimination_before_milestones()
}

fn player_milestones(milestones: &Milestones, player_id: u32) -> &PlayerMilestones {
    milestones
        .players
        .get(&player_id)
        .unwrap_or_else(|| panic!("missing milestones for player {player_id}"))
}

fn assert_fast_pressures_before_full_saturation(milestones: &Milestones) {
    let fast = player_milestones(milestones, 1);
    let full = player_milestones(milestones, 2);
    let fast_attack = fast
        .first_goal_attack_command_tick
        .expect("fast flood did not issue a meaningful attack command");
    let full_attack = full
        .first_goal_attack_command_tick
        .expect("full saturation did not issue a meaningful attack command");

    assert!(
        fast_attack < full_attack,
        "fast flood should attack earlier than full saturation: fast={fast_attack} full={full_attack}"
    );
    assert!(
        full.max_workers > fast.max_workers,
        "full saturation should reach a stronger economy: full workers={} fast workers={}",
        full.max_workers,
        fast.max_workers
    );
}

fn assert_fast_pressures_before_first_tank(milestones: &Milestones) {
    let fast = player_milestones(milestones, 1);
    let tech = player_milestones(milestones, 2);
    let fast_attack = fast
        .first_goal_attack_command_tick
        .expect("fast flood did not issue a meaningful attack command");

    if let Some(first_tank) = tech.first_tank_tick {
        assert!(
            fast_attack < first_tank,
            "fast flood should attack before the first tank: attack={fast_attack} tank={first_tank}"
        );
    }
    if !tech.eliminated {
        assert!(
            tech.oil_worker_assigned,
            "tech_to_tanks should assign at least one worker to oil when it survives the fast rush"
        );
    }
}

fn assert_macro_rifles_and_tanks_both_function(milestones: &Milestones) {
    let full = player_milestones(milestones, 1);
    let tech = player_milestones(milestones, 2);

    assert!(
        full.max_units_by_kind
            .get(kinds::RIFLEMAN)
            .copied()
            .unwrap_or_default()
            >= 6,
        "full saturation should reach strong rifle production"
    );
    assert!(
        tech.first_tank_tick.is_some(),
        "tech_to_tanks should reach tank production"
    );
    assert!(
        milestones.first_damage_tick.is_some(),
        "macro-vs-tech matchup should produce combat damage"
    );
}

#[test]
fn profile_matchup_rifle_flood_fast_vs_full_saturation() {
    run_profile_matchup(MatchupConfig {
        artifact_name: "profile_matchup_rifle_flood_fast_vs_full_saturation",
        seed: 0x1234_5678,
        max_ticks: MAX_TICKS,
        players: [
            MatchupPlayerSpec {
                id: 1,
                name: "Fast Flood",
                color: "#4cc9f0",
                profile_id: RIFLE_FLOOD_FAST_ID,
                goal: rifle_flood_fast_goal(),
            },
            MatchupPlayerSpec {
                id: 2,
                name: "Full Saturation",
                color: "#f72585",
                profile_id: RIFLE_FLOOD_FULL_SATURATION_ID,
                goal: rifle_flood_full_saturation_under_proxy_pressure_goal(),
            },
        ],
        combat_goal: CombatGoal::damage(),
        assert_outcome: assert_fast_pressures_before_full_saturation,
    });
}

#[test]
fn profile_matchup_rifle_flood_fast_vs_tech_to_tanks() {
    let factory_build_ticks = config::building_stats(EntityKind::Factory)
        .expect("factory stats should exist")
        .build_ticks;
    run_profile_matchup(MatchupConfig {
        artifact_name: "profile_matchup_rifle_flood_fast_vs_tech_to_tanks",
        seed: 0,
        // LOS-aware fights delay the tech player's factory start under pressure, but the
        // strategy still commits the factory before the old harness limit. Let the already
        // issued build complete so the milestone observes the intended tech transition.
        max_ticks: MAX_TICKS + factory_build_ticks,
        players: [
            MatchupPlayerSpec {
                id: 1,
                name: "Fast Flood",
                color: "#4cc9f0",
                profile_id: RIFLE_FLOOD_FAST_ID,
                goal: rifle_flood_fast_goal(),
            },
            MatchupPlayerSpec {
                id: 2,
                name: "Tech Tanks",
                color: "#f72585",
                profile_id: TECH_TO_TANKS_ID,
                goal: tech_to_tanks_under_pressure_goal(),
            },
        ],
        combat_goal: CombatGoal::damage(),
        assert_outcome: assert_fast_pressures_before_first_tank,
    });
}

#[test]
fn profile_matchup_rifle_flood_full_saturation_vs_tech_to_tanks() {
    run_profile_matchup(MatchupConfig {
        artifact_name: "profile_matchup_rifle_flood_full_saturation_vs_tech_to_tanks",
        seed: 0,
        max_ticks: MAX_TICKS,
        players: [
            MatchupPlayerSpec {
                id: 1,
                name: "Full Saturation",
                color: "#4cc9f0",
                profile_id: RIFLE_FLOOD_FULL_SATURATION_ID,
                goal: rifle_flood_full_saturation_goal(),
            },
            MatchupPlayerSpec {
                id: 2,
                name: "Tech Tanks",
                color: "#f72585",
                profile_id: TECH_TO_TANKS_ID,
                goal: tech_to_tanks_goal(),
            },
        ],
        combat_goal: CombatGoal::damage(),
        assert_outcome: assert_macro_rifles_and_tanks_both_function,
    });
}

/// Manual long-form matchup runner for inspecting the full result instead of stopping as soon as
/// milestone coverage is complete.
#[test]
#[ignore]
fn profile_matchup_rifle_flood_full_saturation_vs_tech_to_tanks_20k_result() {
    const TICKS: u32 = 20_000;
    const ARTIFACT_NAME: &str = "profile_full_saturation_vs_tech_to_tanks_20k";

    let players = vec![
        PlayerInit {
            id: 1,
            name: "Full Saturation".into(),
            color: "#4cc9f0".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            name: "Tech Tanks".into(),
            color: "#f72585".into(),
            is_ai: true,
        },
    ];
    let mut game = Game::new_without_ai_controllers(&players, 0);
    let start = game.start_payload();
    let mut scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(ProfileBackedScript::new(1, RIFLE_FLOOD_FULL_SATURATION_ID)),
        Box::new(ProfileBackedScript::new(2, TECH_TO_TANKS_ID)),
    ];
    let mut event_log = Vec::new();

    while game.tick_count() < TICKS {
        let alive = game.alive_players();
        if alive.len() <= 1 {
            break;
        }

        let tick = game.tick_count();
        let mut commands = Vec::new();
        for script in &mut scripts {
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

        let tick_events = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.tick()))
            .expect("Game::tick panicked during 20k profile matchup");
        let event_tick = game.tick_count();
        for (player_id, events) in tick_events {
            for event in events {
                event_log.push(EventLogEntry {
                    tick: event_tick,
                    player_id,
                    event,
                });
            }
        }
    }

    assert_replay_matches_live(&game, &players, &event_log).unwrap_or_else(|failure| {
        panic!(
            "20k profile matchup replay determinism failed: {}",
            failure.reason
        );
    });

    let artifact = ReplayArtifact {
        replay_commands: game.command_log().to_vec(),
        players: players.clone(),
        seed: game.seed(),
        starting_steel: game.starting_steel(),
        starting_oil: game.starting_oil(),
    };
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("selfplay-artifacts")
        .join(ARTIFACT_NAME);
    fs::create_dir_all(&dir).unwrap();
    let json = serde_json::to_vec_pretty(&artifact).unwrap();
    fs::write(dir.join("replay.json"), json).unwrap();

    let alive = game.alive_players();
    let winner = if alive.len() == 1 {
        Some(alive[0])
    } else {
        None
    };
    let final_counts = final_unit_counts(&game, &players);
    println!(
        "SIM_RESULT ticks={} winner={:?} alive={:?} artifact={} counts={:?}",
        game.tick_count(),
        winner,
        alive,
        ARTIFACT_NAME,
        final_counts
    );
}

fn final_unit_counts(game: &Game, players: &[PlayerInit]) -> BTreeMap<u32, BTreeMap<String, u32>> {
    let viewer = players.first().map(|p| p.id).unwrap_or(0);
    let snapshot = game.snapshot_full_for(viewer);
    let player_ids: BTreeSet<u32> = players.iter().map(|p| p.id).collect();
    let mut counts: BTreeMap<u32, BTreeMap<String, u32>> = BTreeMap::new();
    for entity in snapshot
        .entities
        .iter()
        .filter(|entity| player_ids.contains(&entity.owner))
    {
        *counts
            .entry(entity.owner)
            .or_default()
            .entry(entity.kind.clone())
            .or_default() += 1;
    }
    counts
}

#[test]
fn profile_backed_self_play_exercises_tech_to_tanks() {
    let players = vec![
        PlayerInit {
            id: 1,
            name: "Tank Profile A".into(),
            color: "#4cc9f0".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            name: "Tank Profile B".into(),
            color: "#f72585".into(),
            is_ai: true,
        },
    ];
    let game = Game::new_without_ai_controllers(&players, 0x1234_5678);
    let start = game.start_payload();
    let specs = players.clone();
    let scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(ProfileBackedScript::new(1, TECH_TO_TANKS_ID)),
        Box::new(ProfileBackedScript::new(2, TECH_TO_TANKS_ID)),
    ];
    let mut runner = SelfPlayRunner::new(
        "profile_backed_self_play_exercises_tech_to_tanks",
        game,
        start,
        specs,
        scripts,
    );

    match runner.run() {
        Ok(report) => finalize_self_play_success(&runner, &players, &report),
        Err(failure) => {
            let artifact = runner
                .write_failure_artifact(&failure)
                .map(|p| {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string());
                    format!("/dev/selfplay?replay={name}")
                })
                .unwrap_or_else(|e| format!("artifact write failed: {e}"));
            panic!("self-play failed: {}; REPLAY={artifact}", failure.reason);
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
            is_ai: true,
        },
    ];
    let game = Game::new_without_ai_controllers(&players, 0x1234_5678);
    let start = game.start_payload();
    let specs = players.clone();
    let scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(WorkerRushScript::new(1, 2)),
        Box::new(ProfileBackedScript::economy_only(2)),
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
        Ok(report) => finalize_self_play_success(&runner, &players, &report),
        Err(failure) => {
            let artifact = runner
                .write_failure_artifact(&failure)
                .map(|p| {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string());
                    format!("/dev/selfplay?replay={name}")
                })
                .unwrap_or_else(|e| format!("artifact write failed: {e}"));
            panic!("self-play failed: {}; REPLAY={artifact}", failure.reason);
        }
    }
}

/// A scripted player that does nothing but send idle workers to mine the nearest steel node.
/// No building, no training, no combat — pure passive mining.
///
/// Intentionally retained as passive/minimal harness coverage. This is not a strategy profile.
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
        tick == 0
            || tick
                .wrapping_add(self.player_id)
                .is_multiple_of(THINK_INTERVAL)
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

        let Some(observation) = view.observation([]) else {
            return Vec::new();
        };
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(
            &facts,
            SpendBudget::new(
                view.snapshot.steel,
                view.snapshot.oil,
                view.snapshot.supply_used,
                view.snapshot.supply_cap,
            ),
        );
        assign_steel_workers(&observation, &mut actions, self.initial_gather_sent);
        self.initial_gather_sent = true;
        actions.into_commands()
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
    let mut game = Game::new(&players, 0x1234_5678);
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

#[cfg(test)]
fn pending_tracker_start_payload() -> StartPayload {
    StartPayload {
        player_id: 1,
        tick: 0,
        map: MapInfo {
            width: 96,
            height: 96,
            tile_size: config::TILE_SIZE,
            terrain: vec![terrain::GRASS; 96 * 96],
            resources: Vec::new(),
        },
        players: vec![PlayerStart {
            id: 1,
            name: "Alpha".into(),
            color: "#4cc9f0".into(),
            start_tile_x: 10,
            start_tile_y: 85,
        }],
    }
}

#[cfg(test)]
fn pending_tracker_snapshot(tick: u32, worker_x: f32, worker_y: f32) -> Snapshot {
    Snapshot {
        tick,
        steel: 0,
        oil: 0,
        supply_used: 1,
        supply_cap: 10,
        entities: vec![EntityView::new(
            2,
            1,
            kinds::WORKER,
            worker_x,
            worker_y,
            40,
            40,
            states::BUILD,
        )],
        resource_deltas: Vec::new(),
        events: Vec::new(),
        player_resources: Vec::new(),
    }
}

#[cfg(test)]
fn pending_tracker_view<'a>(
    tick: u32,
    start: &'a StartPayload,
    snapshot: &'a Snapshot,
) -> PlayerView<'a> {
    PlayerView {
        player_id: 1,
        tick,
        start,
        snapshot,
    }
}

#[test]
fn pending_build_tracker_keeps_moving_worker_past_stale_window() {
    let start = pending_tracker_start_payload();
    let mut tracker = PendingBuildTracker::default();
    tracker.record_commands(
        10,
        &[Command::Build {
            worker: 2,
            building: EntityKind::CityCentre,
            tile_x: 48,
            tile_y: 70,
        }],
    );

    for tick in [70, 130, 190, 250, 310, 370, 430] {
        let snapshot = pending_tracker_snapshot(tick, 100.0 + tick as f32, 200.0);
        tracker.observe(pending_tracker_view(tick, &start, &snapshot));
        assert_eq!(
            tracker.intents().len(),
            1,
            "moving expansion builder should remain reserved at tick {tick}"
        );
    }
}

#[test]
fn pending_build_tracker_expires_stuck_worker() {
    let start = pending_tracker_start_payload();
    let mut tracker = PendingBuildTracker::default();
    tracker.record_commands(
        10,
        &[Command::Build {
            worker: 2,
            building: EntityKind::CityCentre,
            tile_x: 48,
            tile_y: 70,
        }],
    );

    let first_snapshot = pending_tracker_snapshot(20, 100.0, 200.0);
    tracker.observe(pending_tracker_view(20, &start, &first_snapshot));
    let stale_tick = 20 + PENDING_BUILD_STALE_TICKS;
    let stale_snapshot = pending_tracker_snapshot(stale_tick, 100.0, 200.0);
    tracker.observe(pending_tracker_view(stale_tick, &start, &stale_snapshot));

    assert!(tracker.intents().is_empty());
    assert!(tracker.failed(EntityKind::CityCentre, 48, 70));
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
    let mut game_a = Game::new(&players, 0x1234_5678);
    let start = game_a.start_payload();
    let mut scripts_a: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(MineOnlyScript::new(1)),
        Box::new(MineOnlyScript::new(2)),
    ];
    let history_a = run_scripted_ticks(&players, &mut scripts_a, &start, &mut game_a, TICKS);

    let mut game_b = Game::new(&players, 0x1234_5678);
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

/// Two real AI opponents (AiController vs AiController) fight it out. Produces a
/// deterministic command log and writes a replay artifact to
/// `target/selfplay-artifacts/real_ai_vs_real_ai/replay.json`.
#[test]
fn real_ai_vs_real_ai() {
    use std::collections::{BTreeMap, BTreeSet};

    const MIN_PEAK_BARRACKS_ALIVE: usize = 3;
    const MIN_RIFLEMAN_TRAIN_COMMANDS: usize = 25;
    const MIN_ATTACK_MOVE_COMMANDS: usize = 13;
    const MIN_ATTACK_EVENTS: usize = 200;
    const TICKS: u32 = 13_824;

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
    let mut game = Game::new(&players, 0x1234_5678);

    let mut event_log = Vec::new();
    let mut max_barracks_alive: BTreeMap<u32, usize> = BTreeMap::new();
    let mut max_riflemen_alive: BTreeMap<u32, usize> = BTreeMap::new();
    let mut seen_riflemen: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut attack_events: BTreeMap<u32, usize> = BTreeMap::new();
    let mut death_events: BTreeMap<u32, usize> = BTreeMap::new();
    let panic_reason = |payload: &Box<dyn std::any::Any + Send>| -> String {
        if let Some(s) = payload.downcast_ref::<&'static str>() {
            s.to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "panic without string payload".to_string()
        }
    };
    let save_failure_artifact = |game: &Game, reason: &str| -> String {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let artifact_name = format!("real_ai_vs_real_ai_failure_{ts}");
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("selfplay-artifacts")
            .join(&artifact_name);
        if std::fs::create_dir_all(&dir).is_ok() {
            let artifact = ReplayArtifact {
                replay_commands: game.command_log().to_vec(),
                players: players.clone(),
                seed: game.seed(),
                starting_steel: game.starting_steel(),
                starting_oil: game.starting_oil(),
            };
            if let Ok(json) = serde_json::to_vec_pretty(&artifact) {
                let _ = std::fs::write(dir.join("replay.json"), json);
            }
        }
        let url = format!("/dev/selfplay?replay={artifact_name}");
        println!("REPLAY_ARTIFACT={artifact_name}");
        eprintln!("real_ai_vs_real_ai failure: {reason}");
        eprintln!("view replay: {url}");
        url
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for tick in 1..=TICKS {
            let tick_result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.tick()));
            let tick_output = match tick_result {
                Ok(events) => events,
                Err(_) => {
                    let url = save_failure_artifact(&game, "Game::tick panicked");
                    panic!("real_ai_vs_real_ai: tick {tick} panicked; view replay: {url}");
                }
            };
            for (player_id, events) in tick_output {
                for event in events {
                    match event {
                        Event::Attack { .. } => {
                            *attack_events.entry(player_id).or_default() += 1;
                        }
                        Event::Death { .. } => {
                            *death_events.entry(player_id).or_default() += 1;
                        }
                        _ => {}
                    }
                    event_log.push(EventLogEntry {
                        tick,
                        player_id,
                        event,
                    });
                }
            }

            for player in &players {
                let snapshot = game.snapshot_for(player.id);
                let mut barracks_alive = 0usize;
                let mut riflemen_alive = 0usize;
                let seen = seen_riflemen.entry(player.id).or_default();
                for entity in snapshot.entities.iter().filter(|e| e.owner == player.id) {
                    if entity.kind == kinds::BARRACKS {
                        barracks_alive += 1;
                    }
                    if entity.kind == kinds::RIFLEMAN {
                        riflemen_alive += 1;
                        seen.insert(entity.id);
                    }
                }
                max_barracks_alive
                    .entry(player.id)
                    .and_modify(|max| *max = (*max).max(barracks_alive))
                    .or_insert(barracks_alive);
                max_riflemen_alive
                    .entry(player.id)
                    .and_modify(|max| *max = (*max).max(riflemen_alive))
                    .or_insert(riflemen_alive);
            }
        }

        let mut barracks_build_cmds: BTreeMap<u32, usize> = BTreeMap::new();
        let mut rifleman_train_cmds: BTreeMap<u32, usize> = BTreeMap::new();
        let mut attack_move_cmds: BTreeMap<u32, usize> = BTreeMap::new();
        for entry in game.command_log() {
            match &entry.command {
                WireCommand::Build { building, .. } if building == kinds::BARRACKS => {
                    *barracks_build_cmds.entry(entry.player_id).or_default() += 1;
                }
                WireCommand::Train { unit, .. } if unit == kinds::RIFLEMAN => {
                    *rifleman_train_cmds.entry(entry.player_id).or_default() += 1;
                }
                WireCommand::AttackMove { .. } => {
                    *attack_move_cmds.entry(entry.player_id).or_default() += 1;
                }
                _ => {}
            }
        }

        for player in &players {
            let peak_barracks = max_barracks_alive
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let rifleman_trains = rifleman_train_cmds
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let attack_moves = attack_move_cmds
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let attacks = attack_events.get(&player.id).copied().unwrap_or_default();
            let seen_riflemen = seen_riflemen
                .get(&player.id)
                .map(|ids| ids.len())
                .unwrap_or_default();
            let peak_riflemen = max_riflemen_alive
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let barracks_builds = barracks_build_cmds
                .get(&player.id)
                .copied()
                .unwrap_or_default();

            assert!(
                peak_barracks >= MIN_PEAK_BARRACKS_ALIVE,
                "player {} peaked at only {} live barracks (build cmds {}, train cmds {}, peak riflemen {}, seen riflemen {}, attack moves {}, attack events {})",
                player.id,
                peak_barracks,
                barracks_builds,
                rifleman_trains,
                peak_riflemen,
                seen_riflemen,
                attack_moves,
                attacks,
            );
            assert!(
                rifleman_trains >= MIN_RIFLEMAN_TRAIN_COMMANDS,
                "player {} trained only {} riflemen (peak barracks {}, peak riflemen {}, seen riflemen {}, attack moves {}, attack events {})",
                player.id,
                rifleman_trains,
                peak_barracks,
                peak_riflemen,
                seen_riflemen,
                attack_moves,
                attacks,
            );
            assert!(
                attack_moves >= MIN_ATTACK_MOVE_COMMANDS,
                "player {} issued only {} attack-move commands (peak barracks {}, rifleman train cmds {}, peak riflemen {}, attack events {})",
                player.id,
                attack_moves,
                peak_barracks,
                rifleman_trains,
                peak_riflemen,
                attacks,
            );
            assert!(
                attacks >= MIN_ATTACK_EVENTS,
                "player {} produced only {} attack events (peak barracks {}, rifleman train cmds {}, attack moves {}, peak riflemen {}, seen riflemen {}, deaths {})",
                player.id,
                attacks,
                peak_barracks,
                rifleman_trains,
                attack_moves,
                peak_riflemen,
                seen_riflemen,
                death_events.get(&player.id).copied().unwrap_or_default(),
            );
        }

        assert_replay_matches_live(&game, &players, &event_log).unwrap_or_else(|failure| {
            panic!("AI vs AI replay determinism failed: {}", failure.reason);
        });
    }));

    if let Err(payload) = result {
        let reason = panic_reason(&payload);
        let url = save_failure_artifact(&game, &reason);
        panic!("real_ai_vs_real_ai failed; view replay: {url}");
    }

    // Write a replay artifact so the dev self-play viewer can load it.
    let artifact = ReplayArtifact {
        replay_commands: game.command_log().to_vec(),
        players: players.clone(),
        seed: game.seed(),
        starting_steel: game.starting_steel(),
        starting_oil: game.starting_oil(),
    };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let artifact_name = format!("real_ai_vs_real_ai_{ts}");
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("selfplay-artifacts")
        .join(&artifact_name);
    std::fs::create_dir_all(&dir).unwrap();
    let json = serde_json::to_vec_pretty(&artifact).unwrap();
    std::fs::write(dir.join("replay.json"), json).unwrap();
    println!("REPLAY_ARTIFACT={artifact_name}");
}
