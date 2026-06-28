use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::{replay_incompatibility_reason, ApiError, AppState, MatchReplayLaunchResponse};
use rts_server::lobby;

pub(crate) async fn dev_replay_lobby_handler(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let Some(replay) = params
        .get("replay")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "expected /dev/replay-lobby?replay=<artifact_name>".to_string(),
            }),
        )
            .into_response();
    };

    let artifact = match lobby::load_saved_replay_artifact(replay) {
        Ok(artifact) => artifact,
        Err(err) => {
            let status = if err.starts_with("invalid replay artifact name") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::NOT_FOUND
            };
            return (status, Json(ApiError { error: err })).into_response();
        }
    };

    if let Some(reason) = replay_incompatibility_reason(&artifact, &state.version) {
        return (StatusCode::CONFLICT, Json(ApiError { error: reason })).into_response();
    }

    let room = state.lobby.create_replay_room(artifact).await;
    (
        StatusCode::CREATED,
        Json(MatchReplayLaunchResponse { room }),
    )
        .into_response()
}
