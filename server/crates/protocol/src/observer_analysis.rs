use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisPayload {
    pub tick: u32,
    pub players: Vec<ObserverAnalysisPlayer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_analysis: Option<ObserverMapAnalysisDiagnostics>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverMapAnalysisDiagnostics {
    pub map_width: u32,
    pub map_height: u32,
    pub tile_size: u32,
    pub layers: Vec<ObserverMapAnalysisLayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverMapAnalysisLayer {
    pub id: String,
    pub label: String,
    pub default_visible: bool,
    pub primitives: Vec<ObserverMapAnalysisPrimitive>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ObserverMapAnalysisPrimitive {
    TileRect {
        id: String,
        tile_x: u32,
        tile_y: u32,
        tile_w: u32,
        tile_h: u32,
        fill: String,
        stroke: String,
        alpha: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    Marker {
        id: String,
        x: f32,
        y: f32,
        radius: f32,
        shape: String,
        color: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
}
