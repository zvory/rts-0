use std::collections::VecDeque;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use rts_server::db::{ClientStressTestRecord, Db};

use crate::AppState;

pub const MAX_SUBMISSION_BYTES: usize = 2 * 1024 * 1024;
const MAX_CACHE_ENTRIES: usize = 64;
const MAX_FLAMEGRAPH_BYTES: usize = 750 * 1024;

#[derive(Clone)]
pub struct StressTestStore {
    db: Option<Arc<Db>>,
    recent: Arc<Mutex<VecDeque<CachedArtifact>>>,
}

#[derive(Clone)]
struct CachedArtifact {
    run_id: String,
    artifact_label: String,
    artifact: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StressTestSubmission {
    schema_version: u8,
    workload_id: String,
    #[serde(default)]
    user_label: String,
    device_id: String,
    fingerprint: String,
    measured_at: String,
    measured_duration_ms: u32,
    status: String,
    #[serde(default)]
    invalid_reasons: Vec<String>,
    environment: Value,
    stream: Value,
    frame_summary: Value,
    browser_timing: Value,
    profile: ProfileSubmission,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileSubmission {
    kind: String,
    supported: bool,
    #[serde(default)]
    error: String,
    trace: Option<Value>,
    summary: Option<Value>,
    #[serde(default)]
    flamegraph_svg: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveResponse {
    run_id: String,
    artifact_label: String,
    persisted: bool,
    result_url: String,
    flamegraph_url: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl StressTestStore {
    pub fn new(db: Option<Arc<Db>>) -> Self {
        Self {
            db,
            recent: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_CACHE_ENTRIES))),
        }
    }

    async fn save(
        &self,
        submission: StressTestSubmission,
        build_id: &str,
    ) -> Result<SaveResponse, String> {
        validate_submission(&submission)?;
        let received_at = Utc::now();
        let run_id = create_run_id(received_at.timestamp_millis());
        let artifact_label = artifact_label(&submission, received_at, &run_id);
        let frame_work_p95_ms = headline_u16(&submission.frame_summary, "frameWorkP95Ms");
        let renderer_p95_ms = headline_u16(&submission.frame_summary, "rendererP95Ms");
        let frame_count = submission
            .frame_summary
            .get("frameCount")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(1_000_000);
        let average_fps_x100 = ((frame_count as f64 * 100_000.0)
            / f64::from(submission.measured_duration_ms))
        .round()
        .clamp(0.0, 1_000_000.0) as i32;
        let profile_sample_count = submission
            .profile
            .summary
            .as_ref()
            .and_then(|summary| summary.get("sampleCount"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(10_000) as i32;
        let mut artifact = serde_json::to_value(&submission)
            .map_err(|_| "The diagnostics could not be encoded.".to_string())?;
        artifact["server"] = json!({
            "runId": run_id,
            "artifactLabel": artifact_label,
            "receivedAt": received_at,
            "buildId": build_id,
            "persisted": self.db.is_some(),
        });

        let record = ClientStressTestRecord {
            run_id: run_id.clone(),
            artifact_label: artifact_label.clone(),
            received_at,
            build_id: build_id.to_string(),
            status: submission.status.clone(),
            user_label: submission.user_label.clone(),
            device_id: submission.device_id.clone(),
            fingerprint: submission.fingerprint.clone(),
            platform: json_string(&submission.environment, "platform", 120),
            average_fps_x100,
            frame_work_p95_ms,
            renderer_p95_ms,
            profile_kind: submission.profile.kind.clone(),
            profile_sample_count,
            artifact_json: artifact.clone(),
        };
        let persisted = if let Some(db) = &self.db {
            match db.record_client_stress_test(&record).await {
                Ok(()) => true,
                Err(err) => {
                    rts_server::log_error!(
                        %err,
                        run_id = %run_id,
                        artifact_label = %artifact_label,
                        "failed to persist client stress test"
                    );
                    artifact["server"]["persisted"] = Value::Bool(false);
                    false
                }
            }
        } else {
            artifact["server"]["persisted"] = Value::Bool(false);
            false
        };

        self.cache(CachedArtifact {
            run_id: run_id.clone(),
            artifact_label: artifact_label.clone(),
            artifact,
        })
        .await;
        rts_server::log_info!(
            run_id = %run_id,
            artifact_label = %artifact_label,
            build_id,
            status = %submission.status,
            workload = %submission.workload_id,
            average_fps_x100,
            frame_work_p95_ms,
            renderer_p95_ms,
            profile_kind = %submission.profile.kind,
            profile_sample_count,
            persisted,
            "client stress test recorded"
        );

        Ok(SaveResponse {
            result_url: format!("/api/stress-tests/{run_id}"),
            flamegraph_url: format!("/api/stress-tests/{run_id}/flamegraph.svg"),
            run_id,
            artifact_label,
            persisted,
        })
    }

    async fn cache(&self, entry: CachedArtifact) {
        let mut recent = self.recent.lock().await;
        recent.push_front(entry);
        recent.truncate(MAX_CACHE_ENTRIES);
    }

    async fn get(&self, run_id: &str) -> Option<CachedArtifact> {
        if let Some(hit) = self
            .recent
            .lock()
            .await
            .iter()
            .find(|entry| entry.run_id == run_id)
            .cloned()
        {
            return Some(hit);
        }
        let db = self.db.as_ref()?;
        match db.client_stress_test_by_run_id(run_id).await {
            Ok(Some(artifact)) => {
                let artifact_label = artifact
                    .pointer("/server/artifactLabel")
                    .and_then(Value::as_str)
                    .unwrap_or("stress-test")
                    .to_string();
                let entry = CachedArtifact {
                    run_id: run_id.to_string(),
                    artifact_label,
                    artifact,
                };
                self.cache(entry.clone()).await;
                Some(entry)
            }
            Ok(None) => None,
            Err(err) => {
                rts_server::log_error!(%err, run_id, "failed to load client stress test");
                None
            }
        }
    }
}

pub async fn create_handler(
    State(state): State<AppState>,
    Json(submission): Json<StressTestSubmission>,
) -> Response {
    match state.stress_tests.save(submission, &state.version).await {
        Ok(saved) => (StatusCode::CREATED, Json(saved)).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })).into_response(),
    }
}

pub async fn artifact_handler(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Response {
    if !valid_run_id(&run_id) {
        return (StatusCode::NOT_FOUND, "stress test not found").into_response();
    }
    match state.stress_tests.get(&run_id).await {
        Some(entry) => {
            let filename = format!("{}-result.json", entry.artifact_label);
            let disposition =
                HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
                    .unwrap_or_else(|_| {
                        HeaderValue::from_static("attachment; filename=\"stress-test-result.json\"")
                    });
            (
                [
                    (header::CONTENT_DISPOSITION, disposition),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("private, no-store"),
                    ),
                ],
                Json(entry.artifact),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "stress test not found").into_response(),
    }
}

pub async fn flamegraph_handler(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Response {
    if !valid_run_id(&run_id) {
        return (StatusCode::NOT_FOUND, "stress test not found").into_response();
    }
    let Some(entry) = state.stress_tests.get(&run_id).await else {
        return (StatusCode::NOT_FOUND, "stress test not found").into_response();
    };
    let Some(svg) = entry
        .artifact
        .pointer("/profile/flamegraphSvg")
        .and_then(Value::as_str)
        .filter(|svg| !svg.is_empty())
    else {
        return (StatusCode::NOT_FOUND, "flame graph not available").into_response();
    };
    let filename = format!("{}-flamegraph.svg", entry.artifact_label);
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
        .unwrap_or_else(|_| {
            HeaderValue::from_static("attachment; filename=\"stress-test-flamegraph.svg\"")
        });
    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("image/svg+xml; charset=utf-8"),
            ),
            (header::CONTENT_DISPOSITION, disposition),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, no-store"),
            ),
        ],
        svg.to_string(),
    )
        .into_response()
}

fn validate_submission(submission: &StressTestSubmission) -> Result<(), String> {
    if submission.schema_version != 1 || submission.workload_id != "supply-300-hellhole" {
        return Err("Unsupported stress-test workload or schema.".to_string());
    }
    if submission.user_label.len() > 64
        || !submission
            .user_label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '.' | '_' | '-'))
    {
        return Err("The artifact label is invalid.".to_string());
    }
    if !(16..=64).contains(&submission.device_id.len())
        || !submission
            .device_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        || !(8..=64).contains(&submission.fingerprint.len())
        || !submission
            .fingerprint
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric())
    {
        return Err("The device fingerprint is invalid.".to_string());
    }
    if !(1_500..=30_000).contains(&submission.measured_duration_ms) {
        return Err("The measurement duration is outside the supported range.".to_string());
    }
    if !matches!(submission.status.as_str(), "completed" | "invalid") {
        return Err("The stress-test status is invalid.".to_string());
    }
    if submission.measured_at.len() > 64
        || submission.invalid_reasons.len() > 8
        || submission
            .invalid_reasons
            .iter()
            .any(|reason| reason.len() > 120)
    {
        return Err("The measurement metadata is too large.".to_string());
    }
    if !matches!(
        submission.profile.kind.as_str(),
        "js-self-profile" | "phase-timings"
    ) || submission.profile.error.len() > 240
        || submission.profile.flamegraph_svg.len() > MAX_FLAMEGRAPH_BYTES
        || !safe_flamegraph_svg(&submission.profile.flamegraph_svg)
    {
        return Err("The browser profile is invalid or too large.".to_string());
    }
    if submission.profile.kind == "js-self-profile" {
        let Some(trace) = submission.profile.trace.as_ref() else {
            return Err("The JS profile trace is missing.".to_string());
        };
        for (key, cap) in [
            ("samples", 4_000),
            ("stacks", 12_000),
            ("frames", 12_000),
            ("resources", 1_000),
        ] {
            if trace
                .get(key)
                .and_then(Value::as_array)
                .is_none_or(|rows| rows.len() > cap)
            {
                return Err(format!(
                    "The JS profile {key} table is invalid or too large."
                ));
            }
        }
    }
    Ok(())
}

fn create_run_id(timestamp_ms: i64) -> String {
    let mut random = rand::thread_rng();
    format!(
        "stress-{timestamp_ms}-{:016x}{:016x}",
        random.next_u64(),
        random.next_u64()
    )
}

fn safe_flamegraph_svg(svg: &str) -> bool {
    if svg.is_empty() {
        return true;
    }
    let lowercase = svg.to_ascii_lowercase();
    (lowercase.starts_with("<?xml") || lowercase.starts_with("<svg"))
        && lowercase.contains("<svg")
        && ![
            "<script",
            "<foreignobject",
            "<image",
            "<a ",
            "javascript:",
            "onload=",
            "onerror=",
            "href=",
            "xlink",
        ]
        .iter()
        .any(|needle| lowercase.contains(needle))
}

fn artifact_label(
    submission: &StressTestSubmission,
    received_at: chrono::DateTime<Utc>,
    run_id: &str,
) -> String {
    let explicit_identity = slug(&submission.user_label, 32);
    let identity = if explicit_identity.is_empty() {
        submission.fingerprint.chars().take(12).collect::<String>()
    } else {
        explicit_identity
    };
    let platform = slug(&json_string(&submission.environment, "platform", 120), 20);
    let suffix = run_id
        .chars()
        .rev()
        .take(8)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!(
        "hellhole-{}-{}-{}-{}",
        identity,
        if platform.is_empty() {
            "unknown"
        } else {
            &platform
        },
        received_at.format("%Y%m%dT%H%M%SZ"),
        suffix,
    )
}

fn slug(value: &str, max_len: usize) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(max_len)
        .collect()
}

fn valid_run_id(run_id: &str) -> bool {
    run_id.len() >= 40
        && run_id.len() <= 64
        && run_id.starts_with("stress-")
        && run_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

fn json_string(value: &Value, key: &str, max_len: usize) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .take(max_len)
        .collect()
}

fn headline_u16(value: &Value, key: &str) -> i32 {
    value
        .get(key)
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .round()
        .clamp(0.0, f64::from(u16::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> StressTestSubmission {
        StressTestSubmission {
            schema_version: 1,
            workload_id: "supply-300-hellhole".to_string(),
            user_label: "Matt laptop".to_string(),
            device_id: "0123456789abcdef".to_string(),
            fingerprint: "0123456789abcdef".to_string(),
            measured_at: "2026-07-15T12:00:00Z".to_string(),
            measured_duration_ms: 15_000,
            status: "completed".to_string(),
            invalid_reasons: vec![],
            environment: json!({"platform": "Windows"}),
            stream: json!({"offline": true}),
            frame_summary: json!({"frameWorkP95Ms": 16}),
            browser_timing: json!({}),
            profile: ProfileSubmission {
                kind: "phase-timings".to_string(),
                supported: false,
                error: "unsupported".to_string(),
                trace: None,
                summary: None,
                flamegraph_svg: String::new(),
            },
        }
    }

    #[test]
    fn validates_the_bounded_fallback_report() {
        assert!(validate_submission(&fixture()).is_ok());
    }

    #[test]
    fn rejects_unknown_workloads_and_oversized_flamegraphs() {
        let mut submission = fixture();
        submission.workload_id = "other".to_string();
        assert!(validate_submission(&submission).is_err());
        submission = fixture();
        submission.profile.flamegraph_svg = "x".repeat(MAX_FLAMEGRAPH_BYTES + 1);
        assert!(validate_submission(&submission).is_err());
        submission = fixture();
        submission.profile.flamegraph_svg = "<svg onload=\"alert(1)\"></svg>".to_string();
        assert!(validate_submission(&submission).is_err());
    }

    #[test]
    fn artifact_names_are_filename_safe_and_labeled() {
        let submission = fixture();
        let label = artifact_label(
            &submission,
            Utc::now(),
            "stress-1234567890-1234567890abcdef",
        );
        assert!(label.starts_with("hellhole-matt-laptop-windows-"));
        assert!(label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-'));
    }

    #[tokio::test]
    async fn local_store_saves_and_retrieves_without_a_database() {
        let store = StressTestStore::new(None);
        let saved = store.save(fixture(), "build-123").await.unwrap();
        assert!(!saved.persisted);
        assert!(valid_run_id(&saved.run_id));
        let artifact = store.get(&saved.run_id).await.unwrap();
        assert_eq!(artifact.artifact["server"]["runId"], saved.run_id);
        assert_eq!(artifact.artifact["server"]["persisted"], false);
    }
}
