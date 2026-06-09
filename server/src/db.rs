//! Match-history persistence backed by Postgres (Supabase).
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
use tracing::{error, info, warn};

/// One match-history row to insert.
#[derive(Debug, Clone)]
pub struct MatchRecord {
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_ms: i32,
    pub map_name: String,
    pub winner_name: Option<String>,
    pub participants: Vec<String>,
    /// Full PlayerScore[] from `Game::scores()`, opaque JSON to the DB.
    pub score_screen: serde_json::Value,
}

impl MatchRecord {
    pub fn outcome(&self) -> &'static str {
        if self.winner_name.is_some() {
            "win"
        } else {
            "draw"
        }
    }
}

/// One match-history row returned to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchSummary {
    pub id: i64,
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
}

#[derive(Clone)]
pub struct Db {
    pool: PgPool,
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
        info!("database connected and migrations applied");
        Ok(Self { pool })
    }

    /// Insert one match-history row. Logs on error and swallows it — the caller never blocks.
    pub async fn record_match(&self, rec: MatchRecord) {
        let outcome = rec.outcome();
        let result = sqlx::query(
            r#"
            insert into matches
                (started_at, ended_at, duration_ms, map_name,
                 winner_name, outcome, participants, score_screen)
            values ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(rec.started_at)
        .bind(rec.ended_at)
        .bind(rec.duration_ms)
        .bind(&rec.map_name)
        .bind(rec.winner_name.as_deref())
        .bind(outcome)
        .bind(&rec.participants)
        .bind(&rec.score_screen)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => info!(map = %rec.map_name, outcome, "match recorded"),
            Err(err) => error!(%err, map = %rec.map_name, "failed to record match"),
        }
    }

    /// Return the most recent matches in newest-first order. `limit` is clamped to [1, 100].
    pub async fn recent_matches(&self, limit: i64) -> Result<Vec<MatchSummary>, sqlx::Error> {
        let limit = limit.clamp(1, 100);
        let rows = sqlx::query(
            r#"
            select id, started_at, ended_at, duration_ms, map_name,
                   winner_name, outcome, participants, score_screen
            from matches
            order by started_at desc
            limit $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(row_to_summary).collect())
    }
}

fn row_to_summary(row: PgRow) -> MatchSummary {
    MatchSummary {
        id: row.get("id"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
        duration_ms: row.get("duration_ms"),
        map_name: row.get("map_name"),
        winner_name: row.get("winner_name"),
        outcome: row.get("outcome"),
        participants: row.get("participants"),
        score_screen: row.get("score_screen"),
    }
}

/// Best-effort connect at boot. Logs and returns `None` on failure so the server stays usable
/// without a database (dev environments, transient outages).
pub async fn try_connect_from_env() -> Option<Arc<Db>> {
    let url = match std::env::var("DATABASE_URL") {
        Ok(u) if !u.trim().is_empty() => u,
        _ => {
            warn!("DATABASE_URL not set; match history disabled");
            return None;
        }
    };
    match Db::connect(&url).await {
        Ok(db) => Some(Arc::new(db)),
        Err(err) => {
            error!(%err, "failed to connect to database; match history disabled");
            None
        }
    }
}
