use serde::{Deserialize, Serialize};

use rts_contract::{LabScenarioResearch, LabScenarioResources, LabVisionMode};

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
#[serde(rename_all = "camelCase")]
pub struct LabScenarioV1 {
    pub schema_version: u32,
    pub kind: String,
    pub name: String,
    pub seed: u32,
    pub map: LabScenarioMap,
    pub players: Vec<LabScenarioPlayer>,
    pub entities: Vec<LabScenarioEntity>,
    pub metadata: LabScenarioMetadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioMap {
    pub name: String,
    pub schema_version: u32,
    pub content_hash: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioPlayer {
    pub id: u32,
    pub team_id: u32,
    pub faction_id: String,
    pub name: String,
    pub color: String,
    pub is_ai: bool,
    pub resources: LabScenarioResources,
    pub research: LabScenarioResearch,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioEntity {
    pub id: u32,
    pub owner: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub completed: bool,
    pub construction_progress: Option<u32>,
    pub construction_total: Option<u32>,
    pub resource_remaining: Option<u32>,
    #[serde(default)]
    pub facing: Option<f32>,
    #[serde(default)]
    pub weapon_facing: Option<f32>,
    #[serde(default)]
    pub set_up: bool,
    #[serde(default)]
    pub setup_facing: Option<f32>,
    #[serde(default)]
    pub setup_target: Option<LabScenarioPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<LabScenarioOrder>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued_orders: Vec<LabScenarioOrder>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioOrder {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_x: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_y: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staging_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staging_y: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioMetadata {
    pub exported_tick: u32,
    pub lab: LabScenarioLabMetadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioLabMetadata {
    pub vision: LabVisionMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub god_mode_players: Vec<u32>,
}
