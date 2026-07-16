//! Match-history and bounded diagnostics persistence backed by Postgres (Supabase).
//!
//! - The server is the only writer; clients never touch the DB.
//! - Writes happen at match end. Failures are logged but never propagate into the room task —
//!   the simulation must stay live even if the DB is down.
//! - Reads serve the front-page match-history table via `/api/matches`.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{PgPool, Row};

use rts_sim::game::replay::ReplayArtifactV1;

/// One match-history row to insert.
#[derive(Debug, Clone)]
pub struct MatchRecord {
    /// Stable live-match correlation id used to join a persisted AI observation to its logs.
    pub match_run_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_ms: i32,
    pub map_name: String,
    pub winner_name: Option<String>,
    pub outcome: MatchOutcome,
    pub participants: Vec<String>,
    /// Full PlayerScore[] from `Game::scores()`, opaque JSON to the DB.
    pub score_screen: serde_json::Value,
    /// Number of non-AI players in the match. Public recent-match reads hide rows with none.
    pub human_count: i32,
    /// Visibility flag for debug rows. One-human, no-AI sandbox rows write true.
    pub debug_mode: bool,
    /// True for developer-local rows that should only be visible from localhost requests.
    pub local_only: bool,
    /// Optional deterministic replay artifact for replay launch.
    pub replay: Option<MatchReplayRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchOutcome {
    Win,
    Draw,
    Aborted,
}

impl MatchOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Win => "win",
            Self::Draw => "draw",
            Self::Aborted => "aborted",
        }
    }

    pub fn from_winner_name(winner_name: Option<&str>) -> Self {
        if winner_name.is_some() {
            Self::Win
        } else {
            Self::Draw
        }
    }
}

/// One replay artifact row to insert alongside a match-history row.
#[derive(Debug, Clone)]
pub struct MatchReplayRecord {
    pub artifact_schema_version: i32,
    pub build_sha: String,
    pub map_name: String,
    pub map_schema_version: i32,
    pub map_hash: String,
    pub duration_ticks: i32,
    pub artifact_json: serde_json::Value,
}

impl MatchReplayRecord {
    pub fn from_artifact(artifact: &ReplayArtifactV1) -> Result<Self, serde_json::Error> {
        Ok(Self {
            artifact_schema_version: i32::try_from(artifact.artifact_schema_version)
                .unwrap_or(i32::MAX),
            build_sha: artifact.server_build_sha.clone(),
            map_name: artifact.map_name.clone(),
            map_schema_version: i32::try_from(artifact.map_schema_version).unwrap_or(i32::MAX),
            map_hash: artifact.map_content_hash.clone(),
            duration_ticks: i32::try_from(artifact.duration_ticks).unwrap_or(i32::MAX),
            artifact_json: serde_json::to_value(artifact)?,
        })
    }
}

/// One match-history row returned to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchSummary {
    pub id: i64,
    /// One-based position among the visible Recent Matches history, oldest first.
    #[serde(rename = "replayNumber", skip_serializing_if = "Option::is_none")]
    pub replay_number: Option<i64>,
    #[serde(rename = "matchRunId", skip_serializing_if = "Option::is_none")]
    pub match_run_id: Option<String>,
    #[serde(rename = "startedAt")]
    pub started_at: DateTime<Utc>,
    #[serde(rename = "endedAt")]
    pub ended_at: DateTime<Utc>,
    #[serde(rename = "durationMs")]
    pub duration_ms: i32,
    #[serde(rename = "mapName")]
    pub map_name: String,
    #[serde(rename = "winnerName")]
    pub winner_name: Option<String>,
    pub outcome: String,
    pub participants: Vec<String>,
    #[serde(rename = "scoreScreen")]
    pub score_screen: serde_json::Value,
    #[serde(rename = "humanCount")]
    pub human_count: i32,
    #[serde(rename = "debugMode")]
    pub debug_mode: bool,
    #[serde(rename = "localOnly")]
    pub local_only: bool,
    #[serde(rename = "replayAvailable")]
    pub replay_available: bool,
    #[serde(rename = "replayUnavailableReason")]
    pub replay_unavailable_reason: Option<String>,
    #[serde(skip)]
    pub replay_metadata: Option<ReplaySummaryMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaySummaryMetadata {
    pub artifact_schema_version: i32,
    pub build_sha: String,
    pub map_name: String,
    pub map_schema_version: i32,
    pub map_hash: String,
}

#[derive(Clone)]
pub struct Db {
    pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct ClientStressTestRecord {
    pub run_id: String,
    pub artifact_label: String,
    pub received_at: DateTime<Utc>,
    pub build_id: String,
    pub status: String,
    pub user_label: String,
    pub device_id: String,
    pub fingerprint: String,
    pub platform: String,
    pub average_fps_x100: i32,
    pub frame_work_p95_ms: i32,
    pub renderer_p95_ms: i32,
    pub profile_kind: String,
    pub profile_sample_count: i32,
    pub artifact_json: serde_json::Value,
}

impl Db {
    /// Connect to Postgres, run migrations, and return a pool wrapper.
    ///
    /// The pool is bounded so we never exhaust Supabase's free-tier connection budget.
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect(database_url)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;
        crate::log_info!("database connected and migrations applied");
        Ok(Self { pool })
    }

    pub async fn record_client_stress_test(
        &self,
        rec: &ClientStressTestRecord,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            insert into client_stress_tests
                (run_id, artifact_label, received_at, build_id, status, user_label,
                 device_id, fingerprint, platform, average_fps_x100, frame_work_p95_ms,
                 renderer_p95_ms, profile_kind, profile_sample_count, artifact_json)
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
        )
        .bind(&rec.run_id)
        .bind(&rec.artifact_label)
        .bind(rec.received_at)
        .bind(&rec.build_id)
        .bind(&rec.status)
        .bind(&rec.user_label)
        .bind(&rec.device_id)
        .bind(&rec.fingerprint)
        .bind(&rec.platform)
        .bind(rec.average_fps_x100)
        .bind(rec.frame_work_p95_ms)
        .bind(rec.renderer_p95_ms)
        .bind(&rec.profile_kind)
        .bind(rec.profile_sample_count)
        .bind(&rec.artifact_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn client_stress_test_by_run_id(
        &self,
        run_id: &str,
    ) -> Result<Option<serde_json::Value>, sqlx::Error> {
        sqlx::query_scalar("select artifact_json from client_stress_tests where run_id = $1")
            .bind(run_id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Insert one match-history row. Logs on error and swallows it — the caller never blocks.
    pub async fn record_match(&self, rec: MatchRecord) {
        let outcome = rec.outcome.as_str();
        let result: Result<(), sqlx::Error> = async {
            let mut tx = self.pool.begin().await?;
            let match_id: i64 = sqlx::query_scalar(
                r#"
            insert into matches
                (match_run_id, started_at, ended_at, duration_ms, map_name,
                 winner_name, outcome, participants, score_screen,
                 human_count, debug_mode, local_only)
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            returning id
            "#,
            )
            .bind(rec.match_run_id.as_deref())
            .bind(rec.started_at)
            .bind(rec.ended_at)
            .bind(rec.duration_ms)
            .bind(&rec.map_name)
            .bind(rec.winner_name.as_deref())
            .bind(outcome)
            .bind(&rec.participants)
            .bind(&rec.score_screen)
            .bind(rec.human_count)
            .bind(rec.debug_mode)
            .bind(rec.local_only)
            .fetch_one(&mut *tx)
            .await?;

            if let Some(replay) = &rec.replay {
                sqlx::query(
                    r#"
                    insert into match_replays
                        (match_id, artifact_schema_version, build_sha, map_name,
                         map_schema_version, map_hash, duration_ticks, artifact_json)
                    values ($1, $2, $3, $4, $5, $6, $7, $8)
                    "#,
                )
                .bind(match_id)
                .bind(replay.artifact_schema_version)
                .bind(&replay.build_sha)
                .bind(&replay.map_name)
                .bind(replay.map_schema_version)
                .bind(&replay.map_hash)
                .bind(replay.duration_ticks)
                .bind(&replay.artifact_json)
                .execute(&mut *tx)
                .await?;
            }

            tx.commit().await
        }
        .await;

        match result {
            Ok(_) => {
                crate::log_info!(
                    match_run_id = rec.match_run_id.as_deref().unwrap_or(""),
                    map = %rec.map_name,
                    outcome = outcome,
                    local_only = rec.local_only,
                    replay = rec.replay.is_some(),
                    "match recorded"
                )
            }
            Err(err) => crate::log_error!(
                %err,
                match_run_id = rec.match_run_id.as_deref().unwrap_or(""),
                map = %rec.map_name,
                "failed to record match"
            ),
        }
    }

    /// Return the most recent matches in newest-first order. `limit` is clamped to [1, 100].
    pub async fn recent_matches(
        &self,
        limit: i64,
        include_local: bool,
    ) -> Result<Vec<MatchSummary>, sqlx::Error> {
        let limit = limit.clamp(1, 100);
        let rows = sqlx::query(
            r#"
            with visible_matches as (
            select matches.id as id,
                   row_number() over (order by matches.started_at asc, matches.id asc) as replay_number,
                   matches.match_run_id as match_run_id,
                   matches.started_at as started_at,
                   matches.ended_at as ended_at,
                   matches.duration_ms as duration_ms,
                   matches.map_name as map_name,
                   matches.winner_name as winner_name,
                   matches.outcome as outcome,
                   matches.participants as participants,
                   matches.score_screen as score_screen,
                   matches.human_count as human_count,
                   matches.debug_mode as debug_mode,
                   matches.local_only as local_only,
                   r.artifact_schema_version as replay_artifact_schema_version,
                   r.build_sha as replay_build_sha,
                   r.map_name as replay_map_name,
                   r.map_schema_version as replay_map_schema_version,
                   r.map_hash as replay_map_hash
            from matches
            left join match_replays r on r.match_id = matches.id
            where ($2 or not matches.local_only)
              and matches.human_count >= 1
              and not matches.debug_mode
              and not (
                matches.human_count = 1
                and cardinality(matches.participants) = 1
              )
              and (
                $2
                or (
                  not exists (
                    select 1
                    from unnest(participants) as participant(name)
                    where lower(participant.name) = 'smoke'
                  )
                  and not (participants @> array['Alpha', 'Bravo']::text[])
                )
              )
            )
            select *
            from visible_matches
            order by started_at desc, id desc
            limit $1
            "#,
        )
        .bind(limit)
        .bind(include_local)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(row_to_summary).collect())
    }

    /// Return one AI-only observation by its match-run id. These rows deliberately stay out of
    /// the public Recent Matches feed, but the run id shown after a watched match is enough to
    /// recover the replay and the matching structured server logs.
    pub async fn observation_by_run_id(
        &self,
        match_run_id: &str,
        include_local: bool,
    ) -> Result<Option<MatchSummary>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            select matches.id as id,
                   matches.match_run_id as match_run_id,
                   matches.started_at as started_at,
                   matches.ended_at as ended_at,
                   matches.duration_ms as duration_ms,
                   matches.map_name as map_name,
                   matches.winner_name as winner_name,
                   matches.outcome as outcome,
                   matches.participants as participants,
                   matches.score_screen as score_screen,
                   matches.human_count as human_count,
                   matches.debug_mode as debug_mode,
                   matches.local_only as local_only,
                   r.artifact_schema_version as replay_artifact_schema_version,
                   r.build_sha as replay_build_sha,
                   r.map_name as replay_map_name,
                   r.map_schema_version as replay_map_schema_version,
                   r.map_hash as replay_map_hash
            from matches
            left join match_replays r on r.match_id = matches.id
            where matches.match_run_id = $1
              and matches.human_count = 0
              and not matches.debug_mode
              and ($2 or not matches.local_only)
            "#,
        )
        .bind(match_run_id)
        .bind(include_local)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(row_to_summary))
    }

    /// Load a persisted replay artifact for a match visible to this request scope.
    pub async fn replay_artifact_for_match(
        &self,
        match_id: i64,
        include_local: bool,
    ) -> Result<Option<ReplayArtifactV1>, ReplayLoadError> {
        let row = sqlx::query(
            r#"
            select r.artifact_json
            from match_replays r
            join matches m on m.id = r.match_id
            where m.id = $1
              and ($2 or not m.local_only)
            "#,
        )
        .bind(match_id)
        .bind(include_local)
        .fetch_optional(&self.pool)
        .await
        .map_err(ReplayLoadError::Db)?;

        let Some(row) = row else {
            return Ok(None);
        };
        let value: serde_json::Value = row.get("artifact_json");
        serde_json::from_value(value)
            .map(Some)
            .map_err(ReplayLoadError::Decode)
    }
}

fn row_to_summary(row: PgRow) -> MatchSummary {
    let replay_metadata = row
        .try_get::<i32, _>("replay_artifact_schema_version")
        .ok()
        .map(|artifact_schema_version| ReplaySummaryMetadata {
            artifact_schema_version,
            build_sha: row.get("replay_build_sha"),
            map_name: row.get("replay_map_name"),
            map_schema_version: row.get("replay_map_schema_version"),
            map_hash: row.get("replay_map_hash"),
        });
    MatchSummary {
        id: row.get("id"),
        replay_number: row.try_get("replay_number").ok(),
        match_run_id: row.get("match_run_id"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
        duration_ms: row.get("duration_ms"),
        map_name: row.get("map_name"),
        winner_name: row.get("winner_name"),
        outcome: row.get("outcome"),
        participants: row.get("participants"),
        score_screen: row.get("score_screen"),
        human_count: row.get("human_count"),
        debug_mode: row.get("debug_mode"),
        local_only: row.get("local_only"),
        replay_available: replay_metadata.is_some(),
        replay_unavailable_reason: replay_metadata
            .is_none()
            .then(|| "Replay was not recorded for this match.".to_string()),
        replay_metadata,
    }
}

#[derive(Debug)]
pub enum ReplayLoadError {
    Db(sqlx::Error),
    Decode(serde_json::Error),
}

impl std::fmt::Display for ReplayLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayLoadError::Db(err) => write!(f, "{err}"),
            ReplayLoadError::Decode(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ReplayLoadError {}

/// Best-effort connect at boot. Logs and returns `None` on failure so the server stays usable
/// without a database (dev environments, transient outages).
pub async fn try_connect_from_env() -> Option<Arc<Db>> {
    let url = match std::env::var("DATABASE_URL") {
        Ok(u) if !u.trim().is_empty() => u,
        _ => {
            crate::log_warn!("DATABASE_URL not set; database-backed features disabled");
            return None;
        }
    };
    match Db::connect(&url).await {
        Ok(db) => Some(Arc::new(db)),
        Err(err) => {
            crate::log_error!(%err, "failed to connect to database; database-backed features disabled");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MatchOutcome, MatchRecord, MatchSummary};

    fn record_with_outcome(outcome: MatchOutcome) -> MatchRecord {
        MatchRecord {
            match_run_id: None,
            started_at: chrono::Utc::now(),
            ended_at: chrono::Utc::now(),
            duration_ms: 1_000,
            map_name: "Default".to_string(),
            winner_name: None,
            outcome,
            participants: vec!["Alpha".to_string(), "Bravo".to_string()],
            score_screen: serde_json::Value::Array(Vec::new()),
            human_count: 2,
            debug_mode: false,
            local_only: false,
            replay: None,
        }
    }

    #[test]
    fn match_record_outcome_is_explicit() {
        let draw = record_with_outcome(MatchOutcome::Draw);
        let aborted = record_with_outcome(MatchOutcome::Aborted);

        assert_eq!(draw.winner_name, None);
        assert_eq!(draw.outcome.as_str(), "draw");
        assert_eq!(aborted.winner_name, None);
        assert_eq!(aborted.outcome.as_str(), "aborted");
    }

    #[test]
    fn normal_match_outcome_can_be_derived_before_recording() {
        assert_eq!(
            MatchOutcome::from_winner_name(Some("Alpha")),
            MatchOutcome::Win
        );
        assert_eq!(MatchOutcome::from_winner_name(None), MatchOutcome::Draw);
    }

    #[test]
    fn match_summary_serializes_aborted_outcome() {
        let summary = MatchSummary {
            id: 1,
            replay_number: Some(42),
            match_run_id: Some("ai-observation-123".to_string()),
            started_at: chrono::Utc::now(),
            ended_at: chrono::Utc::now(),
            duration_ms: 1_000,
            map_name: "Default".to_string(),
            winner_name: None,
            outcome: "aborted".to_string(),
            participants: vec!["Alpha".to_string(), "Bravo".to_string()],
            score_screen: serde_json::Value::Array(Vec::new()),
            human_count: 2,
            debug_mode: false,
            local_only: false,
            replay_available: true,
            replay_unavailable_reason: None,
            replay_metadata: None,
        };

        let value = serde_json::to_value(summary).expect("summary serializes");
        assert_eq!(value["winnerName"], serde_json::Value::Null);
        assert_eq!(value["outcome"], "aborted");
        assert_eq!(value["matchRunId"], "ai-observation-123");
        assert_eq!(value["replayNumber"], 42);
    }
}
