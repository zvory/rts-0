use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::super::milestones::{AttackerInfo, Milestones, SnapshotSample};
use super::super::player_view::{kind_of, PlayerView};
use super::super::replay::{assert_replay_matches_live, is_safe_artifact_name, SelfPlayFailure};
use super::super::scripts::ScriptedPlayer;
use super::super::validation::validate_snapshot;
use super::super::{
    MAX_STALL_TICKS, MAX_TICKS, SAMPLE_EVERY_TICKS, SAVE_REPLAY_ENV, SELFPLAY_ARTIFACT_DIR,
    SELFPLAY_FAILURE_DIR,
};
use rts_sim::game::entity::EntityKind;
use rts_sim::game::replay::{
    CommandLogEntry, EventLogEntry, ReplayArtifactV1, ReplayStartComposition,
};
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{Command as WireCommand, Event, Snapshot, StartPayload};

pub(super) struct SelfPlayRunner {
    test_name: &'static str,
    max_ticks: u32,
    pub(super) game: Game,
    replay_start: ReplayStartComposition,
    start: StartPayload,
    resource_kinds: BTreeMap<u32, EntityKind>,
    player_specs: Vec<PlayerInit>,
    scripts: Vec<Box<dyn ScriptedPlayer>>,
    commands: Vec<CommandRecord>,
    replay_commands_len: usize,
    events: Vec<EventRecord>,
    pub(super) event_log: Vec<EventLogEntry>,
    samples: Vec<SnapshotSample>,
    pub(super) milestones: Milestones,
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

    pub(super) fn with_milestones(
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

    pub(super) fn with_options(
        test_name: &'static str,
        max_ticks: u32,
        game: Game,
        start: StartPayload,
        player_specs: Vec<PlayerInit>,
        scripts: Vec<Box<dyn ScriptedPlayer>>,
        milestones: Milestones,
    ) -> Self {
        let resource_kinds = resource_kinds_from_start(&start);
        let replay_start =
            ReplayStartComposition::capture(&game, super::server_build_sha())
                .expect("self-play replay start should export");
        SelfPlayRunner {
            test_name,
            max_ticks,
            game,
            replay_start,
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

    pub(super) fn run(&mut self) -> Result<SelfPlayReport, SelfPlayFailure> {
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
                    Event::Overpenetration { .. }
                    | Event::Miss { .. }
                    | Event::Death { .. }
                    | Event::Build { .. }
                    | Event::Notice { .. }
                    | Event::ArtilleryTarget { .. }
                    | Event::ArtilleryFiring { .. }
                    | Event::ArtilleryImpact { .. }
                    | Event::MortarLaunch { .. }
                    | Event::MortarImpact { .. }
                    | Event::PanzerfaustLaunch { .. }
                    | Event::PanzerfaustImpact { .. }
                    | Event::PanzerfaustConversion { .. }
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

    pub(super) fn write_failure_artifact(
        &self,
        failure: &SelfPlayFailure,
    ) -> Result<PathBuf, String> {
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
        let artifact = self.replay_start.finalize(&self.game, None, self.game.scores());
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
        rts_sim::game::replay::REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION
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

pub(super) struct SelfPlayReport {
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

pub(super) fn replay_artifact_url(name: &str) -> String {
    format!("/dev/replay-artifact?replay={name}")
}

pub(super) fn finalize_self_play_success(
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
                    replay_artifact_url(&name)
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
