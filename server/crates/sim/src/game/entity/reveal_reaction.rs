use serde::{Deserialize, Serialize};

use super::CombatState;
use crate::rules::combat::WeaponKind;

pub(in crate::game) const MAX_FIRING_REVEAL_REACTION_GATES_PER_WEAPON: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::game) struct FiringRevealEpisode {
    pub viewer: u32,
    pub source_entity: u32,
    pub started_at_tick: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::game) struct FiringRevealReactionGate {
    pub reveal_viewer: u32,
    pub reveal_source_entity: u32,
    pub episode_started_at_tick: u32,
    pub ready_at_tick: u32,
}

impl CombatState {
    pub(in crate::game) fn firing_reveal_reaction_ready(
        &mut self,
        weapon: WeaponKind,
        target_id: u32,
        episode: FiringRevealEpisode,
        tick: u32,
        ticks: u32,
    ) -> bool {
        if ticks == 0 {
            return true;
        }
        let remaining_reload = self.weapon_cooldown(weapon);
        let gates = self.firing_reveal_reaction_gates.entry(weapon).or_default();
        if !gates.contains_key(&target_id)
            && gates.len() >= MAX_FIRING_REVEAL_REACTION_GATES_PER_WEAPON
        {
            let evicted = gates
                .iter()
                .min_by_key(|(target, gate)| {
                    (
                        gate.episode_started_at_tick,
                        gate.ready_at_tick,
                        gate.reveal_viewer,
                        gate.reveal_source_entity,
                        **target,
                    )
                })
                .map(|(&target, _)| target);
            if let Some(evicted) = evicted {
                gates.remove(&evicted);
            }
        }
        let gate = gates
            .entry(target_id)
            .or_insert_with(|| FiringRevealReactionGate {
                reveal_viewer: episode.viewer,
                reveal_source_entity: episode.source_entity,
                episode_started_at_tick: episode.started_at_tick,
                ready_at_tick: tick.saturating_add(remaining_reload).saturating_add(ticks),
            });
        if gate.reveal_viewer != episode.viewer
            || gate.reveal_source_entity != episode.source_entity
            || gate.episode_started_at_tick != episode.started_at_tick
        {
            *gate = FiringRevealReactionGate {
                reveal_viewer: episode.viewer,
                reveal_source_entity: episode.source_entity,
                episode_started_at_tick: episode.started_at_tick,
                ready_at_tick: tick.saturating_add(remaining_reload).saturating_add(ticks),
            };
        }
        tick >= gate.ready_at_tick
    }

    pub(in crate::game) fn retain_firing_reveal_reaction_gates(
        &mut self,
        mut episode_is_active: impl FnMut(u32, u32, u32) -> bool,
    ) {
        self.firing_reveal_reaction_gates.retain(|_, gates| {
            gates.retain(|_, gate| {
                episode_is_active(
                    gate.reveal_source_entity,
                    gate.reveal_viewer,
                    gate.episode_started_at_tick,
                )
            });
            !gates.is_empty()
        });
    }
}
