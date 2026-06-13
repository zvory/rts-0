//! Live gameplay AI adapter. See `docs/design/ai.md`.
//!
//! The room task invokes controllers before `Game::tick()`, using the same fog-filtered snapshot
//! surface a player would receive. Controllers emit ordinary [`SimCommand`]s; the authoritative
//! simulation validates and records them exactly like human commands.

use std::collections::{BTreeMap, BTreeSet};

use crate::ai_core::decision::{decide_profile, AiDecisionMemory};
use crate::ai_core::observation::AiObservation;
use crate::ai_core::profiles::{
    profile_by_id, AiProfile, RIFLE_FLOOD_FULL_SATURATION, RIFLE_FLOOD_FULL_SATURATION_ID,
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

/// Default live-lobby profile. This preserves the current macro-focused AI behavior better than
/// the faster pressure profile, while still selecting from the canonical shared profile ids.
pub const DEFAULT_LIVE_PROFILE_ID: &str = RIFLE_FLOOD_FULL_SATURATION_ID;

/// Profiles available to ordinary lobby AI opponents. The names map to player-facing behaviors:
/// tank rush, proxy rush, and the previous rifle saturation strategy.
const LIVE_PROFILE_IDS: [&str; 1] = [RIFLE_FLOOD_FULL_SATURATION_ID];

pub fn random_live_profile_id(rng: &mut impl Rng) -> &'static str {
    LIVE_PROFILE_IDS[rng.gen_range(0..LIVE_PROFILE_IDS.len())]
}

pub struct AiThinkContext<'a> {
    pub start: &'a StartPayload,
    pub snapshot: &'a Snapshot,
    pub alive_player_ids: &'a [u32],
    pub retreat_commands: Vec<SimCommand>,
}

/// Drives a single AI-controlled player by emitting ordinary commands each think.
pub struct AiController {
    player: u32,
    profile_id: &'static str,
    memory: AiDecisionMemory,
    pending_builds: PendingBuildTracker,
    staged_units: BTreeSet<u32>,
    active_attack_units: BTreeMap<u32, u32>,
}

impl AiController {
    pub fn new(player: u32) -> Self {
        Self::with_profile_id(player, DEFAULT_LIVE_PROFILE_ID)
    }

    pub fn with_profile_id(player: u32, profile_id: &'static str) -> Self {
        let profile = profile_by_id(profile_id).unwrap_or(&RIFLE_FLOOD_FULL_SATURATION);
        Self {
            player,
            profile_id: profile.id,
            memory: AiDecisionMemory::for_profile(profile),
            pending_builds: PendingBuildTracker::default(),
            staged_units: BTreeSet::new(),
            active_attack_units: BTreeMap::new(),
        }
    }

    pub fn player_id(&self) -> u32 {
        self.player
    }

    pub fn profile_id(&self) -> &'static str {
        self.profile_id
    }

    fn profile(&self) -> &'static AiProfile {
        profile_by_id(self.profile_id).unwrap_or(&RIFLE_FLOOD_FULL_SATURATION)
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn live_controller_uses_default_profile_id() {
        let ai = AiController::new(2);

        assert_eq!(ai.player_id(), 2);
        assert_eq!(ai.profile_id(), RIFLE_FLOOD_FULL_SATURATION_ID);
    }

    #[test]
    fn live_profile_pool_has_only_rifle_flood_full_saturation() {
        assert_eq!(LIVE_PROFILE_IDS, [RIFLE_FLOOD_FULL_SATURATION_ID]);
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

        assert_eq!(ai.profile_id(), RIFLE_FLOOD_FULL_SATURATION_ID);
    }
}
