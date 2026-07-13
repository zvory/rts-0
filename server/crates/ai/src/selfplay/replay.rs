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
use crate::ai_core::profiles::{profile_by_id, required_profiles};
use crate::live::DEFAULT_LIVE_PROFILE_ID;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::replay::{
    replay_commands, CommandLogEntry, EventLogEntry, PlayerSnapshot, ReplayOutcome,
    ReplayStartComposition,
};
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{kinds, Command as WireCommand, Event, Snapshot, StartPayload};

const PROFILE_MATCHUP_TRACE_TAIL: usize = 24;

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
    git_output(
        env!("CARGO_MANIFEST_DIR"),
        &["rev-parse", "--short=12", "HEAD"],
    )
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
    pub end_reason: ProfileMatchupEndReason,
    pub winner: Option<ProfileMatchupWinner>,
    pub starting_city_centres: Vec<ProfileMatchupStartingCityCentreResult>,
    pub players: Vec<ProfileMatchupPlayerResult>,
    pub first_damage_tick: Option<u32>,
    pub attack_events: usize,
    pub death_events: usize,
    pub event_count: usize,
    pub replay_verified: bool,
    pub replay_artifact: Option<String>,
    pub ai_trace_tail: Vec<ProfileMatchupTraceEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMatchupEndReason {
    StartingCityCentreKilled,
    StartingCityCentresDestroyed,
    TickCap,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMatchupWinner {
    pub player_id: u32,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMatchupStartingCityCentreResult {
    pub player_id: u32,
    pub profile: String,
    pub entity_id: u32,
    pub alive: bool,
    pub death_tick: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMatchupPlayerResult {
    pub player_id: u32,
    pub profile: String,
    pub alive: bool,
    pub army_value: u32,
    pub building_value: u32,
    pub worker_count: u32,
    pub command_count: usize,
    pub attack_command_count: usize,
    pub damage_dealt_events: usize,
    pub death_count: usize,
    pub first_attack_command_tick: Option<u32>,
    pub first_rifleman_attack_command_tick: Option<u32>,
    pub first_scout_car_tick: Option<u32>,
    pub first_scout_car_harass_command_tick: Option<u32>,
    pub first_expansion_city_centre_planned_tick: Option<u32>,
    pub first_expansion_city_centre_completed_tick: Option<u32>,
    pub first_tank_tick: Option<u32>,
    pub final_counts: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMatchupTraceEntry {
    pub tick: u32,
    pub player_id: u32,
    pub profile: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone)]
struct StartingCityCentreObjective {
    centres: Vec<StartingCityCentreState>,
}

#[derive(Debug, Clone)]
struct StartingCityCentreState {
    player_id: u32,
    profile: String,
    entity_id: u32,
    death_tick: Option<u32>,
}

impl StartingCityCentreObjective {
    fn capture(game: &Game, start: &StartPayload, players: &[PlayerInit]) -> Result<Self, String> {
        let viewer = players
            .first()
            .map(|player| player.id)
            .ok_or_else(|| "profile matchup requires at least one player".to_string())?;
        let snapshot = game.snapshot_full_for(viewer);
        let mut centres = Vec::with_capacity(players.len());
        for player in players {
            let start_player = start
                .players
                .iter()
                .find(|start_player| start_player.id == player.id)
                .ok_or_else(|| format!("missing start payload row for player {}", player.id))?;
            let tile_size = start.map.tile_size as f32;
            let start_x = start_player.start_tile_x as f32 * tile_size + tile_size * 0.5;
            let start_y = start_player.start_tile_y as f32 * tile_size + tile_size * 0.5;
            let centre = snapshot
                .entities
                .iter()
                .filter(|entity| {
                    entity.owner == player.id
                        && entity.kind == kinds::CITY_CENTRE
                        && entity.build_progress.is_none()
                        && entity.hp > 0
                })
                .min_by(|a, b| {
                    distance_sq(a.x, a.y, start_x, start_y)
                        .total_cmp(&distance_sq(b.x, b.y, start_x, start_y))
                })
                .ok_or_else(|| {
                    format!(
                        "missing completed starting City Centre for player {}",
                        player.id
                    )
                })?;
            centres.push(StartingCityCentreState {
                player_id: player.id,
                profile: player.name.clone(),
                entity_id: centre.id,
                death_tick: None,
            });
        }
        Ok(Self { centres })
    }

    fn alive_player_ids(&self) -> Vec<u32> {
        self.centres
            .iter()
            .filter(|centre| centre.death_tick.is_none())
            .map(|centre| centre.player_id)
            .collect()
    }

    fn observe_snapshot(&mut self, tick: u32, snapshot: &Snapshot) -> bool {
        let mut changed = false;
        for centre in self
            .centres
            .iter_mut()
            .filter(|centre| centre.death_tick.is_none())
        {
            let alive = snapshot.entities.iter().any(|entity| {
                entity.id == centre.entity_id
                    && entity.owner == centre.player_id
                    && entity.kind == kinds::CITY_CENTRE
                    && entity.hp > 0
            });
            if !alive {
                centre.death_tick = Some(tick);
                changed = true;
            }
        }
        changed
    }

    fn winner(&self) -> Option<ProfileMatchupWinner> {
        let alive = self
            .centres
            .iter()
            .filter(|centre| centre.death_tick.is_none())
            .collect::<Vec<_>>();
        let destroyed = self
            .centres
            .iter()
            .filter(|centre| centre.death_tick.is_some())
            .count();
        if alive.len() == 1 && destroyed == self.centres.len().saturating_sub(1) {
            let centre = alive[0];
            Some(ProfileMatchupWinner {
                player_id: centre.player_id,
                profile: centre.profile.clone(),
            })
        } else {
            None
        }
    }

    fn end_reason(&self) -> ProfileMatchupEndReason {
        let destroyed = self
            .centres
            .iter()
            .filter(|centre| centre.death_tick.is_some())
            .count();
        if destroyed == 0 {
            ProfileMatchupEndReason::TickCap
        } else if destroyed == self.centres.len() {
            ProfileMatchupEndReason::StartingCityCentresDestroyed
        } else {
            ProfileMatchupEndReason::StartingCityCentreKilled
        }
    }

    fn results(&self) -> Vec<ProfileMatchupStartingCityCentreResult> {
        self.centres
            .iter()
            .map(|centre| ProfileMatchupStartingCityCentreResult {
                player_id: centre.player_id,
                profile: centre.profile.clone(),
                entity_id: centre.entity_id,
                alive: centre.death_tick.is_none(),
                death_tick: centre.death_tick,
            })
            .collect()
    }
}

fn distance_sq(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

pub fn available_profile_ids() -> Vec<&'static str> {
    required_profiles()
        .into_iter()
        .map(|profile| profile.id)
        .collect()
}

pub fn canonical_profile_id(input: &str) -> Option<&'static str> {
    match input {
        "ai" | "default" => Some(DEFAULT_LIVE_PROFILE_ID),
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
            faction_id: "kriegsia".to_string(),
            name: profile_a.id.to_string(),
            color: "#4cc9f0".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: profile_b.id.to_string(),
            color: "#f72585".to_string(),
            is_ai: true,
        },
    ];
    let mut game = Game::new_without_ai_controllers(&players, options.seed);
    let replay_start = ReplayStartComposition::capture(&game, server_build_sha())?;
    let start = game.start_payload();
    let mut objective = StartingCityCentreObjective::capture(&game, &start, &players)?;
    let mut scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(ProfileBackedScript::new(1, profile_a.id)),
        Box::new(ProfileBackedScript::new(2, profile_b.id)),
    ];
    let mut event_log = Vec::new();
    let mut first_damage_tick = None;
    let mut attack_events = 0usize;
    let mut death_events = 0usize;
    let mut scorecard = ScorecardCollector::default();
    let mut ai_trace_tail = Vec::new();

    while game.tick_count() < options.max_ticks {
        let objective_alive = objective.alive_player_ids();
        let tick = game.tick_count();
        let mut commands = Vec::new();
        for script in &mut scripts {
            let player_id = script.player_id();
            let snapshot = game.snapshot_for(player_id);
            scorecard.observe_snapshot(tick, player_id, &snapshot);
            let view = PlayerView {
                player_id,
                tick,
                start: &start,
                snapshot: &snapshot,
                alive_player_ids: &objective_alive,
            };
            for command in script.commands(view) {
                commands.push((player_id, command));
            }
            if let Some(lines) = script.last_trace_lines() {
                ai_trace_tail.push(ProfileMatchupTraceEntry {
                    tick,
                    player_id,
                    profile: script.name().to_string(),
                    lines: lines.to_vec(),
                });
                if ai_trace_tail.len() > PROFILE_MATCHUP_TRACE_TAIL {
                    ai_trace_tail.remove(0);
                }
            }
        }
        for (player_id, command) in commands {
            scorecard.observe_command(tick, player_id, &command, &game.snapshot_for(player_id));
            game.enqueue(player_id, command);
        }

        scorecard.observe_full_snapshot(&game.snapshot_full_for(players[0].id));
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
            scorecard.observe_snapshot(event_tick, player.id, &snapshot);
        }
        let full_snapshot = game.snapshot_full_for(players[0].id);
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
                scorecard.observe_event(&event, &full_snapshot);
                event_log.push(EventLogEntry {
                    tick: event_tick,
                    player_id,
                    event,
                });
            }
        }
        if objective.observe_snapshot(event_tick, &full_snapshot) {
            break;
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
            write_replay_artifact(name, options.replay_dir.as_ref(), &replay_start, &game)?
                .display()
                .to_string(),
        ),
        None => None,
    };

    let alive = objective.alive_player_ids();
    let winner = objective.winner();
    let end_reason = objective.end_reason();
    let starting_city_centres = objective.results();
    let final_counts = final_unit_counts(&game, &players);
    let final_values = final_material_values(&game, &players);
    let command_stats = command_stats_by_player(game.command_log());
    let final_worker_counts = final_counts
        .iter()
        .map(|(player_id, counts)| {
            (
                *player_id,
                counts.get(kinds::WORKER).copied().unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let players = players
        .iter()
        .map(|player| {
            let stats = command_stats.get(&player.id);
            let values = final_values.get(&player.id).copied().unwrap_or_default();
            let score = scorecard
                .players
                .get(&player.id)
                .cloned()
                .unwrap_or_default();
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
                worker_count: final_worker_counts
                    .get(&player.id)
                    .copied()
                    .unwrap_or_default(),
                command_count: stats.map(|s| s.command_count).unwrap_or_default(),
                attack_command_count: stats.map(|s| s.attack_command_count).unwrap_or_default(),
                damage_dealt_events: score.damage_dealt_events,
                death_count: score.death_count,
                first_attack_command_tick: stats.and_then(|s| s.first_attack_command_tick),
                first_rifleman_attack_command_tick: score.first_rifleman_attack_command_tick,
                first_scout_car_tick: score.first_scout_car_tick,
                first_scout_car_harass_command_tick: score.first_scout_car_harass_command_tick,
                first_expansion_city_centre_planned_tick: score
                    .first_expansion_city_centre_planned_tick,
                first_expansion_city_centre_completed_tick: score
                    .first_expansion_city_centre_completed_tick,
                first_tank_tick: score.first_tank_tick,
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
        end_reason,
        winner,
        starting_city_centres,
        players,
        first_damage_tick,
        attack_events,
        death_events,
        event_count: event_log.len(),
        replay_verified,
        replay_artifact,
        ai_trace_tail,
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
            | WireCommand::SetupAntiTankGuns { .. }
            | WireCommand::TearDownAntiTankGuns { .. }
            | WireCommand::Charge { .. }
            | WireCommand::UseAbility { .. }
            | WireCommand::RecastAbility { .. }
            | WireCommand::SetAutocast { .. }
            | WireCommand::Gather { .. }
            | WireCommand::Build { .. }
            | WireCommand::Deconstruct { .. }
            | WireCommand::Train { .. }
            | WireCommand::SetProductionRepeat { .. }
            | WireCommand::Research { .. }
            | WireCommand::Cancel { .. }
            | WireCommand::Stop { .. }
            | WireCommand::HoldPosition { .. }
            | WireCommand::SetRally { .. } => {}
        }
    }
    stats
}

#[derive(Clone, Default)]
struct PlayerScorecard {
    first_rifleman_attack_command_tick: Option<u32>,
    first_scout_car_tick: Option<u32>,
    first_scout_car_harass_command_tick: Option<u32>,
    first_expansion_city_centre_planned_tick: Option<u32>,
    first_expansion_city_centre_completed_tick: Option<u32>,
    first_tank_tick: Option<u32>,
    damage_dealt_events: usize,
    death_count: usize,
}

#[derive(Default)]
struct ScorecardCollector {
    players: BTreeMap<u32, PlayerScorecard>,
    entity_owners: BTreeMap<u32, u32>,
    counted_deaths: BTreeSet<u32>,
}

impl ScorecardCollector {
    fn observe_full_snapshot(&mut self, snapshot: &Snapshot) {
        for entity in &snapshot.entities {
            self.entity_owners.insert(entity.id, entity.owner);
        }
    }

    fn observe_snapshot(&mut self, tick: u32, player_id: u32, snapshot: &Snapshot) {
        let score = self.players.entry(player_id).or_default();
        let complete_city_centres = snapshot
            .entities
            .iter()
            .filter(|entity| {
                entity.owner == player_id
                    && entity.kind == kinds::CITY_CENTRE
                    && entity.build_progress.is_none()
            })
            .count();
        if complete_city_centres >= 2 {
            score
                .first_expansion_city_centre_completed_tick
                .get_or_insert(tick);
        }
        if snapshot
            .entities
            .iter()
            .any(|entity| entity.owner == player_id && entity.kind == kinds::SCOUT_CAR)
        {
            score.first_scout_car_tick.get_or_insert(tick);
        }
        if snapshot
            .entities
            .iter()
            .any(|entity| entity.owner == player_id && entity.kind == kinds::TANK)
        {
            score.first_tank_tick.get_or_insert(tick);
        }
    }

    fn observe_command(
        &mut self,
        tick: u32,
        player_id: u32,
        command: &rts_sim::game::command::SimCommand,
        snapshot: &Snapshot,
    ) {
        let score = self.players.entry(player_id).or_default();
        if matches!(
            command,
            rts_sim::game::command::SimCommand::Build {
                building: EntityKind::CityCentre,
                ..
            }
        ) && snapshot
            .entities
            .iter()
            .filter(|entity| entity.owner == player_id && entity.kind == kinds::CITY_CENTRE)
            .count()
            >= 1
        {
            score
                .first_expansion_city_centre_planned_tick
                .get_or_insert(tick);
        }

        let Some(units) = command_units(command) else {
            return;
        };
        let unit_kinds = units
            .iter()
            .filter_map(|unit_id| {
                snapshot
                    .entities
                    .iter()
                    .find(|entity| entity.id == *unit_id && entity.owner == player_id)
                    .and_then(|entity| entity.kind.parse::<EntityKind>().ok())
            })
            .collect::<Vec<_>>();
        if is_attack_command(command) && unit_kinds.contains(&EntityKind::Rifleman) {
            score.first_rifleman_attack_command_tick.get_or_insert(tick);
        }
        if is_harass_command(command) && unit_kinds.contains(&EntityKind::ScoutCar) {
            score
                .first_scout_car_harass_command_tick
                .get_or_insert(tick);
        }
    }

    fn observe_event(&mut self, event: &Event, full_snapshot: &Snapshot) {
        match event {
            Event::Attack { from, .. } => {
                if let Some(attacker) = full_snapshot
                    .entities
                    .iter()
                    .find(|entity| entity.id == *from)
                {
                    self.players
                        .entry(attacker.owner)
                        .or_default()
                        .damage_dealt_events += 1;
                }
            }
            Event::Death { id, .. } if self.counted_deaths.insert(*id) => {
                if let Some(owner) = self.entity_owners.get(id).copied() {
                    self.players.entry(owner).or_default().death_count += 1;
                }
            }
            _ => {}
        }
    }
}

fn command_units(command: &rts_sim::game::command::SimCommand) -> Option<&[u32]> {
    match command {
        rts_sim::game::command::SimCommand::Move { units, .. }
        | rts_sim::game::command::SimCommand::AttackMove { units, .. }
        | rts_sim::game::command::SimCommand::Attack { units, .. }
        | rts_sim::game::command::SimCommand::SetupAntiTankGuns { units, .. }
        | rts_sim::game::command::SimCommand::TearDownAntiTankGuns { units }
        | rts_sim::game::command::SimCommand::UseAbility { units, .. }
        | rts_sim::game::command::SimCommand::RecastAbility { units, .. }
        | rts_sim::game::command::SimCommand::SetAutocast { units, .. }
        | rts_sim::game::command::SimCommand::Gather { units, .. }
        | rts_sim::game::command::SimCommand::Stop { units }
        | rts_sim::game::command::SimCommand::HoldPosition { units, .. } => Some(units),
        rts_sim::game::command::SimCommand::Build { units, .. }
        | rts_sim::game::command::SimCommand::Deconstruct { units, .. } => Some(units),
        rts_sim::game::command::SimCommand::Train { .. }
        | rts_sim::game::command::SimCommand::SetProductionRepeat { .. }
        | rts_sim::game::command::SimCommand::Research { .. }
        | rts_sim::game::command::SimCommand::Cancel { .. }
        | rts_sim::game::command::SimCommand::SetRally { .. }
        | rts_sim::game::command::SimCommand::Rejected { .. } => None,
    }
}

fn is_attack_command(command: &rts_sim::game::command::SimCommand) -> bool {
    matches!(
        command,
        rts_sim::game::command::SimCommand::AttackMove { .. }
            | rts_sim::game::command::SimCommand::Attack { .. }
    )
}

fn is_harass_command(command: &rts_sim::game::command::SimCommand) -> bool {
    matches!(command, rts_sim::game::command::SimCommand::Move { .. })
}

fn write_replay_artifact(
    name: &str,
    replay_dir: Option<&PathBuf>,
    replay_start: &ReplayStartComposition,
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
    let artifact = replay_start.finalize(game, None, game.scores());
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
        game.starting_loadouts(),
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

#[cfg(test)]
mod tests {
    use super::{
        available_profile_ids, canonical_profile_id, run_profile_matchup_result,
        ProfileMatchupOptions, ScorecardCollector,
    };
    use crate::ai_core::profiles::{AI_2_1_ID, AI_TURTLE_ID};
    use crate::DEFAULT_LIVE_PROFILE_ID;
    use rts_sim::game::command::SimCommand;
    use rts_sim::game::entity::EntityKind;
    use rts_sim::game::{Game, PlayerInit};
    use rts_sim::protocol::kinds;
    use rts_sim::protocol::{EntityView, Event, Snapshot, SnapshotNetStatus};

    #[test]
    fn canonical_profiles_are_the_only_selectable_profiles() {
        assert_eq!(canonical_profile_id("ai"), Some(DEFAULT_LIVE_PROFILE_ID));
        assert_eq!(
            canonical_profile_id("default"),
            Some(DEFAULT_LIVE_PROFILE_ID)
        );
        assert_eq!(available_profile_ids(), vec![AI_2_1_ID, AI_TURTLE_ID]);
        assert_eq!(canonical_profile_id(AI_2_1_ID), Some(AI_2_1_ID));
        assert_eq!(canonical_profile_id(AI_TURTLE_ID), Some(AI_TURTLE_ID));
        assert_eq!(canonical_profile_id("unsupported_profile"), None);
    }

    #[test]
    fn scorecard_collector_records_phase_one_fields() {
        let mut collector = ScorecardCollector::default();
        let snapshot = snapshot(vec![
            entity(1, 1, kinds::WORKER),
            entity(2, 1, kinds::RIFLEMAN),
            entity(3, 1, kinds::SCOUT_CAR),
            entity(4, 1, kinds::TANK),
            entity(5, 1, kinds::CITY_CENTRE),
            entity(6, 1, kinds::CITY_CENTRE),
        ]);

        collector.observe_snapshot(100, 1, &snapshot);
        collector.observe_command(
            110,
            1,
            &SimCommand::AttackMove {
                units: vec![2],
                x: 500.0,
                y: 500.0,
                queued: false,
            },
            &snapshot,
        );
        collector.observe_command(
            120,
            1,
            &SimCommand::AttackMove {
                units: vec![3],
                x: 600.0,
                y: 600.0,
                queued: false,
            },
            &snapshot,
        );
        collector.observe_command(
            125,
            1,
            &SimCommand::Move {
                units: vec![3],
                x: 600.0,
                y: 600.0,
                queued: false,
            },
            &snapshot,
        );
        collector.observe_command(
            130,
            1,
            &SimCommand::Build {
                units: vec![1],
                building: EntityKind::CityCentre,
                tile_x: 20,
                tile_y: 20,
                queued: false,
            },
            &snapshot,
        );
        collector.observe_full_snapshot(&snapshot);
        collector.observe_event(
            &Event::Attack {
                from: 2,
                to: 99,
                reveal: None,
                to_pos: None,
                weapon_kind: None,
            },
            &snapshot,
        );
        collector.observe_event(
            &Event::Death {
                id: 2,
                x: 0.0,
                y: 0.0,
                kind: kinds::RIFLEMAN.to_string(),
            },
            &snapshot,
        );
        collector.observe_event(
            &Event::Death {
                id: 2,
                x: 0.0,
                y: 0.0,
                kind: kinds::RIFLEMAN.to_string(),
            },
            &snapshot,
        );

        let score = collector.players.get(&1).expect("player scorecard");
        assert_eq!(score.first_scout_car_tick, Some(100));
        assert_eq!(score.first_tank_tick, Some(100));
        assert_eq!(score.first_expansion_city_centre_completed_tick, Some(100));
        assert_eq!(score.first_rifleman_attack_command_tick, Some(110));
        assert_eq!(score.first_scout_car_harass_command_tick, Some(125));
        assert_eq!(score.first_expansion_city_centre_planned_tick, Some(130));
        assert_eq!(score.damage_dealt_events, 1);
        assert_eq!(score.death_count, 1);
    }

    #[test]
    fn profile_matchup_result_includes_ai_trace_tail() {
        let result = run_profile_matchup_result(ProfileMatchupOptions {
            profile_a: AI_2_1_ID.to_string(),
            profile_b: AI_TURTLE_ID.to_string(),
            seed: 7,
            max_ticks: 12,
            verify_replay: false,
            save_replay_name: None,
            replay_dir: None,
        })
        .expect("short profile matchup should run");

        assert!(!result.ai_trace_tail.is_empty());
        assert!(result.ai_trace_tail.len() <= super::PROFILE_MATCHUP_TRACE_TAIL);
        assert_eq!(result.end_reason, super::ProfileMatchupEndReason::TickCap);
        assert!(result.winner.is_none());
        assert_eq!(result.starting_city_centres.len(), 2);
        assert!(result
            .starting_city_centres
            .iter()
            .all(|centre| centre.alive && centre.death_tick.is_none()));
        assert!(result
            .ai_trace_tail
            .iter()
            .any(|entry| entry.lines.iter().any(|line| line.contains("goal=Economy"))));
    }

    #[test]
    fn starting_city_centre_objective_tracks_destroyed_start() {
        let players = vec![
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: AI_2_1_ID.to_string(),
                color: "#4cc9f0".to_string(),
                is_ai: true,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: AI_TURTLE_ID.to_string(),
                color: "#f72585".to_string(),
                is_ai: true,
            },
        ];
        let mut game = Game::new_without_ai_controllers(&players, 7);
        let start = game.start_payload();
        let mut objective = super::StartingCityCentreObjective::capture(&game, &start, &players)
            .expect("starting City Centres should be captured");

        assert_eq!(objective.alive_player_ids(), vec![1, 2]);
        assert!(objective.winner().is_none());
        assert!(objective.results().iter().all(|centre| centre.alive));

        game.eliminate(2);
        let snapshot = game.snapshot_full_for(1);
        assert!(objective.observe_snapshot(42, &snapshot));

        let winner = objective.winner().expect("player 1 should win");
        assert_eq!(winner.player_id, 1);
        assert_eq!(winner.profile, AI_2_1_ID);
        assert_eq!(
            objective.end_reason(),
            super::ProfileMatchupEndReason::StartingCityCentreKilled
        );
        assert_eq!(objective.alive_player_ids(), vec![1]);
        assert!(objective.results().iter().any(|centre| {
            centre.player_id == 2 && !centre.alive && centre.death_tick == Some(42)
        }));
    }

    fn snapshot(entities: Vec<EntityView>) -> Snapshot {
        Snapshot {
            tick: 0,
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
            entities,
            resource_deltas: Vec::new(),
            smokes: Vec::new(),
            ability_objects: Vec::new(),
            trenches: Vec::new(),
            visible_tiles: Vec::new(),
            remembered_buildings: Vec::new(),
            events: Vec::new(),
            upgrades: Vec::new(),
            player_resources: Vec::new(),
            net_status: SnapshotNetStatus::default(),
        }
    }

    fn entity(id: u32, owner: u32, kind: &str) -> EntityView {
        EntityView::new(id, owner, kind, id as f32, 0.0, 100, 100, "idle")
    }
}
