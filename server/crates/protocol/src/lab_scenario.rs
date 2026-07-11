use serde::{Deserialize, Serialize};

use rts_contract::{InitialCamera, LabVisionMode};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioAuthoringMetadata {
    pub slug: String,
    pub name: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_notes: Option<String>,
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
    pub size: u32,
    pub terrain: Vec<u8>,
    pub starts: Vec<LabScenarioTile>,
    #[serde(rename = "expansionSites", alias = "baseSites")]
    pub base_sites: Vec<LabScenarioTile>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabScenarioTile {
    pub x: u32,
    pub y: u32,
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
