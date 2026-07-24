use serde::{Deserialize, Serialize};

use super::super::anti_tank_gun_memory::{AntiTankGunMemory, AntiTankGunMemoryEntry};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AntiTankGunMemoryV1 {
    pub(super) entries: Vec<AntiTankGunMemoryEntryV1>,
}

impl AntiTankGunMemoryV1 {
    pub(super) fn from_memory(memory: &AntiTankGunMemory) -> Self {
        Self {
            entries: memory
                .checkpoint_entries()
                .into_iter()
                .map(|(player_id, entity_id, entry)| AntiTankGunMemoryEntryV1 {
                    player_id,
                    entity_id,
                    entry,
                })
                .collect(),
        }
    }

    pub(super) fn into_memory(self) -> AntiTankGunMemory {
        AntiTankGunMemory::from_checkpoint_entries(
            self.entries
                .into_iter()
                .map(|entry| (entry.player_id, entry.entity_id, entry.entry))
                .collect(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AntiTankGunMemoryEntryV1 {
    pub(super) player_id: u32,
    pub(super) entity_id: u32,
    pub(super) entry: AntiTankGunMemoryEntry,
}
