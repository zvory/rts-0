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
    pub resources: ObserverAnalysisResources,
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisResourceTotals {
    pub steel: u32,
    pub oil: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisResources {
    pub lifetime: ObserverAnalysisResourceTotals,
    #[serde(rename = "last5s")]
    pub last_5s: ObserverAnalysisResourceTotals,
    pub last_minute: ObserverAnalysisResourceTotals,
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
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tooltip: Option<String>,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tooltip: Option<String>,
    },
    Line {
        id: String,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: String,
        alpha: f32,
        width: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tooltip: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kinds, ServerMessage};

    #[test]
    fn observer_analysis_serializes_contract_shape() {
        let msg = ServerMessage::ObserverAnalysis(ObserverAnalysisPayload {
            tick: 77,
            map_analysis: Some(ObserverMapAnalysisDiagnostics {
                map_width: 126,
                map_height: 126,
                tile_size: 32,
                layers: vec![ObserverMapAnalysisLayer {
                    id: "components".to_string(),
                    label: "Components".to_string(),
                    default_visible: true,
                    primitives: vec![
                        ObserverMapAnalysisPrimitive::TileRect {
                            id: "component:0".to_string(),
                            tile_x: 2,
                            tile_y: 3,
                            tile_w: 10,
                            tile_h: 8,
                            fill: "#3da5d9".to_string(),
                            stroke: "#3da5d9".to_string(),
                            alpha: 0.12,
                            label: Some("C0 80t clr8".to_string()),
                            tooltip: Some(
                                "Static map-analysis component tile rectangle".to_string(),
                            ),
                        },
                        ObserverMapAnalysisPrimitive::Line {
                            id: "line:debug".to_string(),
                            x1: 16.0,
                            y1: 32.0,
                            x2: 96.0,
                            y2: 32.0,
                            color: "#00d4ff".to_string(),
                            alpha: 0.8,
                            width: 3.0,
                            label: Some("debug line".to_string()),
                            tooltip: Some("Line primitive with hover text".to_string()),
                        },
                    ],
                }],
            }),
            players: vec![ObserverAnalysisPlayer {
                id: 1,
                units: vec![ObserverAnalysisKindCount {
                    kind: kinds::RIFLEMAN.to_string(),
                    count: 3,
                    steel_value: 180,
                    oil_value: 0,
                }],
                production: vec![ObserverAnalysisProduction {
                    building_id: 10,
                    building_kind: kinds::BARRACKS.to_string(),
                    item_kind: kinds::MACHINE_GUNNER.to_string(),
                    item_type: "unit".to_string(),
                    progress: 0.5,
                    queue_depth: 2,
                }],
                units_lost: vec![ObserverAnalysisKindCount {
                    kind: kinds::WORKER.to_string(),
                    count: 1,
                    steel_value: 50,
                    oil_value: 0,
                }],
                resources_lost: ObserverAnalysisResourcesLost { steel: 50, oil: 0 },
                resources: ObserverAnalysisResources {
                    lifetime: ObserverAnalysisResourceTotals {
                        steel: 120,
                        oil: 20,
                    },
                    last_5s: ObserverAnalysisResourceTotals { steel: 40, oil: 10 },
                    last_minute: ObserverAnalysisResourceTotals {
                        steel: 120,
                        oil: 20,
                    },
                },
                ai_diagnostics: Some(ObserverAnalysisAiDiagnostics {
                    profile_id: "ai_2_1".to_string(),
                    trace_tick: 72,
                    lines: vec![
                        "profile=ai_2_1 tick=72".to_string(),
                        "goal=Production status=Selected blockers=- intents=Train:Rifleman"
                            .to_string(),
                    ],
                }),
            }],
        });
        let json = serde_json::to_value(msg).expect("observer analysis should serialize");

        assert_eq!(json["t"], "observerAnalysis");
        assert_eq!(json["tick"], 77);
        assert_eq!(json["mapAnalysis"]["mapWidth"], 126);
        assert_eq!(json["mapAnalysis"]["layers"][0]["id"], "components");
        assert_eq!(
            json["mapAnalysis"]["layers"][0]["primitives"][0]["kind"],
            "tileRect"
        );
        assert_eq!(
            json["mapAnalysis"]["layers"][0]["primitives"][0]["tileW"],
            10
        );
        assert_eq!(
            json["mapAnalysis"]["layers"][0]["primitives"][1]["kind"],
            "line"
        );
        assert_eq!(
            json["mapAnalysis"]["layers"][0]["primitives"][1]["tooltip"],
            "Line primitive with hover text"
        );
        assert_eq!(json["players"][0]["id"], 1);
        assert_eq!(json["players"][0]["units"][0]["kind"], "rifleman");
        assert_eq!(json["players"][0]["units"][0]["count"], 3);
        assert_eq!(json["players"][0]["units"][0]["steelValue"], 180);
        assert_eq!(json["players"][0]["production"][0]["buildingId"], 10);
        assert_eq!(json["players"][0]["production"][0]["itemType"], "unit");
        assert_eq!(json["players"][0]["production"][0]["queueDepth"], 2);
        assert_eq!(json["players"][0]["unitsLost"][0]["kind"], "worker");
        assert_eq!(json["players"][0]["resourcesLost"]["steel"], 50);
        assert_eq!(json["players"][0]["resources"]["lifetime"]["steel"], 120);
        assert_eq!(json["players"][0]["resources"]["last5s"]["oil"], 10);
        assert_eq!(json["players"][0]["resources"]["lastMinute"]["steel"], 120);
        assert_eq!(json["players"][0]["aiDiagnostics"]["profileId"], "ai_2_1");
        assert_eq!(json["players"][0]["aiDiagnostics"]["traceTick"], 72);
        assert_eq!(
            json["players"][0]["aiDiagnostics"]["lines"][1],
            "goal=Production status=Selected blockers=- intents=Train:Rifleman"
        );
    }
}
