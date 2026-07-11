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
use rts_sim::game::map::Map;
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
    selected_layout_id: String,
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
    selected_layout_id: String,
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
    selected_layout_id: String,
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
            selected_layout_id: request.selected_layout_id,
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
                selected_layout_id: handoff.selected_layout_id,
            })
            .into_response()
        }
        HandoffDestination::Editor => Json(ConsumeMapHandoffResponse {
            destination: HandoffDestination::Editor,
            room: None,
            authored_map: Some(handoff.authored_map),
            selected_layout_id: handoff.selected_layout_id,
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
    if request.selected_layout_id.is_empty() || request.selected_layout_id.len() > 80 {
        return Err("A bounded selected layout id is required.".to_string());
    }
    let player_count = request.materialized_map.starts.len();
    if !(1..=4).contains(&player_count) {
        return Err("Map handoffs require one to four player starts.".to_string());
    }
    Map::validate_authored_json(&authored_json, player_count)
        .map_err(|error| format!("Authored map is invalid: {error}"))?;
    validate_materialized_binding(request)?;
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
        .map_err(|error| format!("Selected map layout is invalid: {error:?}"))?;
    Ok(())
}

fn validate_materialized_binding(request: &CreateMapHandoffRequest) -> Result<(), String> {
    let map = request
        .authored_map
        .as_object()
        .ok_or_else(|| "Authored map must be an object.".to_string())?;
    if map.get("name").and_then(|value| value.as_str()) != Some(&request.materialized_map.name) {
        return Err("Authored and materialized map names do not match.".to_string());
    }
    let terrain = map
        .get("terrain")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "Authored map terrain is missing.".to_string())?;
    if terrain.len() != request.materialized_map.size as usize {
        return Err("Authored and materialized map sizes do not match.".to_string());
    }
    let terrain_codes = terrain
        .iter()
        .flat_map(|row| row.as_str().unwrap_or("").bytes())
        .map(|byte| match byte {
            b'.' => 0,
            b'#' => 1,
            b'~' => 2,
            _ => u8::MAX,
        })
        .collect::<Vec<_>>();
    if terrain_codes != request.materialized_map.terrain {
        return Err("Authored and materialized terrain do not match.".to_string());
    }

    let sites = map
        .get("sites")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "Authored map sites are missing.".to_string())?;
    let by_id: HashMap<_, _> = sites
        .iter()
        .filter_map(|site| Some((site.get("id")?.as_str()?, site)))
        .collect();
    let layouts = map
        .get("layouts")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "Authored map layouts are missing.".to_string())?;
    let layout = layouts
        .iter()
        .find(|layout| {
            layout.get("id").and_then(|value| value.as_str()) == Some(&request.selected_layout_id)
        })
        .ok_or_else(|| "Selected layout does not exist in the authored map.".to_string())?;
    let slots = layout
        .get("slots")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "Selected layout slots are missing.".to_string())?;

    let mut starts = Vec::with_capacity(slots.len());
    let mut naturals = Vec::new();
    let mut seen_naturals = std::collections::HashSet::new();
    for slot in slots {
        let main_id = slot
            .get("main")
            .and_then(|value| value.as_str())
            .ok_or_else(|| "Selected layout has a missing main site.".to_string())?;
        starts.push(site_tile(&by_id, main_id, "main")?);
        let legacy = slot.get("natural").and_then(|value| value.as_str());
        let ids = legacy.into_iter().chain(
            slot.get("naturals")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str()),
        );
        for id in ids {
            if seen_naturals.insert(id) {
                naturals.push(site_tile(&by_id, id, "natural")?);
            }
        }
    }
    if starts != request.materialized_map.starts
        || naturals != request.materialized_map.expansion_sites
    {
        return Err("Selected layout does not match the materialized start/base list.".to_string());
    }
    Ok(())
}

fn site_tile(
    sites: &HashMap<&str, &serde_json::Value>,
    id: &str,
    expected_kind: &str,
) -> Result<rts_server::protocol::LabMapTile, String> {
    let site = sites
        .get(id)
        .ok_or_else(|| format!("Selected layout references missing site {id:?}."))?;
    if site.get("kind").and_then(|value| value.as_str()) != Some(expected_kind) {
        return Err(format!("Selected layout site {id:?} has the wrong kind."));
    }
    let x = site
        .get("x")
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| format!("Selected layout site {id:?} has an invalid x coordinate."))?;
    let y = site
        .get("y")
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| format!("Selected layout site {id:?} has an invalid y coordinate."))?;
    Ok(rts_server::protocol::LabMapTile { x, y })
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
    use rts_server::protocol::LabMapTile;

    fn valid_request() -> CreateMapHandoffRequest {
        let authored_map: serde_json::Value =
            serde_json::from_str(include_str!("../assets/maps/no-terrain.json"))
                .expect("map fixture");
        let sites = authored_map["sites"]
            .as_array()
            .expect("sites")
            .iter()
            .filter_map(|site| Some((site["id"].as_str()?.to_string(), site)))
            .collect::<HashMap<_, _>>();
        let layout = authored_map["layouts"]
            .as_array()
            .expect("layouts")
            .iter()
            .find(|layout| layout["id"] == "2p_cross_nw_se")
            .expect("duel layout");
        let tile = |id: &str| {
            let site = sites.get(id).expect("site");
            LabMapTile {
                x: site["x"].as_u64().expect("x") as u32,
                y: site["y"].as_u64().expect("y") as u32,
            }
        };
        let starts = layout["slots"]
            .as_array()
            .expect("slots")
            .iter()
            .map(|slot| tile(slot["main"].as_str().expect("main")))
            .collect();
        let expansion_sites = layout["slots"]
            .as_array()
            .expect("slots")
            .iter()
            .map(|slot| tile(slot["natural"].as_str().expect("natural")))
            .collect();
        CreateMapHandoffRequest {
            destination: HandoffDestination::Lab,
            authored_map,
            materialized_map: LabMapDraft {
                name: "No Terrain".to_string(),
                size: 126,
                terrain: vec![0; 126 * 126],
                starts,
                expansion_sites,
            },
            selected_layout_id: "2p_cross_nw_se".to_string(),
        }
    }

    #[test]
    fn handoff_validation_binds_the_selected_layout_to_materialized_map() {
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
            .contains("does not match"));
    }

    #[test]
    fn handoff_ids_are_bounded_hex_tokens() {
        assert!(safe_handoff_id("0123456789abcdef0123456789abcdef"));
        assert!(!safe_handoff_id("../map"));
        assert!(!safe_handoff_id("a".repeat(33).as_str()));
    }
}
