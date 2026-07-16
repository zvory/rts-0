use std::collections::BTreeSet;

use crate::game::entity::{Entity, MAX_FIRING_REVEAL_REACTION_GATES_PER_WEAPON};
use crate::game::firing_reveal::FiringRevealSource;

use super::super::{CheckpointPayloadError, FogStateV1};

pub(super) fn validate_firing_reveal_reaction_gates(
    entity: &Entity,
    next_id: u32,
    tick: u32,
) -> Result<(), CheckpointPayloadError> {
    let Some(combat) = entity.combat.as_ref() else {
        return Ok(());
    };
    for gates in combat.firing_reveal_reaction_gates.values() {
        if gates.len() > MAX_FIRING_REVEAL_REACTION_GATES_PER_WEAPON {
            return Err(CheckpointPayloadError::CountCapExceeded {
                field: "entities.combat.firingRevealReactionGates",
                count: gates.len(),
                max: MAX_FIRING_REVEAL_REACTION_GATES_PER_WEAPON,
            });
        }
        for (&target_id, gate) in gates {
            if target_id == 0
                || target_id >= next_id
                || gate.reveal_viewer == 0
                || gate.reveal_source_entity == 0
                || gate.reveal_source_entity >= next_id
                || gate.episode_started_at_tick > tick
                || gate.ready_at_tick < gate.episode_started_at_tick
            {
                return Err(CheckpointPayloadError::InvalidValue {
                    field: "entities.combat.firingRevealReactionGates",
                });
            }
        }
    }
    Ok(())
}

pub(super) fn validate_firing_reveal_visibility(
    fog: &FogStateV1,
    player_ids: &BTreeSet<u32>,
    entity_next_id: u32,
    firing_reveals: &[FiringRevealSource],
    tick: u32,
) -> Result<(), CheckpointPayloadError> {
    let mut source_episodes = BTreeSet::new();
    for source in firing_reveals {
        if !source_episodes.insert((
            source.viewer(),
            source.entity_id(),
            source.started_at_tick(),
        )) {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "firingReveals",
            });
        }
    }
    for (&viewer, by_entity) in &fog.firing_reveal_visibility {
        if !player_ids.contains(&viewer) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "fog.firingRevealVisibility",
                id: viewer,
            });
        }
        for (&entity_id, visibility) in by_entity {
            if entity_id == 0
                || entity_id >= entity_next_id
                || visibility.episode_started_at_tick > tick
                || visibility
                    .revealed_tile
                    .is_some_and(|tile| tile >= fog.size.saturating_mul(fog.size))
            {
                return Err(CheckpointPayloadError::InvalidValue {
                    field: "fog.firingRevealVisibility",
                });
            }
        }
    }
    Ok(())
}

pub(in crate::game::checkpoint) fn validate_reaction_gates_against_visibility(
    entities: &[Entity],
    entity_ids: &BTreeSet<u32>,
    fog: &FogStateV1,
) -> Result<(), CheckpointPayloadError> {
    for entity in entities {
        let Some(combat) = entity.combat.as_ref() else {
            continue;
        };
        for gates in combat.firing_reveal_reaction_gates.values() {
            for (&target_id, gate) in gates {
                if !entity_ids.contains(&target_id) {
                    return Err(CheckpointPayloadError::InvalidReference {
                        field: "entities.combat.firingRevealReactionGates",
                        id: target_id,
                    });
                }
                let active_episode = fog
                    .firing_reveal_visibility
                    .get(&gate.reveal_viewer)
                    .and_then(|by_entity| by_entity.get(&gate.reveal_source_entity))
                    .map(|visibility| visibility.episode_started_at_tick);
                if active_episode != Some(gate.episode_started_at_tick) {
                    return Err(CheckpointPayloadError::InvalidValue {
                        field: "entities.combat.firingRevealReactionGates",
                    });
                }
            }
        }
    }
    Ok(())
}
