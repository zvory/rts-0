use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use serde::Serialize;

use super::player_view::PlayerView;
use super::scripts::{ProfileBackedScript, ScriptedPlayer};
use super::SELFPLAY_ARTIFACT_DIR;
use crate::ai_core::profiles::{
    profile_by_id, required_profiles, RIFLE_FLOOD_FULL_SATURATION_ID, STEEL_EXPANSION_TANKS_ID,
    TECH_TO_TANKS_ID,
};
use rts_sim::game::replay::{
    replay_commands, CommandLogEntry, EventLogEntry, PlayerSnapshot, ReplayArtifactV1,
    ReplayOutcome,
};
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{kinds, Command as WireCommand, Event, Snapshot};

#[derive(Debug)]
pub struct SelfPlayFailure {
    pub reason: String,
}

impl SelfPlayFailure {
    pub fn new(reason: impl Into<String>) -> Self {
        SelfPlayFailure {
            reason: reason.into(),
        }
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

pub fn server_build_sha() -> &'static str {
    static BUILD_ID: OnceLock<String> = OnceLock::new();
    BUILD_ID.get_or_init(resolve_build_id).as_str()
}

fn resolve_build_id() -> String {
    if let Some(id) = env_build_id() {
        return id;
    }
    git_output(env!("CARGO_MANIFEST_DIR"), &["rev-parse", "--short=12", "HEAD"])
        .unwrap_or_else(|| "unknown".to_string())
}

fn env_build_id() -> Option<String> {
    ["COMMIT_HASH", "RTS_BUILD_SHA", "RTS_BUILD_ID"]
        .iter()
        .filter_map(|key| env::var(key).ok())
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn git_output(current_dir: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(current_dir)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[derive(Debug, Clone)]
pub struct ProfileMatchupOptions {
    pub profile_a: String,
    pub profile_b: String,
    pub seed: u32,
    pub max_ticks: u32,
    pub verify_replay: bool,
    pub save_replay_name: Option<String>,
    pub replay_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMatchupResult {
    pub profile_a: String,
    pub profile_b: String,
    pub seed: u32,
    pub max_ticks: u32,
    pub ticks: u32,
    pub completed_by_elimination: bool,
    pub winner: Option<ProfileMatchupWinner>,
    pub players: Vec<ProfileMatchupPlayerResult>,
    pub first_damage_tick: Option<u32>,
    pub attack_events: usize,
    pub death_events: usize,
    pub event_count: usize,
    pub replay_verified: bool,
    pub replay_artifact: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMatchupWinner {
    pub player_id: u32,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMatchupPlayerResult {
    pub player_id: u32,
    pub profile: String,
    pub alive: bool,
    pub army_value: u32,
    pub building_value: u32,
    pub command_count: usize,
    pub attack_command_count: usize,
    pub first_attack_command_tick: Option<u32>,
    pub first_tank_tick: Option<u32>,
    pub final_counts: BTreeMap<String, u32>,
}

pub fn available_profile_ids() -> Vec<&'static str> {
    required_profiles()
        .into_iter()
        .map(|profile| profile.id)
        .collect()
}

pub fn canonical_profile_id(input: &str) -> Option<&'static str> {
    match input {
        "rush" | "fast" => Some("rifle_flood_fast"),
        "saturation" | "full" | "macro" => Some(RIFLE_FLOOD_FULL_SATURATION_ID),
        "tech" | "tanks" => Some(TECH_TO_TANKS_ID),
        "expand" | "expansion" | "steel" | "steel_tanks" => Some(STEEL_EXPANSION_TANKS_ID),
        id => profile_by_id(id).map(|profile| profile.id),
    }
}

pub fn run_profile_matchup_result(
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
            team_id: 1,
            name: profile_a.id.to_string(),
            color: "#4cc9f0".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
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
                    _ => {}
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
            write_replay_artifact(name, options.replay_dir.as_ref(), &game)?
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
    let final_values = final_material_values(&game, &players);
    let command_stats = command_stats_by_player(game.command_log());
    let players = players
        .iter()
        .map(|player| {
            let stats = command_stats.get(&player.id);
            let values = final_values.get(&player.id).copied().unwrap_or_default();
            ProfileMatchupPlayerResult {
                player_id: player.id,
                profile: if player.id == 1 {
                    profile_a.id.to_string()
                } else {
                    profile_b.id.to_string()
                },
                alive: alive.contains(&player.id),
                army_value: values.army,
                building_value: values.buildings,
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

fn command_stats_by_player(commands: &[CommandLogEntry]) -> BTreeMap<u32, CommandStats> {
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
            | WireCommand::SetupAtGuns { .. }
            | WireCommand::TearDownAtGuns { .. }
            | WireCommand::Charge { .. }
            | WireCommand::UseAbility { .. }
            | WireCommand::SetAutocast { .. }
            | WireCommand::Gather { .. }
            | WireCommand::Build { .. }
            | WireCommand::Train { .. }
            | WireCommand::Research { .. }
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

fn write_replay_artifact(
    name: &str,
    replay_dir: Option<&PathBuf>,
    game: &Game,
) -> Result<PathBuf, String> {
    let dir = replay_dir
        .cloned()
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join(SELFPLAY_ARTIFACT_DIR)
        })
        .join(name);
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let artifact =
        ReplayArtifactV1::capture_from_game(game, server_build_sha(), None, game.scores());
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

pub fn is_safe_artifact_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
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

pub fn assert_replay_matches_live(
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

#[derive(Clone, Copy, Default)]
struct MaterialValues {
    army: u32,
    buildings: u32,
}

fn final_material_values(game: &Game, players: &[PlayerInit]) -> BTreeMap<u32, MaterialValues> {
    let viewer = players.first().map(|p| p.id).unwrap_or(0);
    let snapshot = game.snapshot_full_for(viewer);
    let player_ids: BTreeSet<u32> = players.iter().map(|p| p.id).collect();
    let mut values: BTreeMap<u32, MaterialValues> = BTreeMap::new();
    for entity in snapshot
        .entities
        .iter()
        .filter(|entity| player_ids.contains(&entity.owner))
    {
        let Ok(kind) = entity.kind.parse() else {
            continue;
        };
        let (steel, oil) = rts_rules::economy::cost(kind);
        let value = steel.saturating_add(oil);
        let entry = values.entry(entity.owner).or_default();
        if kind.is_unit() {
            entry.army = entry.army.saturating_add(value);
        } else if kind.is_building() {
            entry.buildings = entry.buildings.saturating_add(value);
        }
    }
    values
}
