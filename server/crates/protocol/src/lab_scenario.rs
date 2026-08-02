use serde::{Deserialize, Serialize};

use rts_contract::{InitialCamera, LabVisionMode};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabMapDraft {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<u8>,
    pub starts: Vec<LabMapTile>,
    pub base_sites: Vec<LabBaseSite>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabMapTile {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabBaseSite {
    pub x: u32,
    pub y: u32,
    pub steel_patches: u32,
    pub oil_patches: u32,
}

pub const MAX_STEEL_PATCHES_PER_BASE: u32 = 36;
pub const MAX_OIL_PATCHES_PER_BASE: u32 = 9;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioAuthoringMetadata {
    pub slug: String,
    pub name: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum LabScenarioPayload {
    Checkpoint(LabCheckpointScenarioV1),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabCheckpointScenarioV1 {
    pub schema_version: u32,
    pub kind: String,
    pub name: String,
    pub seed: u32,
    pub map: LabCheckpointScenarioMap,
    pub metadata: LabCheckpointScenarioMetadata,
    pub checkpoint_payload: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabCheckpointScenarioMap {
    pub name: String,
    pub schema_version: u32,
    pub content_hash: String,
    pub materialized_hash: String,
    pub data: LabCheckpointScenarioMapData,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabCheckpointScenarioMapData {
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<u8>,
    pub starts: Vec<LabScenarioTile>,
    #[serde(rename = "baseSites", alias = "expansionSites")]
    pub base_sites: Vec<LabScenarioBaseSite>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabScenarioTile {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabScenarioBaseSite {
    pub x: u32,
    pub y: u32,
    pub steel_patches: u32,
    pub oil_patches: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabCheckpointScenarioMetadata {
    pub exported_tick: u32,
    pub lab: LabScenarioLabMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_scenario: Option<LabCheckpointScenarioSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_entity_id_map: Vec<LabScenarioEntityIdRemap>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabCheckpointScenarioSource {
    pub kind: String,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioEntityIdRemap {
    pub old_id: u32,
    pub new_id: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioLabMetadata {
    pub vision: LabVisionMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub god_mode_players: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_camera: Option<InitialCamera>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_map_data_uses_per_base_resource_counts() {
        let data = LabCheckpointScenarioMapData {
            width: 16,
            height: 12,
            terrain: vec![0; 16 * 12],
            starts: vec![LabScenarioTile { x: 4, y: 4 }],
            base_sites: vec![LabScenarioBaseSite {
                x: 12,
                y: 12,
                steel_patches: 36,
                oil_patches: 9,
            }],
        };

        let serialized = serde_json::to_value(&data).expect("checkpoint map data serializes");
        assert!(serialized.get("baseSites").is_some());
        assert!(serialized.get("expansionSites").is_none());

        let encoded = serde_json::json!({
            "width": 16,
            "height": 12,
            "terrain": vec![0; 16 * 12],
            "starts": [{ "x": 4, "y": 4 }],
            "baseSites": [{
                "x": 12,
                "y": 12,
                "steelPatches": 36,
                "oilPatches": 9
            }],
        });
        let parsed: LabCheckpointScenarioMapData =
            serde_json::from_value(encoded).expect("checkpoint map data parses");
        assert_eq!(parsed.base_sites, data.base_sites);
    }
}
