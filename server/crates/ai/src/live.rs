//! Live gameplay AI adapter. See `docs/design/ai.md`.
//!
//! The room task invokes controllers before `Game::tick()`, using the same fog-filtered snapshot
//! surface a player would receive. Controllers emit ordinary [`SimCommand`]s; the authoritative
//! simulation validates and records them exactly like human commands.

use std::collections::{BTreeMap, BTreeSet};

use crate::ai_core::decision::{
    decide_profile_with_analysis, observer_debug_map_layers_for_profile, AiDecisionMemory,
};
use crate::ai_core::map_analysis::AiStaticMapContextCache;
use crate::ai_core::observation::AiObservation;
use crate::ai_core::profiles::{
    profile_by_id, AiProfile, AI_2_1, AI_2_1_ID, AI_TURTLE_ID, JEFFS_AI_ID,
};
use crate::ai_shared;
use crate::sdk::{AiActionRequest, AiActions, AiFrame, AiStrategy};
use crate::selfplay::pending_build::PendingBuildTracker;
use crate::selfplay::player_view::{
    footprint_placeable_from_snapshot, occupied_tiles_from_snapshot, PlayerView,
};
use rand::Rng;
use rts_protocol::{ObserverMapAnalysisDiagnostics, ObserverMapAnalysisLayer};
use rts_sim::game::command::SimCommand;
use rts_sim::game::Game;
use rts_sim::protocol::{Snapshot, StartPayload};

const DECISION_INTERVAL: u32 = 9;
const LIVE_DECISION_TRACE_MAX_LINES: usize = 24;
const LIVE_DECISION_TRACE_MAX_LINE_CHARS: usize = 256;
const LIVE_DECISION_TRACE_TRUNCATED_LINE: &str = "trace_truncated=true";
const CUSTOM_STRATEGY_PROFILE_ID: &str = "custom_strategy";

/// The default live-lobby profile id.
pub const DEFAULT_LIVE_PROFILE_ID: &str = AI_2_1_ID;

/// Canonical profile ids understood by the live adapter. Experimental profiles are available to
/// internal observer-only sessions; the room actor prevents them from entering human matches.
pub const LIVE_PROFILE_IDS: [&str; 3] = [AI_2_1_ID, JEFFS_AI_ID, AI_TURTLE_ID];

pub fn canonical_live_profile_id(input: &str) -> Option<&'static str> {
    match input {
        "ai" | "default" | AI_2_1_ID => Some(AI_2_1_ID),
        JEFFS_AI_ID => Some(JEFFS_AI_ID),
        AI_TURTLE_ID => Some(AI_TURTLE_ID),
        _ => None,
    }
}

pub fn live_profile_label(profile_id: &str) -> &'static str {
    match canonical_live_profile_id(profile_id) {
        Some(AI_2_1_ID) => "AI 2.1",
        Some(JEFFS_AI_ID) => "Jeff's AI",
        Some(AI_TURTLE_ID) => "AI Turtle",
        _ => "AI",
    }
}

pub fn is_player_live_profile_id(profile_id: &str) -> bool {
    matches!(profile_id, AI_2_1_ID | JEFFS_AI_ID)
}

pub fn random_live_profile_id(rng: &mut impl Rng) -> &'static str {
    LIVE_PROFILE_IDS[rng.gen_range(0..LIVE_PROFILE_IDS.len())]
}

pub fn resolve_live_profile_id_for_match(profile_id: &str) -> &'static str {
    canonical_live_profile_id(profile_id).unwrap_or(DEFAULT_LIVE_PROFILE_ID)
}

pub struct AiThinkContext<'a> {
    pub start: &'a StartPayload,
    pub snapshot: &'a Snapshot,
    pub alive_player_ids: &'a [u32],
    pub retreat_commands: Vec<SimCommand>,
}

/// Selects the host-owned elimination policy used to decide which controllers run this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiAlivePolicy {
    /// Normal mixed-player matches eliminate a player when they have no surviving units.
    Normal,
    /// AI-only observation matches remain active while their starting primary base survives.
    StartingPrimaryBase,
}

/// One controller invocation captured before any emitted command is enqueued.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AiControllerTickInvocation {
    SkippedDead,
    Invoked {
        snapshot: Box<Snapshot>,
        retreat_commands: Vec<SimCommand>,
        emitted_commands: Vec<SimCommand>,
        decision_trace: Option<AiDecisionTraceSnapshot>,
    },
}

/// Ordered evidence from one canonical pre-simulation AI tick.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanonicalAiTickReport {
    pub tick: u32,
    pub alive_player_ids: Vec<u32>,
    pub controllers: Vec<AiControllerTickReport>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AiControllerTickReport {
    pub player_id: u32,
    pub profile_id: &'static str,
    pub invocation: AiControllerTickInvocation,
}

/// Canonical live/offline AI host orchestration.
///
/// The driver captures every controller's immutable pre-tick inputs and result in controller
/// order, then enqueues all results in that same order. It deliberately does not advance the
/// simulation or decide when a match ends.
pub struct CanonicalAiTickDriver;

impl CanonicalAiTickDriver {
    pub fn run(game: &mut Game, controllers: &mut [AiController], alive_policy: AiAlivePolicy) {
        Self::run_inner(game, controllers, alive_policy, false);
    }

    /// Runs the canonical tick path and retains its pre-tick inputs and controller outputs.
    ///
    /// Normal hosts should use [`Self::run`]. Reports retain full per-player snapshots and clone
    /// commands that are also enqueued, so this path is intended for tests and offline tooling
    /// that consume that evidence.
    pub(crate) fn run_with_report(
        game: &mut Game,
        controllers: &mut [AiController],
        alive_policy: AiAlivePolicy,
    ) -> CanonicalAiTickReport {
        Self::run_inner(game, controllers, alive_policy, true)
            .expect("report capture was requested")
    }

    fn run_inner(
        game: &mut Game,
        controllers: &mut [AiController],
        alive_policy: AiAlivePolicy,
        capture_report: bool,
    ) -> Option<CanonicalAiTickReport> {
        let tick = game.tick_count();
        let start = game.start_payload();
        let alive_player_ids = match alive_policy {
            AiAlivePolicy::Normal => game.alive_players(),
            AiAlivePolicy::StartingPrimaryBase => game.primary_base_alive_players(),
        };
        let mut pending_commands = Vec::with_capacity(controllers.len());
        let mut reports = capture_report.then(|| Vec::with_capacity(controllers.len()));

        for controller in controllers {
            let player_id = controller.player_id();
            let profile_id = controller.profile_id();
            if !alive_player_ids.contains(&player_id) {
                if let Some(reports) = reports.as_mut() {
                    reports.push(AiControllerTickReport {
                        player_id,
                        profile_id,
                        invocation: AiControllerTickInvocation::SkippedDead,
                    });
                }
                continue;
            }
            let snapshot = game.snapshot_for(player_id);
            let retreat_commands = game.worker_retreat_commands_for(player_id);
            let reported_retreat_commands = capture_report.then(|| retreat_commands.clone());
            let emitted_commands = controller.think(AiThinkContext {
                start: &start,
                snapshot: &snapshot,
                alive_player_ids: &alive_player_ids,
                retreat_commands,
            });
            let decision_trace = capture_report
                .then(|| controller.latest_decision_trace())
                .flatten()
                .filter(|trace| trace.trace_tick == tick);
            if let Some(reports) = reports.as_mut() {
                reports.push(AiControllerTickReport {
                    player_id,
                    profile_id,
                    invocation: AiControllerTickInvocation::Invoked {
                        snapshot: Box::new(snapshot),
                        retreat_commands: reported_retreat_commands
                            .expect("report capture was requested"),
                        emitted_commands: emitted_commands.clone(),
                        decision_trace,
                    },
                });
            }
            pending_commands.push((player_id, emitted_commands));
        }

        for (player_id, commands) in pending_commands {
            for command in commands {
                game.enqueue(player_id, command);
            }
        }

        reports.map(|controllers| CanonicalAiTickReport {
            tick,
            alive_player_ids,
            controllers,
        })
    }

    pub(crate) fn run_prepared_controller(
        controller: &mut AiController,
        context: AiThinkContext<'_>,
    ) -> (Vec<SimCommand>, Option<AiDecisionTraceSnapshot>) {
        let tick = context.snapshot.tick;
        let commands = controller.think(context);
        let trace = controller
            .latest_decision_trace()
            .filter(|trace| trace.trace_tick == tick);
        (commands, trace)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiDecisionTraceSnapshot {
    pub player_id: u32,
    pub profile_id: &'static str,
    pub trace_tick: u32,
    pub lines: Vec<String>,
}

/// Drives a single AI-controlled player by emitting ordinary commands each think.
pub struct AiController {
    player: u32,
    profile_id: &'static str,
    memory: AiDecisionMemory,
    static_map_context: AiStaticMapContextCache,
    pending_builds: PendingBuildTracker,
    staged_units: BTreeSet<u32>,
    held_stage_units: BTreeSet<u32>,
    active_attack_units: BTreeMap<u32, u32>,
    last_decision_trace: Option<AiDecisionTraceSnapshot>,
    last_debug_map_layers: Vec<ObserverMapAnalysisLayer>,
    custom_strategy: Option<Box<dyn AiStrategy>>,
    strategy_initialized: bool,
}

struct LegacyProfileStrategy;

impl LegacyProfileStrategy {
    fn project(frame: &AiFrame) -> Option<AiObservation> {
        AiObservation::from_frame(frame)
    }
}

impl AiController {
    pub fn new(player: u32) -> Self {
        Self::with_profile_id(player, DEFAULT_LIVE_PROFILE_ID)
    }

    pub fn with_profile_id(player: u32, profile_id: &'static str) -> Self {
        let profile = profile_by_id(profile_id).unwrap_or_else(default_live_profile);
        Self {
            player,
            profile_id: profile.id,
            memory: AiDecisionMemory::for_profile(profile),
            static_map_context: AiStaticMapContextCache::default(),
            pending_builds: PendingBuildTracker::default(),
            staged_units: BTreeSet::new(),
            held_stage_units: BTreeSet::new(),
            active_attack_units: BTreeMap::new(),
            last_decision_trace: None,
            last_debug_map_layers: Vec::new(),
            custom_strategy: None,
            strategy_initialized: false,
        }
    }

    /// Constructs a custom strategy in the same cadence, fog, validation, and replay envelope as
    /// built-in profiles.
    pub fn with_strategy(player: u32, strategy: Box<dyn AiStrategy>) -> Self {
        let mut controller = Self::new(player);
        controller.profile_id = CUSTOM_STRATEGY_PROFILE_ID;
        controller.custom_strategy = Some(strategy);
        controller
    }

    pub fn player_id(&self) -> u32 {
        self.player
    }

    pub fn profile_id(&self) -> &'static str {
        self.profile_id
    }

    pub fn latest_decision_trace(&self) -> Option<AiDecisionTraceSnapshot> {
        self.last_decision_trace.clone()
    }

    pub fn latest_map_analysis_diagnostics(&self) -> Option<ObserverMapAnalysisDiagnostics> {
        self.static_map_context
            .current()
            .map(|context| context.diagnostics().clone())
    }

    pub fn latest_debug_map_layers(&self) -> Vec<ObserverMapAnalysisLayer> {
        self.last_debug_map_layers.clone()
    }

    fn profile(&self) -> &'static AiProfile {
        profile_by_id(self.profile_id).unwrap_or_else(default_live_profile)
    }

    pub fn think(&mut self, context: AiThinkContext<'_>) -> Vec<SimCommand> {
        let mut commands = context.retreat_commands;
        let tick = context.snapshot.tick;
        if self.custom_strategy.is_none() {
            self.static_map_context.get_or_analyze(context.start);
        }
        if !tick
            .wrapping_add(self.player)
            .is_multiple_of(DECISION_INTERVAL)
        {
            return commands;
        }

        let view = PlayerView {
            player_id: self.player,
            tick,
            start: context.start,
            snapshot: context.snapshot,
            alive_player_ids: context.alive_player_ids,
        };
        self.pending_builds.observe(view);
        let pending_builds = self.pending_builds.intents();
        let Some(frame) = AiFrame::from_host(
            context.start,
            context.snapshot,
            self.player,
            pending_builds.clone(),
            Some(context.alive_player_ids),
        ) else {
            return commands;
        };

        #[cfg(test)]
        {
            let direct = AiObservation::from_snapshot_with_alive(
                context.start,
                context.snapshot,
                self.player,
                pending_builds,
                Some(context.alive_player_ids),
            );
            assert_eq!(LegacyProfileStrategy::project(&frame), direct);
        }

        if let Some(strategy) = self.custom_strategy.as_mut() {
            if !self.strategy_initialized {
                strategy.initialize(&frame);
                self.strategy_initialized = true;
            }
            let mut actions = AiActions::new();
            strategy.step(&frame, &mut actions);
            commands.extend(actions.into_requests().into_iter().map(action_to_command));
            self.pending_builds.record_commands(tick, &commands);
            return commands;
        }

        let Some(observation) = LegacyProfileStrategy::project(&frame) else {
            return commands;
        };
        self.prune_combat_memory(&observation, tick);

        let profile = self.profile();
        let occupied = occupied_tiles_from_snapshot(&context.start.map, context.snapshot);
        let failed_builds = &self.pending_builds;
        let map_analysis = self
            .static_map_context
            .get_or_analyze(context.start)
            .analysis();
        self.last_debug_map_layers =
            observer_debug_map_layers_for_profile(&observation, map_analysis, profile);
        let decision = decide_profile_with_analysis(
            &observation,
            profile,
            &mut self.memory,
            map_analysis,
            ai_shared::BuildSearch {
                min_radius: 2,
                max_radius: ai_shared::DEFAULT_BUILD_SEARCH_MAX_RADIUS,
                prefer_away_from_center: false,
                prefer_toward_center: false,
            },
            |building, tile_x, tile_y| {
                !failed_builds.failed(building, tile_x, tile_y)
                    && footprint_placeable_from_snapshot(
                        &context.start.map,
                        context.snapshot,
                        building,
                        tile_x,
                        tile_y,
                        &occupied,
                    )
            },
        );
        debug_assert_eq!(decision.profile_id, self.profile_id);

        self.last_decision_trace = Some(AiDecisionTraceSnapshot {
            player_id: self.player,
            profile_id: self.profile_id,
            trace_tick: tick,
            lines: bounded_decision_trace_lines(decision.trace.format_lines()),
        });
        let legacy_commands =
            self.filter_repeated_stage_commands(tick, &decision.intents, decision.commands);
        let mut actions = AiActions::new();
        for command in legacy_commands {
            if let Some(request) = legacy_command_to_action(command) {
                actions.submit(request);
            }
        }
        commands.extend(actions.into_requests().into_iter().map(action_to_command));
        self.pending_builds.record_commands(tick, &commands);
        commands
    }

    fn prune_combat_memory(&mut self, observation: &AiObservation, tick: u32) {
        let owned: BTreeSet<u32> = observation.owned.iter().map(|entity| entity.id).collect();
        self.staged_units.retain(|id| owned.contains(id));
        self.held_stage_units.retain(|id| owned.contains(id));
        let suppress_ticks = self
            .profile()
            .attack
            .reissue_cadence_ticks
            .max(crate::selfplay::SELFPLAY_ATTACK_STAGE_SUPPRESSION_TICKS);
        self.active_attack_units.retain(|id, issued| {
            owned.contains(id) && tick.saturating_sub(*issued) < suppress_ticks
        });
    }

    fn filter_repeated_stage_commands(
        &mut self,
        tick: u32,
        intents: &[crate::ai_core::decision::AiIntent],
        commands: Vec<SimCommand>,
    ) -> Vec<SimCommand> {
        let mut attacking = BTreeSet::new();
        let mut staging = BTreeSet::new();
        for intent in intents {
            match intent {
                crate::ai_core::decision::AiIntent::Attack { units } => {
                    attacking.extend(units.iter().copied())
                }
                crate::ai_core::decision::AiIntent::Stage { units } => {
                    staging.extend(units.iter().copied())
                }
                crate::ai_core::decision::AiIntent::Move { .. }
                | crate::ai_core::decision::AiIntent::Build { .. }
                | crate::ai_core::decision::AiIntent::ResumeConstruction { .. }
                | crate::ai_core::decision::AiIntent::Train { .. }
                | crate::ai_core::decision::AiIntent::Research { .. }
                | crate::ai_core::decision::AiIntent::Gather { .. } => {}
            }
        }
        for id in &attacking {
            self.staged_units.remove(id);
            self.held_stage_units.remove(id);
            self.active_attack_units.insert(*id, tick);
        }
        if staging.is_empty() {
            return commands;
        }

        let mut filtered = Vec::new();
        let mut freshly_staged = BTreeSet::new();
        let command_stages_units = |units: &[u32]| units.iter().any(|id| staging.contains(id));
        for command in commands {
            match command {
                SimCommand::AttackMove {
                    units,
                    x,
                    y,
                    queued,
                } if command_stages_units(&units) => {
                    let fresh: Vec<u32> = units
                        .into_iter()
                        .filter(|id| !self.staged_units.contains(id))
                        .filter(|id| !self.active_attack_units.contains_key(id))
                        .collect();
                    self.staged_units.extend(fresh.iter().copied());
                    for id in &fresh {
                        self.held_stage_units.remove(id);
                    }
                    freshly_staged.extend(fresh.iter().copied());
                    if !fresh.is_empty() {
                        filtered.push(SimCommand::AttackMove {
                            units: fresh,
                            x,
                            y,
                            queued,
                        });
                    }
                }
                SimCommand::Move {
                    units,
                    x,
                    y,
                    queued,
                } if command_stages_units(&units) => {
                    let fresh: Vec<u32> = units
                        .into_iter()
                        .filter(|id| !self.staged_units.contains(id))
                        .filter(|id| !self.active_attack_units.contains_key(id))
                        .collect();
                    self.staged_units.extend(fresh.iter().copied());
                    for id in &fresh {
                        self.held_stage_units.remove(id);
                    }
                    freshly_staged.extend(fresh.iter().copied());
                    if !fresh.is_empty() {
                        filtered.push(SimCommand::Move {
                            units: fresh,
                            x,
                            y,
                            queued,
                        });
                    }
                }
                SimCommand::HoldPosition { units, queued } if command_stages_units(&units) => {
                    let fresh: Vec<u32> = units
                        .into_iter()
                        .filter(|id| !self.active_attack_units.contains_key(id))
                        .filter(|id| !self.held_stage_units.contains(id))
                        .collect();
                    self.staged_units.extend(fresh.iter().copied());
                    self.held_stage_units.extend(fresh.iter().copied());
                    if !fresh.is_empty() {
                        filtered.push(SimCommand::HoldPosition {
                            units: fresh,
                            queued,
                        });
                    }
                }
                SimCommand::SetupAntiTankGuns {
                    units,
                    x,
                    y,
                    queued,
                } if command_stages_units(&units) => {
                    let fresh: Vec<u32> = units
                        .into_iter()
                        .filter(|id| !self.active_attack_units.contains_key(id))
                        .filter(|id| !self.staged_units.contains(id) || freshly_staged.contains(id))
                        .collect();
                    self.staged_units.extend(fresh.iter().copied());
                    if !fresh.is_empty() {
                        filtered.push(SimCommand::SetupAntiTankGuns {
                            units: fresh,
                            x,
                            y,
                            queued,
                        });
                    }
                }
                other => filtered.push(other),
            }
        }
        filtered
    }
}

fn action_to_command(request: AiActionRequest) -> SimCommand {
    match request {
        AiActionRequest::Move {
            units,
            x,
            y,
            queued,
        } => SimCommand::Move {
            units,
            x,
            y,
            queued,
        },
        AiActionRequest::AttackMove {
            units,
            x,
            y,
            queued,
        } => SimCommand::AttackMove {
            units,
            x,
            y,
            queued,
        },
        AiActionRequest::Attack {
            units,
            target,
            queued,
        } => SimCommand::Attack {
            units,
            target,
            queued,
        },
        AiActionRequest::Gather {
            units,
            node,
            queued,
        } => SimCommand::Gather {
            units,
            node,
            queued,
        },
        AiActionRequest::Build {
            units,
            building,
            tile_x,
            tile_y,
            queued,
        } => SimCommand::Build {
            units,
            building,
            tile_x,
            tile_y,
            queued,
        },
        AiActionRequest::Train { building, unit } => SimCommand::Train { building, unit },
        AiActionRequest::Research { building, upgrade } => {
            SimCommand::Research { building, upgrade }
        }
        AiActionRequest::HoldPosition { units, queued } => {
            SimCommand::HoldPosition { units, queued }
        }
        AiActionRequest::SetupAntiTankGuns {
            units,
            x,
            y,
            queued,
        } => SimCommand::SetupAntiTankGuns {
            units,
            x,
            y,
            queued,
        },
    }
}

fn legacy_command_to_action(command: SimCommand) -> Option<AiActionRequest> {
    match command {
        SimCommand::Move {
            units,
            x,
            y,
            queued,
        } => Some(AiActionRequest::Move {
            units,
            x,
            y,
            queued,
        }),
        SimCommand::AttackMove {
            units,
            x,
            y,
            queued,
        } => Some(AiActionRequest::AttackMove {
            units,
            x,
            y,
            queued,
        }),
        SimCommand::Attack {
            units,
            target,
            queued,
        } => Some(AiActionRequest::Attack {
            units,
            target,
            queued,
        }),
        SimCommand::Gather {
            units,
            node,
            queued,
        } => Some(AiActionRequest::Gather {
            units,
            node,
            queued,
        }),
        SimCommand::Build {
            units,
            building,
            tile_x,
            tile_y,
            queued,
        } => Some(AiActionRequest::Build {
            units,
            building,
            tile_x,
            tile_y,
            queued,
        }),
        SimCommand::Train { building, unit } => Some(AiActionRequest::Train { building, unit }),
        SimCommand::Research { building, upgrade } => {
            Some(AiActionRequest::Research { building, upgrade })
        }
        SimCommand::HoldPosition { units, queued } => {
            Some(AiActionRequest::HoldPosition { units, queued })
        }
        SimCommand::SetupAntiTankGuns {
            units,
            x,
            y,
            queued,
        } => Some(AiActionRequest::SetupAntiTankGuns {
            units,
            x,
            y,
            queued,
        }),
        _ => None,
    }
}

fn default_live_profile() -> &'static AiProfile {
    profile_by_id(DEFAULT_LIVE_PROFILE_ID).unwrap_or(&AI_2_1)
}

fn bounded_decision_trace_lines(lines: Vec<String>) -> Vec<String> {
    let mut iter = lines.into_iter();
    let mut bounded = Vec::new();
    for _ in 0..LIVE_DECISION_TRACE_MAX_LINES {
        let Some(line) = iter.next() else {
            return bounded;
        };
        bounded.push(truncate_decision_trace_line(line));
    }
    if iter.next().is_some() {
        if let Some(last) = bounded.last_mut() {
            *last = LIVE_DECISION_TRACE_TRUNCATED_LINE.to_string();
        }
    }
    bounded
}

fn truncate_decision_trace_line(mut line: String) -> String {
    if let Some((index, _)) = line.char_indices().nth(LIVE_DECISION_TRACE_MAX_LINE_CHARS) {
        line.truncate(index);
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rts_sim::game::PlayerInit;
    use rts_sim::protocol::{terrain, MapInfo, PlayerStart, ResourceNode};

    fn cache_test_start_payload() -> StartPayload {
        StartPayload {
            player_id: 1,
            spectator: false,
            prediction_build_id: None,
            prediction_version: 0,
            match_run_id: None,
            capabilities: Default::default(),
            diagnostics: Default::default(),
            replay: None,
            lab: None,
            observer_view: None,
            tick: 0,
            map: MapInfo {
                width: 8,
                height: 8,
                tile_size: crate::config::TILE_SIZE,
                terrain: vec![terrain::GRASS; 8 * 8],
                resources: vec![ResourceNode {
                    id: 10,
                    kind: rts_sim::protocol::kinds::STEEL.to_string(),
                    x: crate::config::TILE_SIZE as f32 * 5.5,
                    y: crate::config::TILE_SIZE as f32 * 1.5,
                }],
                doodads: Vec::new(),
                stealth_tiles: Vec::new(),
                no_vehicle_tiles: Vec::new(),
            },
            players: vec![PlayerStart {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "P1".to_string(),
                color: "#111".to_string(),
                is_ai: true,
                start_tile_x: 1,
                start_tile_y: 1,
            }],
        }
    }

    fn driver_test_players() -> Vec<PlayerInit> {
        vec![
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "AI One".to_string(),
                color: "#111111".to_string(),
                is_ai: true,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "AI Two".to_string(),
                color: "#222222".to_string(),
                is_ai: true,
            },
        ]
    }

    #[test]
    fn canonical_driver_selects_host_alive_policy_and_skips_eliminated_controllers() {
        let players = driver_test_players();
        let mut normal_game = Game::new_without_ai_controllers(&players, 7);
        let expected_normal = normal_game.alive_players();
        let mut controllers = vec![AiController::new(2), AiController::new(99)];
        let normal = CanonicalAiTickDriver::run_with_report(
            &mut normal_game,
            &mut controllers,
            AiAlivePolicy::Normal,
        );
        assert_eq!(normal.alive_player_ids, expected_normal);
        assert_eq!(normal.controllers[0].player_id, 2);
        assert!(matches!(
            normal.controllers[0].invocation,
            AiControllerTickInvocation::Invoked { .. }
        ));
        assert!(matches!(
            normal.controllers[1].invocation,
            AiControllerTickInvocation::SkippedDead
        ));

        let mut ai_only_game = Game::new_without_ai_controllers(&players, 7);
        let expected_primary_base = ai_only_game.primary_base_alive_players();
        let mut ai_only_controllers = vec![AiController::new(1), AiController::new(2)];
        let ai_only = CanonicalAiTickDriver::run_with_report(
            &mut ai_only_game,
            &mut ai_only_controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
        assert_eq!(ai_only.alive_player_ids, expected_primary_base);
    }

    #[test]
    fn canonical_driver_preserves_controller_order_inputs_and_staggered_traces() {
        let players = driver_test_players();
        let mut game = Game::new_without_ai_controllers(&players, 11);
        let mut controllers = vec![AiController::new(2), AiController::new(1)];

        for tick in 0u32..10 {
            let before_one = game.snapshot_for(1);
            let before_two = game.snapshot_for(2);
            let report = CanonicalAiTickDriver::run_with_report(
                &mut game,
                &mut controllers,
                AiAlivePolicy::StartingPrimaryBase,
            );
            assert_eq!(
                report
                    .controllers
                    .iter()
                    .map(|controller| controller.player_id)
                    .collect::<Vec<_>>(),
                vec![2, 1]
            );
            for controller in &report.controllers {
                let AiControllerTickInvocation::Invoked {
                    snapshot,
                    decision_trace,
                    ..
                } = &controller.invocation
                else {
                    panic!("starting controller unexpectedly skipped");
                };
                let expected_snapshot = if controller.player_id == 1 {
                    &before_one
                } else {
                    &before_two
                };
                assert_eq!(snapshot.as_ref(), expected_snapshot);
                assert_eq!(
                    decision_trace.is_some(),
                    tick.wrapping_add(controller.player_id)
                        .is_multiple_of(DECISION_INTERVAL)
                );
            }
            game.tick();
        }
    }

    #[test]
    fn canonical_driver_report_capture_does_not_change_execution() {
        let players = driver_test_players();
        let mut normal_game = Game::new_without_ai_controllers(&players, 17);
        let mut reported_game = Game::new_without_ai_controllers(&players, 17);
        let mut normal_controllers = vec![AiController::new(1), AiController::new(2)];
        let mut reported_controllers = vec![AiController::new(1), AiController::new(2)];

        for _ in 0..12 {
            CanonicalAiTickDriver::run(
                &mut normal_game,
                &mut normal_controllers,
                AiAlivePolicy::StartingPrimaryBase,
            );
            CanonicalAiTickDriver::run_with_report(
                &mut reported_game,
                &mut reported_controllers,
                AiAlivePolicy::StartingPrimaryBase,
            );
            assert_eq!(normal_game.command_log(), reported_game.command_log());
            assert_eq!(normal_game.tick(), reported_game.tick());
            assert_eq!(
                normal_game.snapshot_full_for(1),
                reported_game.snapshot_full_for(1)
            );
        }
    }

    #[test]
    fn canonical_prepared_driver_keeps_retreats_on_non_decision_ticks() {
        let players = driver_test_players();
        let game = Game::new_without_ai_controllers(&players, 13);
        let start = game.start_payload();
        let snapshot = game.snapshot_for(1);
        assert!(!snapshot
            .tick
            .wrapping_add(1)
            .is_multiple_of(DECISION_INTERVAL));
        let retreat = SimCommand::Move {
            units: vec![42],
            x: 64.0,
            y: 96.0,
            queued: false,
        };
        let mut controller = AiController::new(1);

        let (commands, trace) = CanonicalAiTickDriver::run_prepared_controller(
            &mut controller,
            AiThinkContext {
                start: &start,
                snapshot: &snapshot,
                alive_player_ids: &[1, 2],
                retreat_commands: vec![retreat.clone()],
            },
        );

        assert_eq!(commands, vec![retreat]);
        assert_eq!(trace, None);
    }

    #[test]
    fn custom_strategy_actions_follow_host_retreats_on_decision_ticks() {
        struct MoveStrategy;

        impl AiStrategy for MoveStrategy {
            fn step(&mut self, _frame: &AiFrame, actions: &mut AiActions) {
                actions.submit(AiActionRequest::Move {
                    units: vec![7],
                    x: 128.0,
                    y: 160.0,
                    queued: false,
                });
            }
        }

        let players = driver_test_players();
        let game = Game::new_without_ai_controllers(&players, 13);
        let start = game.start_payload();
        let mut snapshot = game.snapshot_for(1);
        snapshot.tick = 8;
        let retreat = SimCommand::Move {
            units: vec![42],
            x: 64.0,
            y: 96.0,
            queued: false,
        };
        let mut controller = AiController::with_strategy(1, Box::new(MoveStrategy));

        let commands = controller.think(AiThinkContext {
            start: &start,
            snapshot: &snapshot,
            alive_player_ids: &[1, 2],
            retreat_commands: vec![retreat.clone()],
        });

        assert_eq!(commands[0], retreat);
        assert_eq!(
            commands[1],
            SimCommand::Move {
                units: vec![7],
                x: 128.0,
                y: 160.0,
                queued: false,
            }
        );
    }

    #[test]
    fn live_controller_uses_default_profile_id() {
        let ai = AiController::new(2);

        assert_eq!(ai.player_id(), 2);
        assert_eq!(ai.profile_id(), AI_2_1_ID);
        assert_eq!(ai.latest_decision_trace(), None);
    }

    #[test]
    fn live_controller_caches_static_map_analysis_by_start_identity() {
        let mut ai = AiController::new(1);
        let start = cache_test_start_payload();

        let first_key = ai.static_map_context.get_or_analyze(&start).key();
        let second_key = ai.static_map_context.get_or_analyze(&start).key();
        assert_eq!(second_key, first_key);
        assert_eq!(
            ai.static_map_context.current().map(|context| context.key()),
            Some(first_key)
        );

        let mut moved_start = start.clone();
        moved_start.players[0].start_tile_x = 2;
        let moved_key = ai.static_map_context.get_or_analyze(&moved_start).key();

        assert_ne!(moved_key, first_key);
        assert_eq!(
            ai.static_map_context.current().map(|context| context.key()),
            Some(moved_key)
        );

        let mut edited_terrain = moved_start.clone();
        edited_terrain.map.terrain[0] = terrain::ROCK;
        let edited_key = ai.static_map_context.get_or_analyze(&edited_terrain).key();

        assert_ne!(edited_key, moved_key);
        assert_eq!(
            ai.static_map_context.current().map(|context| context.key()),
            Some(edited_key)
        );
    }

    #[test]
    fn live_decision_trace_snapshot_is_bounded() {
        let long_line = "x".repeat(LIVE_DECISION_TRACE_MAX_LINE_CHARS + 8);
        let lines = std::iter::once(long_line)
            .chain((1..(LIVE_DECISION_TRACE_MAX_LINES + 3)).map(|index| format!("line={index}")))
            .collect();

        let bounded = bounded_decision_trace_lines(lines);

        assert_eq!(bounded.len(), LIVE_DECISION_TRACE_MAX_LINES);
        assert_eq!(bounded[0].len(), LIVE_DECISION_TRACE_MAX_LINE_CHARS);
        assert_eq!(
            bounded.last().map(String::as_str),
            Some(LIVE_DECISION_TRACE_TRUNCATED_LINE)
        );
    }

    #[test]
    fn live_stage_filter_sends_hold_position_once_per_staged_unit() {
        let mut ai = AiController::new(1);
        let intents = [crate::ai_core::decision::AiIntent::Stage { units: vec![42] }];
        let hold = SimCommand::HoldPosition {
            units: vec![42],
            queued: false,
        };

        let first = ai.filter_repeated_stage_commands(10, &intents, vec![hold.clone()]);
        let second = ai.filter_repeated_stage_commands(16, &intents, vec![hold]);

        assert_eq!(
            first,
            vec![SimCommand::HoldPosition {
                units: vec![42],
                queued: false,
            }]
        );
        assert!(second.is_empty());
    }

    #[test]
    fn live_adapter_knows_public_and_internal_profiles() {
        assert_eq!(LIVE_PROFILE_IDS, [AI_2_1_ID, JEFFS_AI_ID, AI_TURTLE_ID]);
    }

    #[test]
    fn live_default_is_ai_2_1() {
        assert_eq!(DEFAULT_LIVE_PROFILE_ID, AI_2_1_ID);
    }

    #[test]
    fn random_live_profile_selection_uses_the_full_internal_pool() {
        let mut rng = rand::rngs::SmallRng::seed_from_u64(0xA1);
        let mut selected = BTreeSet::new();
        for _ in 0..32 {
            selected.insert(random_live_profile_id(&mut rng));
        }
        assert_eq!(
            selected,
            BTreeSet::from([AI_2_1_ID, JEFFS_AI_ID, AI_TURTLE_ID])
        );
    }

    #[test]
    fn unknown_profile_id_falls_back_to_default_profile() {
        let ai = AiController::with_profile_id(2, "missing_profile");

        assert_eq!(ai.profile_id(), DEFAULT_LIVE_PROFILE_ID);
    }

    #[test]
    fn live_profile_aliases_are_bounded_to_supported_profiles() {
        assert_eq!(
            canonical_live_profile_id("ai"),
            Some(DEFAULT_LIVE_PROFILE_ID)
        );
        assert_eq!(
            canonical_live_profile_id("default"),
            Some(DEFAULT_LIVE_PROFILE_ID)
        );
        assert_eq!(canonical_live_profile_id(AI_2_1_ID), Some(AI_2_1_ID));
        assert_eq!(canonical_live_profile_id(JEFFS_AI_ID), Some(JEFFS_AI_ID));
        assert_eq!(canonical_live_profile_id(AI_TURTLE_ID), Some(AI_TURTLE_ID));
        assert_eq!(canonical_live_profile_id("unsupported_profile"), None);
    }

    #[test]
    fn live_profile_ids_resolve_to_their_canonical_match_profiles() {
        assert_eq!(resolve_live_profile_id_for_match(AI_2_1_ID), AI_2_1_ID);
        assert_eq!(resolve_live_profile_id_for_match(JEFFS_AI_ID), JEFFS_AI_ID);
        assert_eq!(
            resolve_live_profile_id_for_match(AI_TURTLE_ID),
            AI_TURTLE_ID
        );
    }

    #[test]
    fn live_profile_labels_match_lobby_selector_names() {
        assert_eq!(live_profile_label(AI_2_1_ID), "AI 2.1");
        assert_eq!(live_profile_label(JEFFS_AI_ID), "Jeff's AI");
        assert_eq!(live_profile_label(AI_TURTLE_ID), "AI Turtle");
        assert_eq!(live_profile_label("default"), "AI 2.1");
        assert_eq!(live_profile_label("unsupported_profile"), "AI");
        assert_eq!(live_profile_label("unknown"), "AI");
    }
}
