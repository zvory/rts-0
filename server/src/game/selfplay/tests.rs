use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::milestones::{
    AttackerInfo, CombatGoal, Milestones, PlayerMilestoneGoal, PlayerMilestones, SnapshotSample,
};
use super::pending_build::{PendingBuildTracker, PENDING_BUILD_STALE_TICKS};
use super::player_view::{kind_of, PlayerView};
use super::replay::{
    assert_replay_matches_live, is_safe_artifact_name, ReplayArtifact, SelfPlayFailure,
};
use super::scripts::{MineOnlyScript, ProfileBackedScript, ScriptedPlayer, WorkerRushScript};
use super::validation::validate_snapshot;
use super::{
    MAX_STALL_TICKS, MAX_TICKS, SAMPLE_EVERY_TICKS, SAVE_REPLAY_ENV, SELFPLAY_ARTIFACT_DIR,
    SELFPLAY_FAILURE_DIR,
};
use crate::config;
use crate::game::ai_core::profiles::{
    RIFLE_FLOOD_FAST_ID, RIFLE_FLOOD_FULL_SATURATION_ID, TECH_TO_TANKS_ID,
};
use crate::game::command::SimCommand as Command;
use crate::game::entity::EntityKind;
use crate::game::replay::{CommandLogEntry, EventLogEntry};
use crate::game::{Game, PlayerInit};
use crate::protocol::{
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
struct SelfPlayArtifact {
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

fn tech_to_tanks_under_macro_rifle_pressure_goal() -> PlayerMilestoneGoal {
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

fn assert_full_saturation_pressure_and_tech_response(milestones: &Milestones) {
    let full = player_milestones(milestones, 1);

    assert!(
        full.max_units_by_kind
            .get(kinds::RIFLEMAN)
            .copied()
            .unwrap_or_default()
            >= 6,
        "full saturation should reach strong rifle production"
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
                goal: tech_to_tanks_under_macro_rifle_pressure_goal(),
            },
        ],
        combat_goal: CombatGoal::damage(),
        assert_outcome: assert_full_saturation_pressure_and_tech_response,
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
        spectator: false,
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
