//! Read-only HTTP adapters for authoritative authored-map checks and static route reports.

use axum::body::Bytes;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Json;
use axum::Router;
use serde::Serialize;

use rts_sim::game::map::{analyze_authored_json, check_authored_json};

const MAX_AUTHORED_MAP_ANALYSIS_BYTES: usize = 512 * 1024;
const OUTPUT_SCHEMA_VERSION: u32 = 1;

pub(crate) fn routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route(
            "/api/map-authoring/check",
            post(check_handler).layer(DefaultBodyLimit::max(MAX_AUTHORED_MAP_ANALYSIS_BYTES)),
        )
        .route(
            "/api/map-authoring/report",
            post(report_handler).layer(DefaultBodyLimit::max(MAX_AUTHORED_MAP_ANALYSIS_BYTES)),
        )
}

#[derive(Clone, Copy)]
enum AnalysisMode {
    Check,
    Report,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    schema_version: u32,
    valid: bool,
    error: String,
}

async fn check_handler(body: Bytes) -> Response {
    analyze_request(body, AnalysisMode::Check).await
}

async fn report_handler(body: Bytes) -> Response {
    analyze_request(body, AnalysisMode::Report).await
}

async fn analyze_request(body: Bytes, mode: AnalysisMode) -> Response {
    let authored_json = match String::from_utf8(body.to_vec()) {
        Ok(json) => json,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "Map JSON must be UTF-8."),
    };
    let task = tokio::task::spawn_blocking(move || match mode {
        AnalysisMode::Check => check_authored_json(&authored_json)
            .and_then(|result| serde_json::to_value(result).map_err(|error| error.to_string())),
        AnalysisMode::Report => analyze_authored_json(&authored_json)
            .and_then(|result| serde_json::to_value(result).map_err(|error| error.to_string())),
    });
    match task.await {
        Ok(Ok(result)) => Json(result).into_response(),
        Ok(Err(error)) => error_response(StatusCode::UNPROCESSABLE_ENTITY, &error),
        Err(error) => {
            rts_server::log_warn!(%error, "map authoring analysis task failed");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Map analysis could not be completed.",
            )
        }
    }
}

fn error_response(status: StatusCode, error: &str) -> Response {
    (
        status,
        Json(ErrorResponse {
            schema_version: OUTPUT_SCHEMA_VERSION,
            valid: false,
            error: error.to_string(),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use serde_json::{json, Value};

    use super::*;

    fn valid_map_json() -> String {
        json!({
            "version": 6,
            "name": "HTTP analysis fixture",
            "width": 24,
            "height": 24,
            "description": "fixture",
            "_design": "fixture",
            "terrain": vec![".".repeat(24); 24],
            "startLocations": [{"x": 8, "y": 8}, {"x": 16, "y": 16}],
            "baseSites": [
                {"x": 8, "y": 8, "steelPatches": 4, "oilPatches": 1},
                {"x": 16, "y": 16, "steelPatches": 4, "oilPatches": 1}
            ],
            "doodads": [],
            "stealthTiles": [],
            "noVehicleTiles": []
        })
        .to_string()
    }

    async fn response_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should collect");
        serde_json::from_slice(&bytes).expect("response should be JSON")
    }

    #[tokio::test]
    async fn check_and_report_handlers_share_the_authoritative_library_contract() {
        let check = check_handler(Bytes::from(valid_map_json())).await;
        assert_eq!(check.status(), StatusCode::OK);
        let check = response_json(check).await;
        assert_eq!(check["valid"], true);
        assert_eq!(check["name"], "HTTP analysis fixture");

        let report = report_handler(Bytes::from(valid_map_json())).await;
        assert_eq!(report.status(), StatusCode::OK);
        let report = response_json(report).await;
        assert_eq!(report["valid"], true);
        assert_eq!(report["routes"].as_array().map(Vec::len), Some(2));
    }

    #[tokio::test]
    async fn invalid_map_errors_are_stable_json() {
        let response = check_handler(Bytes::from_static(b"{}")).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let error = response_json(response).await;
        assert_eq!(error["schemaVersion"], OUTPUT_SCHEMA_VERSION);
        assert_eq!(error["valid"], false);
        assert!(error["error"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
    }
}
