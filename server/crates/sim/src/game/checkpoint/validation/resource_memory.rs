use super::super::{CheckpointPayloadError, MAX_ENTITIES};
use crate::game::{entity::Entity, map::Map, resource_memory::ResourceMemory};
use std::collections::BTreeSet;

pub(in crate::game::checkpoint) fn validate(
    memory: Option<&ResourceMemory>,
    players: &BTreeSet<u32>,
    map: &Map,
    tick: u32,
    entities: &[Entity],
    next_id: u32,
) -> Result<(), CheckpointPayloadError> {
    let Some(memory) = memory else {
        return if tick == 0 {
            Ok(())
        } else {
            Err(CheckpointPayloadError::InvalidValue {
                field: "resourceMemory",
            })
        };
    };
    if memory.nodes.len() > MAX_ENTITIES {
        return Err(CheckpointPayloadError::InvalidValue {
            field: "resourceMemory.nodes",
        });
    }
    for entity in entities.iter().filter(|e| e.is_node()) {
        let Some(node) = memory.nodes.get(&entity.id) else {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "resourceMemory.node",
                id: entity.id,
            });
        };
        if node.kind.is_some_and(|kind| kind != entity.kind)
            || node.x != entity.pos_x()
            || node.y != entity.pos_y()
            || entity
                .remaining()
                .is_some_and(|remaining| remaining > node.initial_remaining)
        {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "resourceMemory.node",
            });
        }
    }
    for (&id, node) in &memory.nodes {
        // Missing legacy kinds are recoverable only while the resource still exists.
        if node.kind.map_or_else(
            || {
                !entities
                    .iter()
                    .any(|entity| entity.id == id && entity.is_node())
            },
            |kind| !kind.is_node(),
        ) || id == 0
            || id >= next_id
            || !node.x.is_finite()
            || !node.y.is_finite()
            || node.x < 0.0
            || node.y < 0.0
            || node.x >= map.world_width_px()
            || node.y >= map.world_height_px()
            || node.observations.len() > players.len()
        {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "resourceMemory.node",
            });
        }
        for (player, observation) in &node.observations {
            if !players.contains(player)
                || observation.tick > tick
                || observation.remaining > node.initial_remaining
            {
                return Err(CheckpointPayloadError::InvalidValue {
                    field: "resourceMemory.observation",
                });
            }
        }
    }
    Ok(())
}
