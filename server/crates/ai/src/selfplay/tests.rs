use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::milestones::{
    AttackerInfo, CombatGoal, Milestones, PlayerMilestoneGoal, SnapshotSample,
};
use super::pending_build::{PendingBuildTracker, PENDING_BUILD_STALE_TICKS};
use super::player_view::{is_complete, kind_of, PlayerView};
use super::replay::{assert_replay_matches_live, is_safe_artifact_name, SelfPlayFailure};
use super::scripts::{MineOnlyScript, ProfileBackedScript, ScriptedPlayer, WorkerRushScript};
use super::validation::validate_snapshot;
use super::{
    MAX_STALL_TICKS, MAX_TICKS, SAMPLE_EVERY_TICKS, SAVE_REPLAY_ENV, SELFPLAY_ARTIFACT_DIR,
    SELFPLAY_FAILURE_DIR,
};
use crate::ai_core::profiles::AI_1_0_TECH_ID;
use crate::config;
use crate::{AiController, AiThinkContext};
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::replay::{CommandLogEntry, EventLogEntry, ReplayArtifactV1};
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{
    kinds, states, terrain, Command as WireCommand, EntityView, Event, MapInfo, PlayerStart,
    Snapshot, StartPayload,
};

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
                // Game flips alive_players() the tick the last building dies; the snapshot
                // we just observed may still show owned buildings, so PlayerMilestones
                // hasn't set `eliminated` yet. Backfill it here so goals that opt into
                // `.allowing_elimination_before_milestones()` can accept the outcome.
                for spec in &self.player_specs {
                    if !alive.contains(&spec.id) {
                        if let Some(player) = self.milestones.players.get_mut(&spec.id) {
                            player.eliminated = true;
                        }
                    }
                }
                if self.milestones.complete() {
                    return Ok(SelfPlayReport {
                        ticks: tick,
                        commands: self.commands.len(),
                        replay_commands: self.replay_commands_len,
                    });
                }
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
                    alive_player_ids: &alive,
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
                    Event::Death { .. }
                    | Event::Build { .. }
                    | Event::Notice { .. }
                    | Event::ArtilleryTarget { .. }
                    | Event::ArtilleryImpact { .. }
                    | Event::MortarLaunch { .. }
                    | Event::MortarImpact { .. }
                    | Event::SmokeLaunch { .. } => None,
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
        self.write_artifact_dir(&dir, Some(failure.reason.clone()))?;
        Ok(dir)
    }

    fn write_success_artifact_if_requested(&self) -> Result<Option<PathBuf>, String> {
        let Some(name) = requested_replay_artifact_name(self.test_name)? else {
            return Ok(None);
        };
        let dir = self.artifact_root(SELFPLAY_ARTIFACT_DIR).join(name);
        self.write_artifact_dir(&dir, None)?;
        Ok(Some(dir))
    }

    fn diagnostic_payload(&self, failure: Option<String>) -> SelfPlayDiagnostic {
        SelfPlayDiagnostic {
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

    fn write_artifact_dir(&self, dir: &Path, failure: Option<String>) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let artifact = ReplayArtifactV1::capture_from_game(
            &self.game,
            super::server_build_sha(),
            None,
            self.game.scores(),
        );
        let json = serde_json::to_vec_pretty(&artifact).map_err(|e| e.to_string())?;
        fs::write(dir.join("replay.json"), json).map_err(|e| e.to_string())?;
        let diagnostic = self.diagnostic_payload(failure);
        fs::write(dir.join("summary.log"), diagnostic.summary_log()).map_err(|e| e.to_string())?;
        let json = serde_json::to_vec_pretty(&diagnostic).map_err(|e| e.to_string())?;
        fs::write(dir.join("diagnostic.json"), json).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[test]
fn selfplay_failure_artifact_writes_unified_replay_schema() {
    let players = vec![PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Failure Artifact".to_string(),
        color: "#ffffff".to_string(),
        is_ai: false,
    }];
    let game = Game::new(&players, 0x1234_5678);
    let start = game.start_payload();
    let runner = SelfPlayRunner::new("failure_artifact_schema", game, start, players, Vec::new());

    let dir = runner
        .write_failure_artifact(&SelfPlayFailure::new("schema check"))
        .unwrap();
    let json = fs::read_to_string(dir.join("replay.json")).unwrap();
    let artifact: ReplayArtifactV1 = serde_json::from_str(&json).unwrap();

    assert_eq!(
        artifact.artifact_schema_version,
        rts_sim::game::replay::REPLAY_ARTIFACT_SCHEMA_VERSION_V2
    );
    assert_eq!(artifact.seed, 0x1234_5678);
    assert_eq!(artifact.players[0].name, "Failure Artifact");
    assert_eq!(artifact.players[0].faction_id, "kriegsia");

    let _ = fs::remove_dir_all(dir);
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

struct SelfPlayReport {
    ticks: u32,
    commands: usize,
    replay_commands: usize,
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

#[derive(Serialize)]
struct SelfPlayDiagnostic {
    test_name: &'static str,
    failure: Option<String>,
    start: StartPayload,
    players: Vec<PlayerInit>,
    milestones: Milestones,
    commands: Vec<CommandRecord>,
    replay_commands: Vec<CommandLogEntry>,
    events: Vec<EventRecord>,
    replay_events: Vec<EventLogEntry>,
    samples: Vec<SnapshotSample>,
    seed: u32,
}

impl SelfPlayDiagnostic {
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
            team_id: player.id,
            faction_id: "kriegsia".to_string(),
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

fn ai_1_0_tech_goal() -> PlayerMilestoneGoal {
    PlayerMilestoneGoal {
        require_gathering: true,
        require_oil: true,
        require_oil_worker_assignment: true,
        require_depot_supply: true,
        require_barracks_complete: true,
        require_rifleman: true,
        require_tank: true,
        ..PlayerMilestoneGoal::default()
    }
    .with_min_workers(12)
    .with_min_supply_cap(config::CITY_CENTRE_SUPPLY + config::DEPOT_SUPPLY)
    .with_min_buildings(kinds::TRAINING_CENTRE, 1)
    .with_min_buildings(kinds::RESEARCH_COMPLEX, 1)
    .with_min_buildings(kinds::FACTORY, 1)
    .with_min_buildings(kinds::CITY_CENTRE, 2)
    .with_min_units(kinds::RIFLEMAN, 4)
    .with_min_units(kinds::SCOUT_CAR, 1)
    .with_min_units(kinds::TANK, 1)
}

#[derive(Default)]
struct ResourceRegressionEvidence {
    pre_expansion_steel_gather_tick: Option<u32>,
    first_oil_gather_tick: Option<u32>,
    first_mineable_oil_tick: Option<u32>,
    first_second_completed_city_centre_tick: Option<u32>,
}

fn profile_players() -> Vec<PlayerInit> {
    vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "AI Resource Regression".into(),
            color: "#4cc9f0".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "AI Mirror".into(),
            color: "#f72585".into(),
            is_ai: true,
        },
    ]
}

fn gather_node_kind(start: &StartPayload, node: u32) -> Option<EntityKind> {
    start
        .map
        .resources
        .iter()
        .find(|resource| resource.id == node)
        .and_then(|resource| resource.kind.parse().ok())
}

fn completed_city_centres(snapshot: &Snapshot, player_id: u32) -> Vec<&EntityView> {
    snapshot
        .entities
        .iter()
        .filter(|entity| entity.owner == player_id)
        .filter(|entity| kind_of(entity) == Some(EntityKind::CityCentre))
        .filter(|entity| is_complete(entity))
        .collect()
}

fn resource_remaining(start: &StartPayload, snapshot: &Snapshot, node: u32) -> u32 {
    snapshot
        .resource_deltas
        .iter()
        .find(|delta| delta.id == node)
        .map(|delta| delta.remaining)
        .unwrap_or_else(|| {
            start
                .map
                .resources
                .iter()
                .any(|resource| resource.id == node)
                .then_some(1)
                .unwrap_or(0)
        })
}

fn resource_mineable_by_completed_city_centre(
    start: &StartPayload,
    snapshot: &Snapshot,
    player_id: u32,
    node: u32,
) -> bool {
    let Some(resource) = start
        .map
        .resources
        .iter()
        .find(|resource| resource.id == node)
    else {
        return false;
    };
    if resource_remaining(start, snapshot, node) == 0 {
        return false;
    }
    let range_px = config::MINING_CC_RANGE_TILES * start.map.tile_size as f32;
    let range2 = range_px * range_px + 0.01;
    completed_city_centres(snapshot, player_id)
        .iter()
        .any(|cc| {
            let dx = cc.x - resource.x;
            let dy = cc.y - resource.y;
            dx * dx + dy * dy <= range2
        })
}

fn has_free_mineable_resource(
    start: &StartPayload,
    snapshot: &Snapshot,
    player_id: u32,
    kind: EntityKind,
) -> bool {
    let occupied_nodes: BTreeSet<u32> = snapshot
        .entities
        .iter()
        .filter(|entity| entity.owner == player_id)
        .filter(|entity| kind_of(entity) == Some(EntityKind::Worker))
        .filter_map(|entity| entity.latched_node)
        .collect();
    start.map.resources.iter().any(|resource| {
        resource.kind.parse::<EntityKind>().ok() == Some(kind)
            && !occupied_nodes.contains(&resource.id)
            && resource_mineable_by_completed_city_centre(start, snapshot, player_id, resource.id)
    })
}

fn run_resource_regression_profile(max_ticks: u32) -> ResourceRegressionEvidence {
    let players = profile_players();
    let mut game = Game::new_without_ai_controllers(&players, 0x4100_0004);
    let start = game.start_payload();
    let mut scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(ProfileBackedScript::economy_only(1)),
        Box::new(ProfileBackedScript::economy_only(2)),
    ];
    let mut evidence = ResourceRegressionEvidence::default();

    for tick in 0..max_ticks {
        let alive_player_ids = game.alive_players();
        let snapshots: BTreeMap<u32, Snapshot> = players
            .iter()
            .map(|player| (player.id, game.snapshot_for(player.id)))
            .collect();
        let player_one_snapshot = &snapshots[&1];
        if evidence.first_second_completed_city_centre_tick.is_none()
            && completed_city_centres(player_one_snapshot, 1).len() >= 2
        {
            evidence.first_second_completed_city_centre_tick = Some(tick);
        }
        if evidence.first_mineable_oil_tick.is_none()
            && has_free_mineable_resource(&start, player_one_snapshot, 1, EntityKind::Oil)
        {
            evidence.first_mineable_oil_tick = Some(tick);
        }

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
                alive_player_ids: &alive_player_ids,
            };
            commands.extend(
                script
                    .commands(view)
                    .into_iter()
                    .map(|command| (pid, command)),
            );
        }

        for (player_id, command) in commands {
            if player_id == 1 {
                if let Command::Gather { node, .. } = &command {
                    let kind = gather_node_kind(&start, *node);
                    let has_free_steel = has_free_mineable_resource(
                        &start,
                        player_one_snapshot,
                        1,
                        EntityKind::Steel,
                    );
                    let has_free_oil =
                        has_free_mineable_resource(&start, player_one_snapshot, 1, EntityKind::Oil);
                    if player_one_snapshot.supply_used >= 20
                        && player_one_snapshot.supply_used <= 25
                        && has_free_steel
                        && !has_free_oil
                    {
                        assert_eq!(
                            kind,
                            Some(EntityKind::Steel),
                            "pre-expansion gather at tick {tick} targeted {kind:?} while only steel was mineable"
                        );
                        evidence.pre_expansion_steel_gather_tick.get_or_insert(tick);
                    }
                    if kind == Some(EntityKind::Oil) {
                        assert!(
                            resource_mineable_by_completed_city_centre(
                                &start,
                                player_one_snapshot,
                                1,
                                *node
                            ),
                            "oil gather at tick {tick} targeted a known but non-mineable node"
                        );
                        evidence.first_oil_gather_tick.get_or_insert(tick);
                    }
                }
            }
            game.enqueue(player_id, command);
        }

        game.tick();
    }

    evidence
}

#[test]
fn profile_backed_ai_prefers_mineable_steel_over_known_non_mineable_oil() {
    let evidence = run_resource_regression_profile(6_000);

    assert!(
        evidence.pre_expansion_steel_gather_tick.is_some(),
        "expected a low-to-mid supply pre-expansion steel gather while oil was known but not mineable"
    );
}

#[test]
fn profile_backed_ai_assigns_oil_after_expansion_city_centre_completes() {
    let evidence = run_resource_regression_profile(9_000);

    assert!(
        evidence.first_second_completed_city_centre_tick.is_some(),
        "expected AI 1.0 economy progression to complete an expansion City Centre"
    );
    assert!(
        evidence.first_mineable_oil_tick.is_some(),
        "expected expansion completion to make at least one oil node mineable"
    );
    assert!(
        evidence.first_oil_gather_tick.is_some(),
        "expected profile-backed economy to assign a worker to oil after expansion"
    );
    assert!(
        evidence.first_oil_gather_tick >= evidence.first_mineable_oil_tick,
        "oil gather should not precede the first mineable-oil tick"
    );
}

#[test]
fn profile_backed_self_play_exercises_ai_1_0_tech_arc() {
    if crate::skip_unless_full_ai("profile_backed_self_play_exercises_ai_1_0_tech_arc") {
        return;
    }

    run_profile_matchup(MatchupConfig {
        artifact_name: "profile_backed_self_play_exercises_ai_1_0_tech_arc",
        seed: 0x4100_0004,
        max_ticks: 14_000,
        players: [
            MatchupPlayerSpec {
                id: 1,
                name: "AI 1.0 Tech",
                color: "#4cc9f0",
                profile_id: AI_1_0_TECH_ID,
                goal: ai_1_0_tech_goal(),
            },
            MatchupPlayerSpec {
                id: 2,
                name: "AI 1.0 Mirror",
                color: "#f72585",
                profile_id: AI_1_0_TECH_ID,
                goal: ai_1_0_tech_goal().allowing_elimination_before_milestones(),
            },
        ],
        combat_goal: CombatGoal::damage(),
        assert_outcome: |_| {},
    });
}

#[test]
fn scripted_self_play_worker_rush_vs_economy() {
    let players = vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Worker Rush".into(),
            color: "#e71d36".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
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

#[test]
fn scripted_self_play_mine_only_steel_fairness() {
    const TWO_MINUTES_TICKS: u32 = 2 * 60 * config::TICK_HZ;
    const STEEL_TOLERANCE: u32 = 15;

    let players = vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Miner A".into(),
            color: "#4cc9f0".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
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

    let snapshots: BTreeMap<u32, Snapshot> = players
        .iter()
        .map(|p| (p.id, game.snapshot_for(p.id)))
        .collect();
    let alive_player_ids = game.alive_players();
    let mut commands = Vec::new();
    for script in &mut scripts {
        let pid = script.player_id();
        let Some(snapshot) = snapshots.get(&pid) else {
            continue;
        };
        let view = PlayerView {
            player_id: pid,
            tick: 0,
            start: &start,
            snapshot,
            alive_player_ids: &alive_player_ids,
        };
        commands.extend(
            script
                .commands(view)
                .into_iter()
                .map(|command| (pid, command)),
        );
    }
    for (player_id, command) in commands {
        game.enqueue(player_id, command);
    }

    for _ in 0..TWO_MINUTES_TICKS {
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

/// Run a scripted match pair for a fixed number of ticks and assert both games expose identical
/// per-player snapshots before each tick.
#[cfg(test)]
fn assert_scripted_runs_identical_for_ticks(
    players: &[PlayerInit],
    scripts_a: &mut [Box<dyn ScriptedPlayer>],
    scripts_b: &mut [Box<dyn ScriptedPlayer>],
    start: &StartPayload,
    game_a: &mut Game,
    game_b: &mut Game,
    ticks: u32,
) {
    for tick in 0..ticks {
        let alive_a = game_a.alive_players();
        let alive_b = game_b.alive_players();
        let snapshots_a: BTreeMap<u32, Snapshot> = players
            .iter()
            .map(|p| (p.id, game_a.snapshot_for(p.id)))
            .collect();
        let snapshots_b: BTreeMap<u32, Snapshot> = players
            .iter()
            .map(|p| (p.id, game_b.snapshot_for(p.id)))
            .collect();
        for p in players {
            assert_eq!(
                snapshots_a[&p.id], snapshots_b[&p.id],
                "tick {tick}: player {} snapshots diverged between two fresh runs",
                p.id
            );
        }

        let mut commands_a = Vec::new();
        for script in scripts_a.iter_mut() {
            let pid = script.player_id();
            let Some(snapshot) = snapshots_a.get(&pid) else {
                continue;
            };
            let view = PlayerView {
                player_id: pid,
                tick,
                start,
                snapshot,
                alive_player_ids: &alive_a,
            };
            commands_a.extend(
                script
                    .commands(view)
                    .into_iter()
                    .map(|command| (pid, command)),
            );
        }
        let mut commands_b = Vec::new();
        for script in scripts_b.iter_mut() {
            let pid = script.player_id();
            let Some(snapshot) = snapshots_b.get(&pid) else {
                continue;
            };
            let view = PlayerView {
                player_id: pid,
                tick,
                start,
                snapshot,
                alive_player_ids: &alive_b,
            };
            commands_b.extend(
                script
                    .commands(view)
                    .into_iter()
                    .map(|command| (pid, command)),
            );
        }

        for (player_id, command) in commands_a {
            game_a.enqueue(player_id, command);
        }
        for (player_id, command) in commands_b {
            game_b.enqueue(player_id, command);
        }

        game_a.tick();
        game_b.tick();
    }
}

#[cfg(test)]
fn pending_tracker_start_payload() -> StartPayload {
    StartPayload {
        player_id: 1,
        spectator: false,
        prediction_build_id: None,
        prediction_version: 0,
        debug_mode: false,
        replay: None,
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
            team_id: 1,
            faction_id: "kriegsia".to_string(),
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
        smokes: Vec::new(),
        ability_objects: Vec::new(),
        visible_tiles: Vec::new(),
        remembered_buildings: Vec::new(),
        events: Vec::new(),
        upgrades: Vec::new(),
        player_resources: Vec::new(),
        net_status: rts_sim::protocol::SnapshotNetStatus::default(),
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
        alive_player_ids: &[1],
    }
}

#[test]
fn pending_build_tracker_keeps_moving_worker_past_stale_window() {
    let start = pending_tracker_start_payload();
    let mut tracker = PendingBuildTracker::default();
    tracker.record_commands(
        10,
        &[Command::Build {
            units: vec![2],
            building: EntityKind::CityCentre,
            tile_x: 48,
            tile_y: 70,
            queued: false,
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
            units: vec![2],
            building: EntityKind::CityCentre,
            tile_x: 48,
            tile_y: 70,
            queued: false,
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
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "A".into(),
            color: "#4cc9f0".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
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
    let mut game_b = Game::new(&players, 0x1234_5678);
    let mut scripts_b: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(MineOnlyScript::new(1)),
        Box::new(MineOnlyScript::new(2)),
    ];

    assert_scripted_runs_identical_for_ticks(
        &players,
        &mut scripts_a,
        &mut scripts_b,
        &start,
        &mut game_a,
        &mut game_b,
        TICKS,
    );

    // Command logs must also be identical.
    assert_eq!(
        game_a.command_log(),
        game_b.command_log(),
        "command logs diverged between two fresh runs"
    );
}

#[test]
fn live_ai_two_vs_two_keeps_allied_controllers_independent_and_non_hostile() {
    const TICKS: u32 = 3_600;

    let players = vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "AI Alpha".into(),
            color: "#4cc9f0".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "AI Bravo".into(),
            color: "#4895ef".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 3,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "AI Charlie".into(),
            color: "#f72585".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 4,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "AI Delta".into(),
            color: "#b5179e".into(),
            is_ai: true,
        },
    ];
    let mut game = Game::new_with_starting_resources(&players, 10_000, 10_000, 0x715A_F311);
    let mut controllers: Vec<AiController> = players
        .iter()
        .map(|player| AiController::new(player.id))
        .collect();
    let mut entity_owner: BTreeMap<u32, u32> = BTreeMap::new();
    let mut command_players = BTreeSet::new();
    let mut attack_command_players = BTreeSet::new();
    let mut command_log_cursor = 0usize;

    for _ in 0..TICKS {
        let start = game.start_payload();
        let alive_players = game.alive_players();
        let mut commands = Vec::new();
        for controller in &mut controllers {
            let player_id = controller.player_id();
            if !alive_players.contains(&player_id) {
                continue;
            }
            let snapshot = game.snapshot_for(player_id);
            for entity in &snapshot.entities {
                if entity.owner != 0 {
                    entity_owner.insert(entity.id, entity.owner);
                }
            }
            commands.extend(
                controller
                    .think(AiThinkContext {
                        start: &start,
                        snapshot: &snapshot,
                        alive_player_ids: &alive_players,
                        retreat_commands: game.worker_retreat_commands_for(player_id),
                    })
                    .into_iter()
                    .map(|command| (player_id, command)),
            );
        }

        for (player_id, command) in commands {
            if let Command::Attack { target, .. } = &command {
                if let Some(target_owner) = entity_owner.get(target) {
                    assert!(
                        game.is_enemy_player(player_id, *target_owner),
                        "AI player {player_id} issued direct attack against allied player {target_owner}"
                    );
                }
            }
            game.enqueue(player_id, command);
        }

        let tick_events = game.tick();
        for (recipient, events) in tick_events {
            for event in events {
                if let Event::Attack { from, to, .. } = event {
                    let attacker_owner = entity_owner.get(&from).copied();
                    let target_owner = entity_owner.get(&to).copied();
                    if let (Some(attacker_owner), Some(target_owner)) =
                        (attacker_owner, target_owner)
                    {
                        assert!(
                            game.is_enemy_player(attacker_owner, target_owner),
                            "same-team attack event delivered to player {recipient}: {attacker_owner} attacked {target_owner}"
                        );
                    }
                }
            }
        }

        for player in &players {
            let snapshot = game.snapshot_for(player.id);
            for entity in &snapshot.entities {
                if entity.owner != 0 {
                    entity_owner.insert(entity.id, entity.owner);
                }
            }
        }

        let command_log = game.command_log();
        for entry in &command_log[command_log_cursor..] {
            command_players.insert(entry.player_id);
            match &entry.command {
                WireCommand::Attack { target, .. } => {
                    attack_command_players.insert(entry.player_id);
                    let target_owner = entity_owner.get(target).copied().unwrap_or_default();
                    assert!(
                        target_owner == 0 || game.is_enemy_player(entry.player_id, target_owner),
                        "AI player {} recorded direct attack against non-enemy owner {}",
                        entry.player_id,
                        target_owner
                    );
                }
                WireCommand::AttackMove { .. } => {
                    attack_command_players.insert(entry.player_id);
                }
                _ => {}
            }
        }
        command_log_cursor = command_log.len();

        if command_players.len() == players.len() && attack_command_players.len() >= 2 {
            break;
        }
    }

    assert_eq!(
        command_players,
        players
            .iter()
            .map(|player| player.id)
            .collect::<BTreeSet<_>>(),
        "each AI player should own and issue its own commands"
    );
    assert!(
        !attack_command_players.is_empty(),
        "short 2v2 AI run should reach at least one attack intent"
    );
}

/// Two real AI opponents (AiController vs AiController) fight it out. Produces a
/// deterministic command log and writes a replay artifact to
/// `target/selfplay-artifacts/real_ai_vs_real_ai/replay.json`.
#[test]
fn real_ai_vs_real_ai() {
    use std::collections::{BTreeMap, BTreeSet};

    if crate::skip_unless_full_ai("real_ai_vs_real_ai") {
        return;
    }

    const MIN_PEAK_BARRACKS_ALIVE: usize = 1;
    const MIN_RIFLEMAN_TRAIN_COMMANDS: usize = 4;
    const MIN_SCOUT_CAR_TRAIN_COMMANDS: usize = 1;
    const MIN_TANK_TRAIN_COMMANDS: usize = 1;
    const MIN_ATTACK_MOVE_COMMANDS: usize = 4;
    const MIN_ATTACK_EVENTS: usize = 50;
    const TICKS: u32 = 13_824;

    let players = vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "AI Alpha".into(),
            color: "#4cc9f0".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "AI Beta".into(),
            color: "#f72585".into(),
            is_ai: true,
        },
    ];
    let mut game = Game::new(&players, 0x1234_5678);
    let mut controllers: Vec<AiController> = players
        .iter()
        .map(|player| AiController::new(player.id))
        .collect();

    let mut event_log = Vec::new();
    let mut max_barracks_alive: BTreeMap<u32, usize> = BTreeMap::new();
    let mut max_riflemen_alive: BTreeMap<u32, usize> = BTreeMap::new();
    let mut max_scout_cars_alive: BTreeMap<u32, usize> = BTreeMap::new();
    let mut max_tanks_alive: BTreeMap<u32, usize> = BTreeMap::new();
    let mut seen_riflemen: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut seen_scout_cars: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut seen_tanks: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut attack_events: BTreeMap<u32, usize> = BTreeMap::new();
    let mut death_events: BTreeMap<u32, usize> = BTreeMap::new();
    let mut barracks_build_cmds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut rifleman_train_cmds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut scout_car_train_cmds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut tank_train_cmds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut attack_move_cmds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut command_log_cursor = 0usize;
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
            .join("selfplay-failures")
            .join(&artifact_name);
        if std::fs::create_dir_all(&dir).is_ok() {
            let artifact = ReplayArtifactV1::capture_from_game(
                game,
                super::server_build_sha(),
                None,
                game.scores(),
            );
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
            let start = game.start_payload();
            let alive_players = game.alive_players();
            let mut commands = Vec::new();
            for controller in &mut controllers {
                let player_id = controller.player_id();
                if !alive_players.contains(&player_id) {
                    continue;
                }
                let snapshot = game.snapshot_for(player_id);
                commands.extend(
                    controller
                        .think(AiThinkContext {
                            start: &start,
                            snapshot: &snapshot,
                            alive_player_ids: &alive_players,
                            retreat_commands: game.worker_retreat_commands_for(player_id),
                        })
                        .into_iter()
                        .map(|command| (player_id, command)),
                );
            }
            for (player_id, command) in commands {
                game.enqueue(player_id, command);
            }

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
                let mut scout_cars_alive = 0usize;
                let mut tanks_alive = 0usize;
                let seen_rifle = seen_riflemen.entry(player.id).or_default();
                let seen_scout = seen_scout_cars.entry(player.id).or_default();
                let seen_tank = seen_tanks.entry(player.id).or_default();
                for entity in snapshot.entities.iter().filter(|e| e.owner == player.id) {
                    if entity.kind == kinds::BARRACKS {
                        barracks_alive += 1;
                    }
                    if entity.kind == kinds::RIFLEMAN {
                        riflemen_alive += 1;
                        seen_rifle.insert(entity.id);
                    }
                    if entity.kind == kinds::SCOUT_CAR {
                        scout_cars_alive += 1;
                        seen_scout.insert(entity.id);
                    }
                    if entity.kind == kinds::TANK {
                        tanks_alive += 1;
                        seen_tank.insert(entity.id);
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
                max_scout_cars_alive
                    .entry(player.id)
                    .and_modify(|max| *max = (*max).max(scout_cars_alive))
                    .or_insert(scout_cars_alive);
                max_tanks_alive
                    .entry(player.id)
                    .and_modify(|max| *max = (*max).max(tanks_alive))
                    .or_insert(tanks_alive);
            }

            let command_log = game.command_log();
            for entry in &command_log[command_log_cursor..] {
                match &entry.command {
                    WireCommand::Build { building, .. } if building == kinds::BARRACKS => {
                        *barracks_build_cmds.entry(entry.player_id).or_default() += 1;
                    }
                    WireCommand::Train { unit, .. } if unit == kinds::RIFLEMAN => {
                        *rifleman_train_cmds.entry(entry.player_id).or_default() += 1;
                    }
                    WireCommand::Train { unit, .. } if unit == kinds::SCOUT_CAR => {
                        *scout_car_train_cmds.entry(entry.player_id).or_default() += 1;
                    }
                    WireCommand::Train { unit, .. } if unit == kinds::TANK => {
                        *tank_train_cmds.entry(entry.player_id).or_default() += 1;
                    }
                    WireCommand::AttackMove { .. } => {
                        *attack_move_cmds.entry(entry.player_id).or_default() += 1;
                    }
                    _ => {}
                }
            }
            command_log_cursor = command_log.len();

            if players.iter().all(|player| {
                max_barracks_alive
                    .get(&player.id)
                    .copied()
                    .unwrap_or_default()
                    >= MIN_PEAK_BARRACKS_ALIVE
                    && rifleman_train_cmds
                        .get(&player.id)
                        .copied()
                        .unwrap_or_default()
                        >= MIN_RIFLEMAN_TRAIN_COMMANDS
                    && scout_car_train_cmds
                        .get(&player.id)
                        .copied()
                        .unwrap_or_default()
                        >= MIN_SCOUT_CAR_TRAIN_COMMANDS
                    && tank_train_cmds.get(&player.id).copied().unwrap_or_default()
                        >= MIN_TANK_TRAIN_COMMANDS
                    && attack_move_cmds
                        .get(&player.id)
                        .copied()
                        .unwrap_or_default()
                        >= MIN_ATTACK_MOVE_COMMANDS
                    && attack_events.get(&player.id).copied().unwrap_or_default()
                        >= MIN_ATTACK_EVENTS
            }) {
                break;
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
            let scout_car_trains = scout_car_train_cmds
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let tank_trains = tank_train_cmds.get(&player.id).copied().unwrap_or_default();
            let seen_scout_cars = seen_scout_cars
                .get(&player.id)
                .map(|ids| ids.len())
                .unwrap_or_default();
            let seen_tanks = seen_tanks
                .get(&player.id)
                .map(|ids| ids.len())
                .unwrap_or_default();
            let peak_riflemen = max_riflemen_alive
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let peak_scout_cars = max_scout_cars_alive
                .get(&player.id)
                .copied()
                .unwrap_or_default();
            let peak_tanks = max_tanks_alive.get(&player.id).copied().unwrap_or_default();
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
                scout_car_trains >= MIN_SCOUT_CAR_TRAIN_COMMANDS,
                "player {} trained only {} scout cars (peak scout cars {}, seen scout cars {}, tank trains {}, peak tanks {}, seen tanks {}, attack moves {}, attack events {})",
                player.id,
                scout_car_trains,
                peak_scout_cars,
                seen_scout_cars,
                tank_trains,
                peak_tanks,
                seen_tanks,
                attack_moves,
                attacks,
            );
            assert!(
                tank_trains >= MIN_TANK_TRAIN_COMMANDS,
                "player {} trained only {} tanks (peak tanks {}, seen tanks {}, scout car trains {}, peak scout cars {}, seen scout cars {}, attack moves {}, attack events {})",
                player.id,
                tank_trains,
                peak_tanks,
                seen_tanks,
                scout_car_trains,
                peak_scout_cars,
                seen_scout_cars,
                attack_moves,
                attacks,
            );
            assert!(
                attack_moves >= MIN_ATTACK_MOVE_COMMANDS,
                "player {} issued only {} attack-move commands (peak barracks {}, rifleman train cmds {}, scout car train cmds {}, tank train cmds {}, peak riflemen {}, peak scout cars {}, peak tanks {}, attack events {})",
                player.id,
                attack_moves,
                peak_barracks,
                rifleman_trains,
                scout_car_trains,
                tank_trains,
                peak_riflemen,
                peak_scout_cars,
                peak_tanks,
                attacks,
            );
            assert!(
                attacks >= MIN_ATTACK_EVENTS,
                "player {} produced only {} attack events (peak barracks {}, rifleman train cmds {}, scout car train cmds {}, tank train cmds {}, attack moves {}, peak riflemen {}, peak scout cars {}, peak tanks {}, seen riflemen {}, seen scout cars {}, seen tanks {}, deaths {})",
                player.id,
                attacks,
                peak_barracks,
                rifleman_trains,
                scout_car_trains,
                tank_trains,
                attack_moves,
                peak_riflemen,
                peak_scout_cars,
                peak_tanks,
                seen_riflemen,
                seen_scout_cars,
                seen_tanks,
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
    let artifact =
        ReplayArtifactV1::capture_from_game(&game, super::server_build_sha(), None, game.scores());
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
