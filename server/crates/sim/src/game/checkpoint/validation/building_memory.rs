use std::collections::BTreeSet;

use super::super::{BuildingMemoryV1, CheckpointPayloadError};

pub(in crate::game::checkpoint) fn validate(
    memory: &BuildingMemoryV1,
    player_ids: &BTreeSet<u32>,
    tick: u32,
) -> Result<(), CheckpointPayloadError> {
    let mut keys = BTreeSet::new();
    for entry in &memory.entries {
        if !player_ids.contains(&entry.player_id) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "buildingMemory.playerId",
                id: entry.player_id,
            });
        }
        if entry.entry.id != entry.building_id {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "buildingMemory.entry.id",
            });
        }
        if !keys.insert((entry.player_id, entry.building_id)) {
            return Err(CheckpointPayloadError::DuplicateId {
                field: "buildingMemory",
                id: entry.building_id,
            });
        }
    }
    for clear in &memory.cleared {
        if !player_ids.contains(&clear.player_id) {
            return Err(CheckpointPayloadError::InvalidReference {
                field: "buildingMemory.cleared.playerId",
                id: clear.player_id,
            });
        }
        if clear.cleared_tick > tick {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "buildingMemory.cleared.clearedTick",
            });
        }
        if !keys.insert((clear.player_id, clear.building_id)) {
            return Err(CheckpointPayloadError::DuplicateId {
                field: "buildingMemory",
                id: clear.building_id,
            });
        }
    }
    Ok(())
}
