//! Environment-gated, loopback-only handoff for large Lab replay artifacts.
//!
//! This is intentionally a server-shell development seam, not a wire-protocol endpoint. The
//! private Interact server receives an unguessable capability from its owning driver and all
//! replay validation/mutation is still delegated to the authoritative room task.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

use super::AppState;
use rts_server::protocol::{lab_replay_artifact_from_slice, LabReplayArtifactV1};

pub(super) const CAPABILITY_ENV: &str = "RTS_INTERACT_LAB_ARTIFACT_CAPABILITY";
pub(super) const CAPABILITY_HEADER: &str = "x-interact-lab-capability";
const ROOM_HEADER: &str = "x-interact-lab-room";
pub(super) const MAX_ARTIFACT_BYTES: usize = 8 * 1024 * 1024;
const MAX_TRANSFERS: usize = 16;
const TRANSFER_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Default)]
pub(super) struct InteractLabArtifactBridge {
    capability: Option<String>,
    transfers: Arc<Mutex<HashMap<String, Transfer>>>,
}

struct Transfer {
    bytes: Vec<u8>,
    room: Option<String>,
    expires_at: Instant,
}

#[derive(Deserialize)]
pub(super) struct ExportRequest {
    room: String,
    name: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct ImportRequest {
    room: String,
    artifact_id: String,
}

#[derive(Deserialize)]
pub(super) struct CleanupRequest {
    room: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TransferResponse {
    artifact_id: String,
    bytes: usize,
    expires_in_seconds: u64,
}

impl InteractLabArtifactBridge {
    pub(super) fn from_env() -> Self {
        let capability = std::env::var(CAPABILITY_ENV).ok().filter(|value| {
            value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
        Self {
            capability,
            transfers: Arc::default(),
        }
    }

    fn authorize(&self, peer: SocketAddr, headers: &HeaderMap) -> Result<(), StatusCode> {
        if !peer.ip().is_loopback() {
            return Err(StatusCode::NOT_FOUND);
        }
        let Some(expected) = self.capability.as_deref() else {
            return Err(StatusCode::NOT_FOUND);
        };
        let supplied = headers
            .get(CAPABILITY_HEADER)
            .and_then(|value| value.to_str().ok());
        if supplied != Some(expected) {
            return Err(StatusCode::UNAUTHORIZED);
        }
        Ok(())
    }

    fn insert(&self, bytes: Vec<u8>, room: Option<String>) -> Result<TransferResponse, String> {
        if bytes.len() > MAX_ARTIFACT_BYTES {
            return Err("lab replay artifact exceeds 8 MiB".to_string());
        }
        let mut transfers = self
            .transfers
            .lock()
            .map_err(|_| "artifact transfer store is unavailable".to_string())?;
        transfers.retain(|_, transfer| transfer.expires_at > Instant::now());
        if transfers.len() >= MAX_TRANSFERS {
            return Err("too many pending artifact transfers".to_string());
        }
        let artifact_id = format!("transfer_{:032x}", rand::random::<u128>());
        transfers.insert(
            artifact_id.clone(),
            Transfer {
                bytes,
                room,
                expires_at: Instant::now() + TRANSFER_TTL,
            },
        );
        let bytes = transfers
            .get(&artifact_id)
            .map_or(0, |entry| entry.bytes.len());
        Ok(TransferResponse {
            artifact_id,
            bytes,
            expires_in_seconds: TRANSFER_TTL.as_secs(),
        })
    }

    fn take(&self, artifact_id: &str, room: &str) -> Result<Vec<u8>, String> {
        if !valid_artifact_id(artifact_id) {
            return Err("invalid artifact id".to_string());
        }
        let mut transfers = self
            .transfers
            .lock()
            .map_err(|_| "artifact transfer store is unavailable".to_string())?;
        transfers.retain(|_, transfer| transfer.expires_at > Instant::now());
        let transfer = transfers
            .get(artifact_id)
            .ok_or_else(|| "artifact id is unknown or expired".to_string())?;
        if transfer.room.as_deref() != Some(room) {
            return Err("artifact id does not belong to this Lab room".to_string());
        }
        transfers
            .remove(artifact_id)
            .map(|transfer| transfer.bytes)
            .ok_or_else(|| "artifact id is unknown or expired".to_string())
    }

    fn remove_for_room(&self, room: &str) -> usize {
        let Ok(mut transfers) = self.transfers.lock() else {
            return 0;
        };
        let before = transfers.len();
        transfers.retain(|_, transfer| transfer.room.as_deref() != Some(room));
        before.saturating_sub(transfers.len())
    }
}

pub(super) async fn export_handler(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ExportRequest>,
) -> Response {
    if let Err(status) = state.interact_lab_artifacts.authorize(peer, &headers) {
        return status.into_response();
    }
    if !valid_room(&request.room) {
        return error(StatusCode::BAD_REQUEST, "invalid Lab room id");
    }
    match state
        .lobby
        .export_lab_replay_artifact(&request.room, request.name)
        .await
        .and_then(|artifact| serde_json::to_vec(&artifact).map_err(|err| err.to_string()))
        .and_then(|bytes| {
            state
                .interact_lab_artifacts
                .insert(bytes, Some(request.room))
        }) {
        Ok(result) => Json(result).into_response(),
        Err(message) => error(StatusCode::BAD_REQUEST, &message),
    }
}

pub(super) async fn upload_handler(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    if let Err(status) = state.interact_lab_artifacts.authorize(peer, &headers) {
        return status.into_response();
    }
    let room = headers
        .get(ROOM_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| valid_room(value));
    let Some(room) = room else {
        return error(StatusCode::BAD_REQUEST, "valid Lab room header is required");
    };
    let artifact = match lab_replay_artifact_from_slice(&bytes) {
        Ok(artifact) => artifact,
        Err(err) => return error(StatusCode::BAD_REQUEST, &format!("replay rejected: {err}")),
    };
    let canonical = match serde_json::to_vec(&artifact) {
        Ok(bytes) => bytes,
        Err(err) => return error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    match state
        .interact_lab_artifacts
        .insert(canonical, Some(room.to_string()))
    {
        Ok(result) => Json(result).into_response(),
        Err(message) => error(StatusCode::BAD_REQUEST, &message),
    }
}

pub(super) async fn download_handler(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(artifact_id): Path<String>,
) -> Response {
    if let Err(status) = state.interact_lab_artifacts.authorize(peer, &headers) {
        return status.into_response();
    }
    let room = headers
        .get(ROOM_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| valid_room(value));
    let Some(room) = room else {
        return error(StatusCode::BAD_REQUEST, "valid Lab room header is required");
    };
    match state.interact_lab_artifacts.take(&artifact_id, room) {
        Ok(bytes) => ([("content-type", "application/json")], bytes).into_response(),
        Err(message) => error(StatusCode::NOT_FOUND, &message),
    }
}

pub(super) async fn import_handler(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ImportRequest>,
) -> Response {
    if let Err(status) = state.interact_lab_artifacts.authorize(peer, &headers) {
        return status.into_response();
    }
    if !valid_room(&request.room) {
        return error(StatusCode::BAD_REQUEST, "invalid Lab room id");
    }
    let artifact: LabReplayArtifactV1 = match state
        .interact_lab_artifacts
        .take(&request.artifact_id, &request.room)
        .and_then(|bytes| lab_replay_artifact_from_slice(&bytes).map_err(|err| err.to_string()))
    {
        Ok(artifact) => artifact,
        Err(message) => return error(StatusCode::BAD_REQUEST, &message),
    };
    match state
        .lobby
        .import_lab_replay_artifact(&request.room, artifact)
        .await
    {
        Ok(()) => Json(serde_json::json!({ "imported": true })).into_response(),
        Err(message) => error(StatusCode::BAD_REQUEST, &message),
    }
}

pub(super) async fn cleanup_handler(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CleanupRequest>,
) -> Response {
    if let Err(status) = state.interact_lab_artifacts.authorize(peer, &headers) {
        return status.into_response();
    }
    let removed = state.interact_lab_artifacts.remove_for_room(&request.room);
    Json(serde_json::json!({ "removed": removed })).into_response()
}

fn valid_room(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn valid_artifact_id(value: &str) -> bool {
    value.len() == 41
        && value.starts_with("transfer_")
        && value[9..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_is_disabled_without_capability() {
        let bridge = InteractLabArtifactBridge::default();
        let headers = HeaderMap::new();
        assert_eq!(
            bridge.authorize("127.0.0.1:1234".parse().unwrap(), &headers),
            Err(StatusCode::NOT_FOUND)
        );
    }

    #[test]
    fn transfer_ids_and_room_names_are_strict() {
        assert!(valid_room("interact-lab-deadbeef-12"));
        assert!(!valid_room("../other"));
        assert!(valid_artifact_id(&format!("transfer_{}", "a".repeat(32))));
        assert!(!valid_artifact_id("transfer_../artifact"));
    }

    #[test]
    fn capability_peer_and_expiry_are_enforced() {
        let bridge = InteractLabArtifactBridge {
            capability: Some("a".repeat(64)),
            transfers: Arc::default(),
        };
        let mut headers = HeaderMap::new();
        headers.insert(CAPABILITY_HEADER, "b".repeat(64).parse().unwrap());
        assert_eq!(
            bridge.authorize("127.0.0.1:1".parse().unwrap(), &headers),
            Err(StatusCode::UNAUTHORIZED)
        );
        headers.insert(CAPABILITY_HEADER, "a".repeat(64).parse().unwrap());
        assert_eq!(
            bridge.authorize("192.0.2.1:1".parse().unwrap(), &headers),
            Err(StatusCode::NOT_FOUND)
        );
        assert!(bridge
            .authorize("127.0.0.1:1".parse().unwrap(), &headers)
            .is_ok());

        let transfer = bridge
            .insert(b"{}".to_vec(), Some("room".to_string()))
            .unwrap();
        bridge
            .transfers
            .lock()
            .unwrap()
            .get_mut(&transfer.artifact_id)
            .unwrap()
            .expires_at = Instant::now() - Duration::from_secs(1);
        assert!(bridge
            .take(&transfer.artifact_id, "room")
            .unwrap_err()
            .contains("expired"));

        let transfer = bridge
            .insert(b"{}".to_vec(), Some("room".to_string()))
            .unwrap();
        assert!(bridge.take(&transfer.artifact_id, "other").is_err());
        assert_eq!(bridge.take(&transfer.artifact_id, "room").unwrap(), b"{}");
        assert!(bridge.take(&transfer.artifact_id, "room").is_err());
    }
}
