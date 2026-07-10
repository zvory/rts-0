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
use crate::ai_core::profile_suites::{
    canonical_profile_request_id, resolve_profile_request_id, AI_1_0_SUITE_ID, AI_1_1_SUITE_ID,
    AI_1_2_SUITE_ID, AI_2_0_SUITE_ID, AI_2_1_SUITE_ID, AI_TURTLE_SUITE_ID,
};
use crate::ai_core::profiles::{
    profile_by_id, AiProfile, AI_1_0_TECH, AI_1_0_TECH_ID, AI_1_1_TANK_MG_ID,
    AI_1_2_WAVE_COHORTS_ID, AI_2_0_TANK_PRESSURE_ID, AI_2_1_ECONOMY_MANAGER_ID,
    AI_TURTLE_CHOKES_ID,
};
use crate::ai_shared;
use crate::selfplay::pending_build::PendingBuildTracker;
use crate::selfplay::player_view::{
    footprint_placeable_from_snapshot, occupied_tiles_from_snapshot, PlayerView,
};
use rand::Rng;
use rts_protocol::{ObserverMapAnalysisDiagnostics, ObserverMapAnalysisLayer};
use rts_sim::game::command::SimCommand;
use rts_sim::protocol::{Snapshot, StartPayload};

const DECISION_INTERVAL: u32 = 9;
const LIVE_DECISION_TRACE_MAX_LINES: usize = 24;
const LIVE_DECISION_TRACE_MAX_LINE_CHARS: usize = 256;
const LIVE_DECISION_TRACE_TRUNCATED_LINE: &str = "trace_truncated=true";

/// Default live-lobby profile. Keep this on the stable promoted live AI unless intentionally
/// rolling the ordinary Add AI path forward.
pub const DEFAULT_LIVE_PROFILE_ID: &str = AI_1_2_WAVE_COHORTS_ID;
pub const DEFAULT_LIVE_PROFILE_REQUEST_ID: &str = AI_1_2_SUITE_ID;

/// Profile or suite requests available to ordinary lobby AI opponents.
pub const LIVE_PROFILE_REQUEST_IDS: [&str; 6] = [
    AI_1_0_SUITE_ID,
    AI_1_1_SUITE_ID,
    AI_1_2_SUITE_ID,
    AI_2_0_SUITE_ID,
    AI_2_1_SUITE_ID,
    AI_TURTLE_SUITE_ID,
];

pub fn canonical_live_profile_request_id(input: &str) -> Option<&'static str> {
    match input {
        "ai" | "default" => Some(DEFAULT_LIVE_PROFILE_REQUEST_ID),
        value => canonical_profile_request_id(value),
    }
}

pub fn live_profile_label(profile_or_request_id: &str) -> &'static str {
    match canonical_live_profile_request_id(profile_or_request_id) {
        Some(AI_1_0_SUITE_ID) | Some(AI_1_0_TECH_ID) => "AI 1.0",
        Some(AI_1_1_SUITE_ID) | Some(AI_1_1_TANK_MG_ID) => "AI 1.1",
        Some(AI_1_2_SUITE_ID) | Some(AI_1_2_WAVE_COHORTS_ID) => "AI 1.2",
        Some(AI_2_0_SUITE_ID) | Some(AI_2_0_TANK_PRESSURE_ID) => "AI 2.0",
        Some(AI_2_1_SUITE_ID) | Some(AI_2_1_ECONOMY_MANAGER_ID) => "AI 2.1",
        Some(AI_TURTLE_SUITE_ID) | Some(AI_TURTLE_CHOKES_ID) => "AI Turtle",
        _ => "AI",
    }
}

pub fn random_live_profile_request_id(rng: &mut impl Rng) -> &'static str {
    LIVE_PROFILE_REQUEST_IDS[rng.gen_range(0..LIVE_PROFILE_REQUEST_IDS.len())]
}

pub fn resolve_live_profile_id_for_match(
    request_id: &str,
    seed: u32,
    player_id: u32,
) -> &'static str {
    let request_id =
        canonical_live_profile_request_id(request_id).unwrap_or(DEFAULT_LIVE_PROFILE_REQUEST_ID);
    resolve_profile_request_id(request_id, seed, u64::from(player_id))
        .unwrap_or(DEFAULT_LIVE_PROFILE_ID)
}

pub struct AiThinkContext<'a> {
    pub start: &'a StartPayload,
    pub snapshot: &'a Snapshot,
    pub alive_player_ids: &'a [u32],
    pub retreat_commands: Vec<SimCommand>,
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
        }
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
        self.static_map_context.get_or_analyze(context.start);
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
        let Some(observation) = AiObservation::from_snapshot_with_alive(
            context.start,
            context.snapshot,
            self.player,
            self.pending_builds.intents(),
            Some(context.alive_player_ids),
        ) else {
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
        commands.extend(self.filter_repeated_stage_commands(
            tick,
            &decision.intents,
            decision.commands,
        ));
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
                SimCommand::HoldPosition { units } if command_stages_units(&units) => {
                    let fresh: Vec<u32> = units
                        .into_iter()
                        .filter(|id| !self.active_attack_units.contains_key(id))
                        .filter(|id| !self.held_stage_units.contains(id))
                        .collect();
                    self.staged_units.extend(fresh.iter().copied());
                    self.held_stage_units.extend(fresh.iter().copied());
                    if !fresh.is_empty() {
                        filtered.push(SimCommand::HoldPosition { units: fresh });
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

fn default_live_profile() -> &'static AiProfile {
    profile_by_id(DEFAULT_LIVE_PROFILE_ID).unwrap_or(&AI_1_0_TECH)
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
            },
            players: vec![PlayerStart {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "P1".to_string(),
                color: "#111".to_string(),
                start_tile_x: 1,
                start_tile_y: 1,
            }],
        }
    }

    #[test]
    fn live_controller_uses_default_profile_id() {
        let ai = AiController::new(2);

        assert_eq!(ai.player_id(), 2);
        assert_eq!(ai.profile_id(), AI_1_2_WAVE_COHORTS_ID);
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
        let hold = SimCommand::HoldPosition { units: vec![42] };

        let first = ai.filter_repeated_stage_commands(10, &intents, vec![hold.clone()]);
        let second = ai.filter_repeated_stage_commands(16, &intents, vec![hold]);

        assert_eq!(first, vec![SimCommand::HoldPosition { units: vec![42] }]);
        assert!(second.is_empty());
    }

    #[test]
    fn live_profile_request_pool_exposes_supported_lobby_choices() {
        assert_eq!(
            LIVE_PROFILE_REQUEST_IDS,
            [
                AI_1_0_SUITE_ID,
                AI_1_1_SUITE_ID,
                AI_1_2_SUITE_ID,
                AI_2_0_SUITE_ID,
                AI_2_1_SUITE_ID,
                AI_TURTLE_SUITE_ID,
            ]
        );
    }

    #[test]
    fn live_default_stays_on_stable_promoted_profile() {
        assert_eq!(DEFAULT_LIVE_PROFILE_ID, AI_1_2_WAVE_COHORTS_ID);
        assert_eq!(DEFAULT_LIVE_PROFILE_REQUEST_ID, AI_1_2_SUITE_ID);
        assert!(LIVE_PROFILE_REQUEST_IDS.contains(&AI_2_0_SUITE_ID));
        assert!(LIVE_PROFILE_REQUEST_IDS.contains(&AI_2_1_SUITE_ID));
        assert!(LIVE_PROFILE_REQUEST_IDS.contains(&AI_TURTLE_SUITE_ID));
    }

    #[test]
    fn random_live_profile_request_selection_uses_live_pool() {
        let mut rng = rand::rngs::SmallRng::seed_from_u64(0xA1);
        for _ in 0..32 {
            let selected = random_live_profile_request_id(&mut rng);
            assert!(LIVE_PROFILE_REQUEST_IDS.contains(&selected));
        }
    }

    #[test]
    fn unknown_profile_id_falls_back_to_default_profile() {
        let ai = AiController::with_profile_id(2, "missing_profile");

        assert_eq!(ai.profile_id(), DEFAULT_LIVE_PROFILE_ID);
    }

    #[test]
    fn live_profile_request_aliases_are_bounded_to_supported_profiles() {
        assert_eq!(
            canonical_live_profile_request_id("ai"),
            Some(DEFAULT_LIVE_PROFILE_REQUEST_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("default"),
            Some(DEFAULT_LIVE_PROFILE_REQUEST_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("ai_1_0"),
            Some(AI_1_0_SUITE_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("ai_1_0_tech"),
            Some(AI_1_0_TECH_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("ai_1_1"),
            Some(AI_1_1_SUITE_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("ai_1_1_tank_mg"),
            Some(AI_1_1_TANK_MG_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("ai_1_2"),
            Some(AI_1_2_SUITE_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("ai_1_2_wave_cohorts"),
            Some(AI_1_2_WAVE_COHORTS_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("ai_2_0"),
            Some(AI_2_0_SUITE_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("ai20"),
            Some(AI_2_0_SUITE_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("ai_2_1"),
            Some(AI_2_1_SUITE_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("ai21"),
            Some(AI_2_1_SUITE_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("ai_turtle"),
            Some(AI_TURTLE_SUITE_ID)
        );
        assert_eq!(
            canonical_live_profile_request_id("turtle"),
            Some(AI_TURTLE_SUITE_ID)
        );
        assert_eq!(canonical_live_profile_request_id("ai_2_0_agent_rush"), None);
        assert_eq!(canonical_live_profile_request_id("ai_2_0_rifle_tank"), None);
        assert_eq!(canonical_live_profile_request_id("rifle_flood_fast"), None);
    }

    #[test]
    fn live_suite_requests_resolve_to_concrete_match_profiles() {
        assert_eq!(
            resolve_live_profile_id_for_match(AI_1_2_SUITE_ID, 8, 2),
            AI_1_2_WAVE_COHORTS_ID
        );
        assert_eq!(
            resolve_live_profile_id_for_match(AI_2_0_SUITE_ID, 8, 2),
            AI_2_0_TANK_PRESSURE_ID
        );
        assert_eq!(
            resolve_live_profile_id_for_match(AI_2_1_SUITE_ID, 8, 2),
            AI_2_1_ECONOMY_MANAGER_ID
        );
        assert_eq!(
            resolve_live_profile_id_for_match(AI_TURTLE_SUITE_ID, 8, 2),
            AI_TURTLE_CHOKES_ID
        );
    }

    #[test]
    fn live_profile_labels_match_lobby_selector_names() {
        assert_eq!(live_profile_label(AI_1_0_TECH_ID), "AI 1.0");
        assert_eq!(live_profile_label(AI_1_1_TANK_MG_ID), "AI 1.1");
        assert_eq!(live_profile_label(AI_1_2_WAVE_COHORTS_ID), "AI 1.2");
        assert_eq!(live_profile_label(AI_2_0_SUITE_ID), "AI 2.0");
        assert_eq!(live_profile_label(AI_2_0_TANK_PRESSURE_ID), "AI 2.0");
        assert_eq!(live_profile_label(AI_2_1_SUITE_ID), "AI 2.1");
        assert_eq!(live_profile_label(AI_2_1_ECONOMY_MANAGER_ID), "AI 2.1");
        assert_eq!(live_profile_label(AI_TURTLE_SUITE_ID), "AI Turtle");
        assert_eq!(live_profile_label(AI_TURTLE_CHOKES_ID), "AI Turtle");
        assert_eq!(live_profile_label("default"), "AI 1.2");
        assert_eq!(live_profile_label("unknown"), "AI");
    }
}
