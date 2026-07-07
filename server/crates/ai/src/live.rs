//! Live gameplay AI adapter. See `docs/design/ai.md`.
//!
//! The room task invokes controllers before `Game::tick()`, using the same fog-filtered snapshot
//! surface a player would receive. Controllers emit ordinary [`SimCommand`]s; the authoritative
//! simulation validates and records them exactly like human commands.

use std::collections::{BTreeMap, BTreeSet};

use crate::ai_core::decision::{decide_profile, AiDecisionMemory};
use crate::ai_core::observation::AiObservation;
use crate::ai_core::profiles::{
    profile_by_id, AiProfile, AI_1_0_TECH, AI_1_0_TECH_ID, AI_1_1_TANK_MG_ID,
    AI_1_2_WAVE_COHORTS_ID,
};
use crate::ai_shared;
use crate::selfplay::pending_build::PendingBuildTracker;
use crate::selfplay::player_view::{
    footprint_placeable_from_snapshot, occupied_tiles_from_snapshot, PlayerView,
};
use rand::Rng;
use rts_sim::game::command::SimCommand;
use rts_sim::protocol::{Snapshot, StartPayload};

const DECISION_INTERVAL: u32 = 9;
const LIVE_DECISION_TRACE_MAX_LINES: usize = 24;
const LIVE_DECISION_TRACE_MAX_LINE_CHARS: usize = 256;
const LIVE_DECISION_TRACE_TRUNCATED_LINE: &str = "trace_truncated=true";

/// Default live-lobby profile. Keep this on the highest supported live AI version.
pub const DEFAULT_LIVE_PROFILE_ID: &str = AI_1_2_WAVE_COHORTS_ID;

/// Profiles available to ordinary lobby AI opponents.
pub const LIVE_PROFILE_IDS: [&str; 3] = [
    AI_1_0_TECH_ID,
    AI_1_1_TANK_MG_ID,
    AI_1_2_WAVE_COHORTS_ID,
];

pub fn canonical_live_profile_id(input: &str) -> Option<&'static str> {
    match input {
        "ai" | "default" => Some(DEFAULT_LIVE_PROFILE_ID),
        "ai1" | "ai_1_0" | "ai_1_0_tech" => Some(AI_1_0_TECH_ID),
        "ai_1_1" | "ai11" | "ai_1_1_tank_mg" => Some(AI_1_1_TANK_MG_ID),
        "ai_1_2" | "ai12" | "ai_1_2_wave_cohorts" => Some(AI_1_2_WAVE_COHORTS_ID),
        _ => None,
    }
}

pub fn random_live_profile_id(rng: &mut impl Rng) -> &'static str {
    LIVE_PROFILE_IDS[rng.gen_range(0..LIVE_PROFILE_IDS.len())]
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
    pending_builds: PendingBuildTracker,
    staged_units: BTreeSet<u32>,
    active_attack_units: BTreeMap<u32, u32>,
    last_decision_trace: Option<AiDecisionTraceSnapshot>,
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
            pending_builds: PendingBuildTracker::default(),
            staged_units: BTreeSet::new(),
            active_attack_units: BTreeMap::new(),
            last_decision_trace: None,
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

    fn profile(&self) -> &'static AiProfile {
        profile_by_id(self.profile_id).unwrap_or_else(default_live_profile)
    }

    pub fn think(&mut self, context: AiThinkContext<'_>) -> Vec<SimCommand> {
        let mut commands = context.retreat_commands;
        let tick = context.snapshot.tick;
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
        let decision = decide_profile(
            &observation,
            profile,
            &mut self.memory,
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
                | crate::ai_core::decision::AiIntent::Train { .. }
                | crate::ai_core::decision::AiIntent::Research { .. }
                | crate::ai_core::decision::AiIntent::Gather { .. } => {}
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
                SimCommand::AttackMove {
                    units,
                    x,
                    y,
                    queued,
                } if units.iter().any(|id| staging.contains(id)) => {
                    let fresh: Vec<u32> = units
                        .into_iter()
                        .filter(|id| !self.staged_units.contains(id))
                        .filter(|id| !self.active_attack_units.contains_key(id))
                        .collect();
                    self.staged_units.extend(fresh.iter().copied());
                    if !fresh.is_empty() {
                        filtered.push(SimCommand::AttackMove {
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

    #[test]
    fn live_controller_uses_default_profile_id() {
        let ai = AiController::new(2);

        assert_eq!(ai.player_id(), 2);
        assert_eq!(ai.profile_id(), AI_1_2_WAVE_COHORTS_ID);
        assert_eq!(ai.latest_decision_trace(), None);
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
    fn live_profile_pool_exposes_supported_lobby_profiles() {
        assert_eq!(
            LIVE_PROFILE_IDS,
            [
                AI_1_0_TECH_ID,
                AI_1_1_TANK_MG_ID,
                AI_1_2_WAVE_COHORTS_ID
            ]
        );
    }

    #[test]
    fn live_default_tracks_highest_semantic_profile_version() {
        let highest = LIVE_PROFILE_IDS
            .iter()
            .copied()
            .max_by_key(|profile_id| semantic_version_parts(profile_id))
            .unwrap();

        assert_eq!(DEFAULT_LIVE_PROFILE_ID, highest);
    }

    #[test]
    fn random_live_profile_selection_uses_live_pool() {
        let mut rng = rand::rngs::SmallRng::seed_from_u64(0xA1);
        for _ in 0..32 {
            let selected = random_live_profile_id(&mut rng);
            assert!(LIVE_PROFILE_IDS.contains(&selected));
        }
    }

    #[test]
    fn unknown_profile_id_falls_back_to_default_profile() {
        let ai = AiController::with_profile_id(2, "missing_profile");

        assert_eq!(ai.profile_id(), DEFAULT_LIVE_PROFILE_ID);
    }

    #[test]
    fn live_profile_aliases_are_bounded_to_supported_profiles() {
        assert_eq!(canonical_live_profile_id("ai"), Some(DEFAULT_LIVE_PROFILE_ID));
        assert_eq!(
            canonical_live_profile_id("default"),
            Some(DEFAULT_LIVE_PROFILE_ID)
        );
        assert_eq!(canonical_live_profile_id("ai_1_0"), Some(AI_1_0_TECH_ID));
        assert_eq!(
            canonical_live_profile_id("ai_1_1"),
            Some(AI_1_1_TANK_MG_ID)
        );
        assert_eq!(
            canonical_live_profile_id("ai_1_2"),
            Some(AI_1_2_WAVE_COHORTS_ID)
        );
        assert_eq!(canonical_live_profile_id("rifle_flood_fast"), None);
    }

    fn semantic_version_parts(profile_id: &str) -> Vec<u32> {
        let Some(rest) = profile_id.strip_prefix("ai_") else {
            return vec![0];
        };
        rest.split('_')
            .take_while(|part| part.chars().all(|ch| ch.is_ascii_digit()))
            .map(|part| part.parse::<u32>().unwrap_or(0))
            .collect()
    }
}
