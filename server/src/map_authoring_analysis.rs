//! Read-only HTTP adapters for authoritative authored-map checks and static route reports.

use axum::body::Bytes;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Json;
use axum::Router;
use serde::Serialize;
use std::sync::{Arc, LazyLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use rts_sim::game::process_authored_json;

const MAX_AUTHORED_MAP_ANALYSIS_BYTES: usize = 512 * 1024;
const OUTPUT_SCHEMA_VERSION: u32 = 2;
const MAX_CONCURRENT_MAP_ANALYSES: usize = 2;
static MAP_ANALYSIS_PERMITS: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(MAX_CONCURRENT_MAP_ANALYSES)));

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
    analyze_request_with_permits(body, mode, Arc::clone(&MAP_ANALYSIS_PERMITS)).await
}

async fn analyze_request_with_permits(
    body: Bytes,
    mode: AnalysisMode,
    permits: Arc<Semaphore>,
) -> Response {
    analyze_request_with_job(
        body,
        mode,
        permits,
        |authored_json, include_route_report| {
            process_authored_json(&authored_json, include_route_report)
        },
    )
    .await
}

async fn analyze_request_with_job<F>(
    body: Bytes,
    mode: AnalysisMode,
    permits: Arc<Semaphore>,
    job: F,
) -> Response
where
    F: FnOnce(String, bool) -> Result<serde_json::Value, String> + Send + 'static,
{
    let authored_json = match String::from_utf8(body.to_vec()) {
        Ok(json) => json,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "Map JSON must be UTF-8."),
    };
    let Ok(permit) = permits.try_acquire_owned() else {
        return error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many map analyses are already running; try again shortly.",
        );
    };
    let include_route_report = matches!(mode, AnalysisMode::Report);
    let task = tokio::task::spawn_blocking(move || {
        run_analysis_job(permit, authored_json, include_route_report, job)
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

fn run_analysis_job<F>(
    permit: OwnedSemaphorePermit,
    authored_json: String,
    include_route_report: bool,
    job: F,
) -> Result<serde_json::Value, String>
where
    F: FnOnce(String, bool) -> Result<serde_json::Value, String>,
{
    let result = job(authored_json, include_route_report);
    drop(permit);
    result
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
    use std::time::Duration;
    use tokio::sync::oneshot;

    use super::*;

    fn valid_map_json() -> String {
        json!({
            "version": 7,
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
            "concealmentTiles": [],
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
        assert_eq!(report["truncated"], false);
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

    #[tokio::test]
    async fn saturated_analysis_admission_rejects_before_work_is_spawned() {
        let permits = Arc::new(Semaphore::new(0));
        let response = analyze_request_with_permits(
            Bytes::from(valid_map_json()),
            AnalysisMode::Report,
            permits,
        )
        .await;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let error = response_json(response).await;
        assert_eq!(error["valid"], false);
        assert!(error["error"]
            .as_str()
            .is_some_and(|value| value.contains("already running")));
    }

    #[tokio::test]
    async fn cancelled_handler_holds_admission_until_blocking_job_finishes() {
        let permits = Arc::new(Semaphore::new(1));
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let first = tokio::spawn(analyze_request_with_job(
            Bytes::from(valid_map_json()),
            AnalysisMode::Report,
            Arc::clone(&permits),
            move |_, _| {
                let _ = started_tx.send(());
                release_rx
                    .recv()
                    .expect("test should release the blocking analysis");
                Ok(json!({ "valid": true }))
            },
        ));

        tokio::time::timeout(Duration::from_secs(1), started_rx)
            .await
            .expect("blocking analysis should start")
            .expect("blocking analysis should signal its start");
        first.abort();
        assert!(first
            .await
            .expect_err("handler should be cancelled")
            .is_cancelled());
        assert_eq!(permits.available_permits(), 0);

        let saturated = analyze_request_with_permits(
            Bytes::from(valid_map_json()),
            AnalysisMode::Check,
            Arc::clone(&permits),
        )
        .await;
        assert_eq!(saturated.status(), StatusCode::TOO_MANY_REQUESTS);

        release_tx
            .send(())
            .expect("blocking analysis should still be running");
        let released = tokio::time::timeout(Duration::from_secs(1), permits.acquire())
            .await
            .expect("permit should release when blocking analysis returns")
            .expect("test semaphore should remain open");
        drop(released);
        assert_eq!(permits.available_permits(), 1);
    }
}
