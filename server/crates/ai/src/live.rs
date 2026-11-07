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
    profile_by_id, runs_live_jeff, AiProfile, AI_2_1, AI_2_1_ID, AI_TURTLE_ID, JEFFS_AI_ID,
    JEFFS_AI_V1_1_ID, PRESSURE_AI_ID,
};
use crate::ai_shared;
use crate::compositions::{composition_by_id, AI_2_3_ID};
use crate::rush::{RushStrategy, RUSH_AI_ID};
use crate::sdk::{AiActions, AiFrame, AiStrategy, KnownBuildSite, WorldQueries};
use crate::selfplay::pending_build::PendingBuildTracker;
use crate::selfplay::player_view::PlayerView;
use rand::Rng;
use rts_protocol::{ObserverMapAnalysisDiagnostics, ObserverMapAnalysisLayer};
use rts_rules::math::GameplayMath as _;
use rts_sim::game::command::SimCommand;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::Game;
use rts_sim::protocol::{Snapshot, StartPayload};

mod mirror;
mod opening;
mod rush_handoff;

const DECISION_INTERVAL: u32 = 9;
const LIVE_DECISION_TRACE_MAX_LINES: usize = 24;
/// Jeff's AI V1.1: a fight is over for a unit with no visible enemy unit this close for this long.
const QUIET_FIGHT_TILES: f32 = 10.0;
const QUIET_FIGHT_TICKS: u32 = crate::config::TICK_HZ * 4;
const LIVE_DECISION_TRACE_MAX_LINE_CHARS: usize = 256;
const LIVE_DECISION_TRACE_TRUNCATED_LINE: &str = "trace_truncated=true";
const CUSTOM_STRATEGY_PROFILE_ID: &str = "custom_strategy";

/// The default live-lobby profile id.
pub const DEFAULT_LIVE_PROFILE_ID: &str = AI_2_3_ID;

/// Canonical profile ids understood by the live adapter. Experimental profiles are available to
/// internal observer-only sessions; the room actor prevents them from entering human matches.
pub const LIVE_PROFILE_IDS: [&str; 7] = [
    AI_2_3_ID,
    AI_2_1_ID,
    JEFFS_AI_ID,
    JEFFS_AI_V1_1_ID,
    AI_TURTLE_ID,
    RUSH_AI_ID,
    PRESSURE_AI_ID,
];

/// The pool random picks draw from: every live profile but Jeff's AI V1.0, which is chosen only by
/// name. Jeff's AI V1.1 holds V1.0's old place, so the random stream is unchanged.
const RANDOM_LIVE_PROFILE_IDS: [&str; 6] = [
    AI_2_3_ID,
    AI_2_1_ID,
    JEFFS_AI_V1_1_ID,
    AI_TURTLE_ID,
    RUSH_AI_ID,
    PRESSURE_AI_ID,
];

pub fn canonical_live_profile_id(input: &str) -> Option<&'static str> {
    match input {
        "ai" | "default" => Some(DEFAULT_LIVE_PROFILE_ID),
        id if composition_by_id(id).is_some() => composition_by_id(id).map(|c| c.id),
        AI_2_1_ID => Some(AI_2_1_ID),
        JEFFS_AI_ID => Some(JEFFS_AI_ID),
        JEFFS_AI_V1_1_ID => Some(JEFFS_AI_V1_1_ID),
        RUSH_AI_ID => Some(RUSH_AI_ID),
        PRESSURE_AI_ID => Some(PRESSURE_AI_ID),
        AI_TURTLE_ID => Some(AI_TURTLE_ID),
        _ => None,
    }
}

pub fn live_profile_label(profile_id: &str) -> &'static str {
    let canonical = canonical_live_profile_id(profile_id);
    if let Some(composition) = canonical.and_then(composition_by_id) {
        return composition.label;
    }
    match canonical {
        Some(AI_2_1_ID) => "AI 2.1",
        Some(JEFFS_AI_ID) => "Jeff's AI V1.0",
        Some(JEFFS_AI_V1_1_ID) => "Jeff's AI",
        Some(RUSH_AI_ID) => "Rush AI",
        Some(PRESSURE_AI_ID) => "Pressure AI",
        Some(AI_TURTLE_ID) => "AI Turtle",
        _ => "AI",
    }
}

pub fn is_player_live_profile_id(profile_id: &str) -> bool {
    matches!(
        profile_id,
        AI_2_3_ID | JEFFS_AI_ID | JEFFS_AI_V1_1_ID | RUSH_AI_ID | PRESSURE_AI_ID
    )
}

pub fn random_live_profile_id(rng: &mut impl Rng) -> &'static str {
    RANDOM_LIVE_PROFILE_IDS[rng.gen_range(0..RANDOM_LIVE_PROFILE_IDS.len() as u64) as usize]
}

pub fn resolve_live_profile_id_for_match(profile_id: &str) -> &'static str {
    canonical_live_profile_id(profile_id).unwrap_or(DEFAULT_LIVE_PROFILE_ID)
}

pub struct AiThinkContext<'a> {
    /// Host-owned seed used only for once-per-match behavior selection.
    pub match_seed: u32,
    pub start: &'a StartPayload,
    pub snapshot: &'a Snapshot,
    pub alive_player_ids: &'a [u32],
    /// Recipient-scoped cumulative loss counts from Game::known_losses_for.
    pub known_losses: BTreeMap<u32, BTreeMap<rts_rules::EntityKind, u32>>,
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
                known_losses: game.known_losses_for(player_id),
                match_seed: game.seed(),
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
    behavior_profile_id: &'static str,
    profile_id: &'static str,
    selection_pending: bool,
    memory: AiDecisionMemory,
    static_map_context: AiStaticMapContextCache,
    pending_builds: PendingBuildTracker,
    staged_units: BTreeSet<u32>,
    held_stage_units: BTreeSet<u32>,
    active_attack_units: BTreeMap<u32, u32>,
    /// Jeff's AI V1.1: since when each attack-held unit has had no enemy near (`release_quiet_fights`).
    quiet_since: BTreeMap<u32, u32>,
    last_decision_trace: Option<AiDecisionTraceSnapshot>,
    last_debug_map_layers: Vec<ObserverMapAnalysisLayer>,
    custom_strategy: Option<Box<dyn AiStrategy>>,
    strategy_initialized: bool,
    pressure_waves: crate::pressure::PressureWaves,
    retired_barracks: BTreeSet<u32>,
    rush_strategy: RushStrategy,
    opening: opening::OpeningProgress,
    /// Jeff's AI V1.1 (`mirror`): the frame he decides in off the canonical start, worked out on
    /// the match's first think.
    mirror: Option<Option<mirror::Mirror>>,
}

struct LegacyProfileStrategy;

impl LegacyProfileStrategy {
    fn project(frame: &AiFrame) -> Option<AiObservation> {
        AiObservation::from_frame(frame)
    }
}

impl AiController {
    #[cfg(all(test, not(debug_assertions)))]
    pub(crate) fn decision_memory(&self) -> &AiDecisionMemory {
        &self.memory
    }

    pub fn new(player: u32) -> Self {
        Self::with_profile_id(player, DEFAULT_LIVE_PROFILE_ID)
    }

    pub fn with_profile_id(player: u32, profile_id: &'static str) -> Self {
        let profile_id = canonical_live_profile_id(profile_id)
            .or_else(|| profile_by_id(profile_id).map(|profile| profile.id))
            .unwrap_or(DEFAULT_LIVE_PROFILE_ID);
        let composition = composition_by_id(profile_id);
        let profile = profile_by_id(profile_id).unwrap_or_else(legacy_fallback_profile);
        Self {
            player,
            behavior_profile_id: if profile_id == RUSH_AI_ID {
                RUSH_AI_ID
            } else {
                profile.id
            },
            profile_id,
            selection_pending: composition.is_some(),
            memory: AiDecisionMemory::for_profile(profile),
            static_map_context: AiStaticMapContextCache::default(),
            pending_builds: PendingBuildTracker::default(),
            staged_units: BTreeSet::new(),
            held_stage_units: BTreeSet::new(),
            active_attack_units: BTreeMap::new(),
            quiet_since: BTreeMap::new(),
            last_decision_trace: None,
            last_debug_map_layers: Vec::new(),
            custom_strategy: None,
            strategy_initialized: false,
            pressure_waves: Default::default(),
            retired_barracks: BTreeSet::new(),
            rush_strategy: Default::default(),
            opening: Default::default(),
            mirror: None,
        }
    }

    /// Constructs a custom strategy in the same cadence, fog, validation, and replay envelope as
    /// built-in profiles.
    pub fn with_strategy(player: u32, strategy: Box<dyn AiStrategy>) -> Self {
        let mut controller = Self::new(player);
        controller.behavior_profile_id = CUSTOM_STRATEGY_PROFILE_ID;
        controller.profile_id = CUSTOM_STRATEGY_PROFILE_ID;
        controller.selection_pending = false;
        controller.custom_strategy = Some(strategy);
        controller
    }

    pub fn player_id(&self) -> u32 {
        self.player
    }

    /// Canonical requested identity; a composition keeps its identity after selecting a member.
    pub fn profile_id(&self) -> &'static str {
        self.profile_id
    }

    /// The concrete behavior chosen for this match, after the first host invocation.
    pub fn selected_profile_id(&self) -> Option<&'static str> {
        (!self.selection_pending).then_some(self.behavior_profile_id)
    }

    fn select_for_match(&mut self, seed: u32) {
        if !self.selection_pending {
            return;
        }
        let requested = self.profile_id;
        let selected = composition_by_id(requested)
            .and_then(|composition| composition.select(seed, self.player))
            .unwrap_or(AI_2_1_ID);
        *self = Self::with_profile_id(self.player, selected);
        self.profile_id = requested;
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
        profile_by_id(self.behavior_profile_id).unwrap_or_else(legacy_fallback_profile)
    }

    pub fn think(&mut self, context: AiThinkContext<'_>) -> Vec<SimCommand> {
        self.select_for_match(context.match_seed);
        if self.mirror.is_none() {
            let on = crate::ai_core::profiles::is_jeff_v1_1(self.behavior_profile_id)
                && !crate::ai_core::profiles::jeff_v1_1_feature_off("mirror");
            self.mirror = Some(if on {
                mirror::mirror_for(context.start, self.player)
            } else {
                None
            });
        }
        if let Some(Some(mirror)) = self.mirror.clone() {
            return self.think_mirrored(&mirror, context);
        }
        if matches!(self.behavior_profile_id, RUSH_AI_ID | PRESSURE_AI_ID) {
            self.opening.observe(context.snapshot, self.player);
        }
        let mut commands = self.handoff_rush(&context);
        commands.extend(self.think_active(context));
        commands.retain_mut(|command| match command {
            SimCommand::Train { building, .. } => !self.retired_barracks.contains(building),
            SimCommand::AdjustProductionRepeat {
                buildings, delta, ..
            } if *delta > 0 => {
                buildings.retain(|id| !self.retired_barracks.contains(id));
                !buildings.is_empty()
            }
            _ => true,
        });
        commands
    }

    /// Jeff's AI V1.1 (`mirror`) off the canonical start: decides on the map and snapshot mapped
    /// into the canonical frame and maps his orders back. The host's worker retreats are real-frame
    /// orders and pass straight through.
    fn think_mirrored(
        &mut self,
        mirror: &mirror::Mirror,
        context: AiThinkContext<'_>,
    ) -> Vec<SimCommand> {
        let mut commands = context.retreat_commands;
        let tick = context.snapshot.tick;
        if !tick
            .wrapping_add(self.player)
            .is_multiple_of(DECISION_INTERVAL)
        {
            return commands;
        }
        let snapshot = mirror.frame.snapshot(context.snapshot);
        let inner = AiThinkContext {
            match_seed: context.match_seed,
            start: &mirror.start,
            snapshot: &snapshot,
            alive_player_ids: context.alive_player_ids,
            known_losses: context.known_losses,
            retreat_commands: Vec::new(),
        };
        let mut decided = self.think_active(inner);
        decided.retain_mut(|command| match command {
            SimCommand::Train { building, .. } => !self.retired_barracks.contains(building),
            SimCommand::AdjustProductionRepeat {
                buildings, delta, ..
            } if *delta > 0 => {
                buildings.retain(|id| !self.retired_barracks.contains(id));
                !buildings.is_empty()
            }
            _ => true,
        });
        commands.extend(
            decided
                .into_iter()
                .map(|command| mirror.frame.command(command)),
        );
        commands
    }

    fn think_active(&mut self, context: AiThinkContext<'_>) -> Vec<SimCommand> {
        // Rush deliberately keeps its Engineer advancing, including under fire.
        let mut commands = if self.behavior_profile_id == RUSH_AI_ID {
            Vec::new()
        } else {
            context.retreat_commands
        };
        let tick = context.snapshot.tick;
        let static_map_context =
            if self.custom_strategy.is_none() && self.behavior_profile_id != RUSH_AI_ID {
                Some(
                    self.static_map_context
                        .get_or_analyze(context.start)
                        .clone(),
                )
            } else {
                None
            };
        if !tick
            .wrapping_add(self.player)
            .is_multiple_of(DECISION_INTERVAL)
        {
            return commands;
        }

        let view = PlayerView {
            known_losses: &context.known_losses,
            match_seed: context.match_seed,
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
            pending_builds.iter().copied().map(Into::into),
            Some(context.alive_player_ids),
        ) else {
            return commands;
        };

        let frame = frame.with_losses(&context.known_losses);
        // The start payload cannot change during this invocation. Reuse one validated context
        // rather than rehashing its terrain, overlays, starts, and resources for each consumer.
        let static_map_context = static_map_context.unwrap_or_else(|| {
            self.static_map_context
                .get_or_analyze(context.start)
                .clone()
        });
        let frame = frame.with_base_sites(static_map_context.base_sites());

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

        let strategy: Option<&mut dyn AiStrategy> = if self.behavior_profile_id == RUSH_AI_ID {
            Some(&mut self.rush_strategy)
        } else {
            self.custom_strategy
                .as_mut()
                .map(|strategy| strategy.as_mut() as &mut dyn AiStrategy)
        };
        if let Some(strategy) = strategy {
            if !self.strategy_initialized {
                strategy.initialize(&frame);
                self.strategy_initialized = true;
            }
            let mut actions = AiActions::for_frame(&frame);
            strategy.step(&frame, &mut actions);
            commands.extend(
                actions
                    .into_requests()
                    .into_iter()
                    .map(crate::action_emitter::emit_request),
            );
            self.pending_builds.record_commands(tick, &commands);
            if self.profile_id != self.behavior_profile_id {
                self.last_decision_trace = Some(AiDecisionTraceSnapshot {
                    player_id: self.player,
                    profile_id: self.profile_id,
                    trace_tick: tick,
                    lines: vec![
                        format!("profile={} tick={tick}", self.profile_id),
                        format!(
                            "composition={} selected={}",
                            self.profile_id, self.behavior_profile_id
                        ),
                    ],
                });
            }
            return commands;
        }

        let Some(mut observation) = LegacyProfileStrategy::project(&frame) else {
            return commands;
        };
        // Keep retired structures in economy/defense facts, but never budget manual training
        // against a producer whose commands would be suppressed after the handoff.
        for entity in &mut observation.owned {
            if self.retired_barracks.contains(&entity.id) {
                entity.production_queue_len = None;
            }
        }
        self.prune_combat_memory(&observation, tick);

        let profile = self.profile();
        // Jeff's natural security retries its natural site after a dropped order and gives it up
        // after two. Excluding the exact footprint after the first lost The River's fixed natural
        // for the rest of the game, so Jeff's own rule decides.
        if runs_live_jeff(profile.id) {
            if let Some(site) = self.memory.natural_site() {
                self.pending_builds
                    .forget_failed(EntityKind::ResourceDepot, site);
            }
        }
        let failed_builds = &self.pending_builds;
        let world_queries = WorldQueries::new(&frame);
        let build_site_exclusions = failed_builds.known_build_site_exclusions(&world_queries);
        let map_analysis = static_map_context.analysis();
        self.last_debug_map_layers =
            observer_debug_map_layers_for_profile(&observation, map_analysis, profile);
        self.memory
            .note_losses(frame.own_losses(), frame.observed_enemy_losses());
        let decision = decide_profile_with_analysis(
            &observation,
            profile,
            &mut self.memory,
            map_analysis,
            &world_queries,
            ai_shared::BuildSearch {
                min_radius: 2,
                max_radius: ai_shared::DEFAULT_BUILD_SEARCH_MAX_RADIUS,
                prefer_away_from_center: false,
                prefer_toward_center: false,
            },
            |building, tile_x, tile_y| {
                world_queries.tile(tile_x, tile_y).is_some_and(|tile| {
                    world_queries.known_build_site_compatibility(
                        building,
                        tile,
                        &build_site_exclusions,
                    ) == KnownBuildSite::NoKnownConflict
                })
            },
        );
        debug_assert_eq!(decision.profile_id, self.behavior_profile_id);

        let mut trace_lines = decision.trace.format_lines();
        if self.profile_id != self.behavior_profile_id {
            trace_lines.insert(
                0,
                format!(
                    "composition={} selected={}",
                    self.profile_id, self.behavior_profile_id
                ),
            );
        }
        self.last_decision_trace = Some(AiDecisionTraceSnapshot {
            player_id: self.player,
            profile_id: self.profile_id,
            trace_tick: tick,
            lines: bounded_decision_trace_lines(trace_lines),
        });
        let factory_clearance_ordered = decision
            .intents
            .iter()
            .any(|intent| matches!(intent, crate::ai_core::decision::AiIntent::Move { .. }))
            && decision.intents.iter().any(|intent| {
                matches!(
                    intent,
                    crate::ai_core::decision::AiIntent::Build {
                        kind: EntityKind::Factory
                    }
                )
            });
        commands.extend(self.filter_repeated_stage_commands(
            tick,
            &decision.intents,
            decision.commands,
        ));
        if self.behavior_profile_id == PRESSURE_AI_ID {
            if let Some(enemy) = frame
                .players()
                .iter()
                .find(|p| p.team_id != frame.team_id() && p.is_alive)
            {
                let size = frame.map().tile_size as f32;
                let target = (
                    (enemy.start_tile.0 as f32 + 0.5) * size,
                    (enemy.start_tile.1 as f32 + 0.5) * size,
                );
                self.pressure_waves.rally(&frame, enemy.id, target);
                let mut actions = AiActions::for_frame(&frame);
                self.pressure_waves.step(
                    &frame,
                    target,
                    &decision.pressure_wave_units,
                    self.opening.trained_riflemen(),
                    &mut actions,
                );
                commands.extend(
                    actions
                        .into_requests()
                        .into_iter()
                        .map(crate::action_emitter::emit_request),
                );
            }
        }
        self.pending_builds.record_commands_with_factory_clearance(
            tick,
            &commands,
            factory_clearance_ordered,
        );
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
        if crate::ai_core::profiles::is_jeff_v1_1(self.behavior_profile_id)
            && !crate::ai_core::profiles::jeff_v1_1_feature_off("quiet")
        {
            self.release_quiet_fights(observation, tick);
        }
    }

    /// Jeff's AI V1.1 sends a defender back to its post once its fight is over, when no visible
    /// enemy unit has come within `QUIET_FIGHT_TILES` of it for `QUIET_FIGHT_TICKS`. An attack order
    /// otherwise holds it out of staging for up to two minutes, and the units that won a fight stood
    /// where it ended: none was back on its post 120 s after a fight.
    fn release_quiet_fights(&mut self, observation: &AiObservation, tick: u32) {
        let reach = QUIET_FIGHT_TILES * observation.map.tile_size as f32;
        let quiet: BTreeSet<u32> = observation
            .owned
            .iter()
            .filter(|unit| self.active_attack_units.contains_key(&unit.id))
            .filter(|unit| {
                !observation.visible_enemies.iter().any(|enemy| {
                    enemy.kind.is_unit() && (enemy.x - unit.x).det_hypot(enemy.y - unit.y) <= reach
                })
            })
            .map(|unit| unit.id)
            .collect();
        self.quiet_since.retain(|id, _| quiet.contains(id));
        for id in quiet {
            let since = *self.quiet_since.entry(id).or_insert(tick);
            if tick.saturating_sub(since) >= QUIET_FIGHT_TICKS {
                self.active_attack_units.remove(&id);
                self.quiet_since.remove(&id);
            }
        }
    }

    fn filter_repeated_stage_commands(
        &mut self,
        tick: u32,
        intents: &[crate::ai_core::decision::AiIntent],
        commands: Vec<SimCommand>,
    ) -> Vec<SimCommand> {
        let mut attacking = BTreeSet::new();
        let mut staging = BTreeSet::new();
        let mut assembling = BTreeSet::new();
        for intent in intents {
            match intent {
                crate::ai_core::decision::AiIntent::Attack { units } => {
                    attacking.extend(units.iter().copied())
                }
                crate::ai_core::decision::AiIntent::Stage { units } => {
                    staging.extend(units.iter().copied())
                }
                crate::ai_core::decision::AiIntent::Assemble { units } => {
                    assembling.extend(units.iter().copied())
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
        // Assembly supplies bounded repositioning/hold commands, including changed destinations.
        // Clear stale stage and attack suppression before filtering, and do not cache assembly as
        // an ordinary stage: defense must still be able to retask the cohort when assembly ends.
        for id in &assembling {
            self.staged_units.remove(id);
            self.held_stage_units.remove(id);
            self.active_attack_units.remove(id);
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
                        .filter(|id| assembling.contains(id) || !self.staged_units.contains(id))
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
                        .filter(|id| assembling.contains(id) || !self.staged_units.contains(id))
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
                        .filter(|id| assembling.contains(id) || !self.held_stage_units.contains(id))
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
        for id in &assembling {
            self.staged_units.remove(id);
            self.held_stage_units.remove(id);
        }
        filtered
    }
}

fn legacy_fallback_profile() -> &'static AiProfile {
    profile_by_id(AI_2_1_ID).unwrap_or(&AI_2_1)
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
#[path = "live/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "live/composition_tests.rs"]
mod composition_tests;

#[cfg(test)]
#[path = "live/v1_1_tests.rs"]
mod v1_1_tests;

#[cfg(test)]
#[path = "live/losses_tests.rs"]
mod losses_tests;

#[cfg(test)]
#[path = "live/map_context_tests.rs"]
mod map_context_tests;
