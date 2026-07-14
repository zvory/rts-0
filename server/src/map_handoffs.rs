use std::collections::HashMap;
use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use rts_server::protocol::LabMapDraft;
use rts_sim::game::lab::LabOp;
use rts_sim::game::map::{AuthoredMapData, Map};
use rts_sim::game::{Game, PlayerInit};

use crate::AppState;

const HANDOFF_TTL: Duration = Duration::from_secs(120);
const MAX_HANDOFFS: usize = 64;
const MAX_AUTHORED_MAP_BYTES: usize = 512 * 1024;
const HANDOFF_ID_LEN: usize = 32;

#[derive(Clone, Default)]
pub(crate) struct MapHandoffStore {
    entries: std::sync::Arc<Mutex<HashMap<String, MapHandoff>>>,
}

#[derive(Clone)]
struct MapHandoff {
    destination: HandoffDestination,
    authored_map: serde_json::Value,
    materialized_map: LabMapDraft,
    expires_at: Instant,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum HandoffDestination {
    Lab,
    Editor,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CreateMapHandoffRequest {
    destination: HandoffDestination,
    authored_map: serde_json::Value,
    materialized_map: LabMapDraft,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateMapHandoffResponse {
    handoff_id: String,
    expires_in_ms: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsumeMapHandoffResponse {
    destination: HandoffDestination,
    #[serde(skip_serializing_if = "Option::is_none")]
    room: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authored_map: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub(crate) async fn create_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateMapHandoffRequest>,
) -> Response {
    if let Err(error) = validate_request(&request) {
        return error_response(StatusCode::BAD_REQUEST, error);
    }

    let now = Instant::now();
    let mut entries = state.map_handoffs.entries.lock().await;
    entries.retain(|_, handoff| handoff.expires_at > now);
    if entries.len() >= MAX_HANDOFFS {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Too many map handoffs are pending; try again shortly.".to_string(),
        );
    }

    let handoff_id = loop {
        let candidate = format!("{:032x}", rand::random::<u128>());
        if !entries.contains_key(&candidate) {
            break candidate;
        }
    };
    entries.insert(
        handoff_id.clone(),
        MapHandoff {
            destination: request.destination,
            authored_map: request.authored_map,
            materialized_map: request.materialized_map,
            expires_at: now + HANDOFF_TTL,
        },
    );

    Json(CreateMapHandoffResponse {
        handoff_id,
        expires_in_ms: HANDOFF_TTL.as_millis() as u64,
    })
    .into_response()
}

pub(crate) async fn consume_handler(
    State(state): State<AppState>,
    Path(handoff_id): Path<String>,
) -> Response {
    if !safe_handoff_id(&handoff_id) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "Invalid map handoff id.".to_string(),
        );
    }

    let now = Instant::now();
    let handoff = {
        let mut entries = state.map_handoffs.entries.lock().await;
        entries.retain(|_, handoff| handoff.expires_at > now);
        entries.remove(&handoff_id)
    };
    let Some(handoff) = handoff else {
        return error_response(
            StatusCode::GONE,
            "This map handoff expired or was already used.".to_string(),
        );
    };

    match handoff.destination {
        HandoffDestination::Lab => {
            let room = match state
                .lobby
                .create_map_editor_lab_room(handoff.materialized_map)
                .await
            {
                Ok(room) => room,
                Err(_) => {
                    return error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "Server is draining for deploy; new Labs are disabled.".to_string(),
                    );
                }
            };
            Json(ConsumeMapHandoffResponse {
                destination: HandoffDestination::Lab,
                room: Some(room),
                authored_map: None,
            })
            .into_response()
        }
        HandoffDestination::Editor => Json(ConsumeMapHandoffResponse {
            destination: HandoffDestination::Editor,
            room: None,
            authored_map: Some(handoff.authored_map),
        })
        .into_response(),
    }
}

fn validate_request(request: &CreateMapHandoffRequest) -> Result<(), String> {
    let authored_json = serde_json::to_string(&request.authored_map)
        .map_err(|error| format!("Map JSON could not be encoded: {error}"))?;
    if authored_json.len() > MAX_AUTHORED_MAP_BYTES {
        return Err("Map JSON is too large.".to_string());
    }
    let player_count = request.materialized_map.starts.len();
    if !(1..=4).contains(&player_count) {
        return Err("Map handoffs require one to four player starts.".to_string());
    }
    let authored_map = Map::materialize_authored_json(&authored_json, player_count)
        .map_err(|error| format!("Authored map is invalid: {error}"))?;
    validate_materialized_binding(&authored_map, &request.materialized_map)?;
    validate_materialized_map(&request.materialized_map, player_count)?;
    Ok(())
}

fn validate_materialized_map(draft: &LabMapDraft, player_count: usize) -> Result<(), String> {
    let players: Vec<_> = (0..player_count)
        .map(|index| PlayerInit {
            id: index as u32 + 1,
            team_id: index as u32 + 1,
            faction_id: "kriegsia".to_string(),
            name: format!("Map Editor {}", index + 1),
            color: "#ffffff".to_string(),
            is_ai: false,
        })
        .collect();
    let map = Map::load("Default", player_count, 0)
        .map_err(|error| format!("Could not prepare map validation: {error}"))?;
    let metadata = Map::metadata_for_name("Default")
        .map_err(|error| format!("Could not prepare map metadata: {error}"))?;
    let mut game = Game::new_lab(&players, 0, map, metadata);
    game.apply_lab_op(LabOp::ApplyMapDraft(draft.clone()))
        .map_err(|error| format!("Map locations are invalid: {error:?}"))?;
    Ok(())
}

fn validate_materialized_binding(
    authored: &AuthoredMapData,
    materialized: &LabMapDraft,
) -> Result<(), String> {
    if authored.name != materialized.name {
        return Err("Authored and materialized map names do not match.".to_string());
    }
    if authored.size != materialized.size {
        return Err("Authored and materialized map sizes do not match.".to_string());
    }
    if authored.terrain != materialized.terrain {
        return Err("Authored and materialized terrain do not match.".to_string());
    }
    if !locations_match(&authored.starts, &materialized.starts)
        || !locations_match(&authored.base_sites, &materialized.base_sites)
    {
        return Err("Authored map locations do not match the materialized map.".to_string());
    }
    Ok(())
}

fn locations_match(
    authored: &[(u32, u32)],
    materialized: &[rts_server::protocol::LabMapTile],
) -> bool {
    authored.len() == materialized.len()
        && authored
            .iter()
            .zip(materialized)
            .all(|(&(x, y), tile)| x == tile.x && y == tile.y)
}

fn safe_handoff_id(value: &str) -> bool {
    value.len() == HANDOFF_ID_LEN && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn error_response(status: StatusCode, error: String) -> Response {
    (status, Json(ErrorResponse { error })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_server::protocol::{terrain, LabMapTile};

    fn valid_request() -> CreateMapHandoffRequest {
        let authored_map: serde_json::Value =
            serde_json::from_str(include_str!("../assets/maps/no-terrain.json"))
                .expect("map fixture");
        let tile = |value: &serde_json::Value| LabMapTile {
            x: value["x"].as_u64().expect("x") as u32,
            y: value["y"].as_u64().expect("y") as u32,
        };
        let starts = authored_map["startLocations"]
            .as_array()
            .expect("start locations")
            .iter()
            .map(tile)
            .collect();
        let base_sites = authored_map["baseSites"]
            .as_array()
            .expect("base sites")
            .iter()
            .map(tile)
            .collect();
        CreateMapHandoffRequest {
            destination: HandoffDestination::Lab,
            authored_map,
            materialized_map: LabMapDraft {
                name: "No Terrain".to_string(),
                size: 126,
                terrain: vec![0; 126 * 126],
                starts,
                base_sites,
            },
        }
    }

    #[test]
    fn handoff_validation_binds_flat_locations_to_materialized_map() {
        let valid = valid_request();
        assert!(
            validate_request(&valid).is_ok(),
            "unexpected validation error: {:?}",
            validate_request(&valid)
        );
        let mut request = valid_request();
        request.materialized_map.starts[0].x += 1;
        assert!(validate_request(&request)
            .expect_err("mismatched materialization must fail")
            .contains("do not match"));
    }

    #[test]
    fn handoff_validation_binds_every_authored_road_variant() {
        let mut request = valid_request();
        let road_chars = ['=', '-', '|', '\\', '/'];
        let road_codes = [
            terrain::ROAD_BARE,
            terrain::ROAD_HORIZONTAL,
            terrain::ROAD_VERTICAL,
            terrain::ROAD_DIAGONAL_NW_SE,
            terrain::ROAD_DIAGONAL_NE_SW,
        ];
        let first_row = request.authored_map["terrain"][0]
            .as_str()
            .expect("terrain row");
        let mut chars = first_row.chars().collect::<Vec<_>>();
        chars[..road_chars.len()].copy_from_slice(&road_chars);
        request.authored_map["terrain"][0] = chars.into_iter().collect::<String>().into();
        request.materialized_map.terrain[..road_codes.len()].copy_from_slice(&road_codes);

        assert_eq!(validate_request(&request), Ok(()));
    }

    #[test]
    fn handoff_ids_are_bounded_hex_tokens() {
        assert!(safe_handoff_id("0123456789abcdef0123456789abcdef"));
        assert!(!safe_handoff_id("../map"));
        assert!(!safe_handoff_id("a".repeat(33).as_str()));
    }
}
