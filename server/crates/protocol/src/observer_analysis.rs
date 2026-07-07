use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisPayload {
    pub tick: u32,
    pub players: Vec<ObserverAnalysisPlayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisPlayer {
    pub id: u32,
    pub units: Vec<ObserverAnalysisKindCount>,
    pub production: Vec<ObserverAnalysisProduction>,
    pub units_lost: Vec<ObserverAnalysisKindCount>,
    pub resources_lost: ObserverAnalysisResourcesLost,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_diagnostics: Option<ObserverAnalysisAiDiagnostics>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisAiDiagnostics {
    pub profile_id: String,
    pub trace_tick: u32,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisKindCount {
    pub kind: String,
    pub count: u32,
    pub steel_value: u32,
    pub oil_value: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisProduction {
    pub building_id: u32,
    pub building_kind: String,
    pub item_kind: String,
    /// `"unit"` or `"upgrade"`.
    pub item_type: String,
    /// 0.0..1.0 completion of the front queued item.
    pub progress: f32,
    pub queue_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisResourcesLost {
    pub steel: u32,
    pub oil: u32,
}
