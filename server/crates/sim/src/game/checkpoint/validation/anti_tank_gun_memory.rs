use std::collections::BTreeSet;

use super::super::{AntiTankGunMemoryV1, CheckpointPayloadError};
use crate::game::map::Map;

pub(in crate::game::checkpoint) fn validate(
    memory: &AntiTankGunMemoryV1,
    player_ids: &BTreeSet<u32>,
    map: &Map,
    tick: u32,
) -> Result<(), CheckpointPayloadError> {
    let world_size = map.world_size_px();
    let mut keys = BTreeSet::new();
    for entry in &memory.entries {
        if !player_ids.contains(&entry.player_id) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "antiTankGunMemory.playerId",
                id: entry.player_id,
            });
        }
        if !player_ids.contains(&entry.entry.owner) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "antiTankGunMemory.owner",
                id: entry.entry.owner,
            });
        }
        if entry.entry.id != entry.entity_id
            || !entry.entry.x.is_finite()
            || !entry.entry.y.is_finite()
            || !entry.entry.facing.is_finite()
            || entry.entry.x < 0.0
            || entry.entry.y < 0.0
            || entry.entry.x >= world_size
            || entry.entry.y >= world_size
            || entry.entry.observed_tick > tick
        {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "antiTankGunMemory.entry",
            });
        }
        if !keys.insert((entry.player_id, entry.entity_id)) {
            return Err(CheckpointPayloadError::DuplicateId {
                field: "antiTankGunMemory",
                id: entry.entity_id,
            });
        }
    }
    Ok(())
}
