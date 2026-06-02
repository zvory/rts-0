//! Application observability: structured lag logs plus optional Datadog metrics.
//!
//! Metrics are intentionally disabled unless an explicit Datadog endpoint is configured. That
//! keeps local development from emitting telemetry while still letting Fly deployments opt in with
//! secrets/env vars.

use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;
use tracing::{debug, info, warn};

use crate::config;
use crate::protocol::ClientPerfReport;

const SERVICE_NAME: &str = "rts";
const METRIC_CHANNEL_CAP: usize = 4096;
const DATADOG_API_FLUSH_INTERVAL: Duration = Duration::from_secs(10);
const DATADOG_API_MAX_BATCH_POINTS: usize = 500;
const DEFAULT_SLOW_CLIENT_FPS: f32 = 45.0;
const DEFAULT_SLOW_CLIENT_FRAME_MS: f32 = 100.0;
const DEFAULT_SLOW_CLIENT_SNAPSHOT_GAP_MS: f32 = 250.0;
const DEFAULT_SLOW_CLIENT_RTT_MS: f32 = 250.0;

static GLOBAL: OnceLock<Arc<Observability>> = OnceLock::new();

pub fn init(version: String) -> Arc<Observability> {
    let obs = Arc::new(Observability::from_env(version));
    if GLOBAL.set(obs.clone()).is_err() {
        warn!("observability was already initialized; keeping the first instance");
    }
    obs
}

pub fn global() -> &'static Arc<Observability> {
    GLOBAL.get_or_init(|| Arc::new(Observability::disabled("unknown".to_string())))
}

pub struct Observability {
    version: String,
    env: String,
    metrics: Option<MetricsSink>,
    active_connections: AtomicI64,
    connected_players: AtomicI64,
    active_rooms: AtomicI64,
    active_matches: AtomicI64,
    slow_tick_ms: f64,
    slow_client_fps: f32,
    slow_client_frame_ms: f32,
    slow_client_snapshot_gap_ms: f32,
    slow_client_rtt_ms: f32,
}

impl Observability {
    fn from_env(version: String) -> Self {
        let env_name = env::var("DD_ENV")
            .or_else(|_| env::var("RTS_ENV"))
            .unwrap_or_else(|_| {
                if env::var("FLY_APP_NAME").is_ok() {
                    "prod".to_string()
                } else {
                    "local".to_string()
                }
            });
        let default_tags = default_tags(&env_name);
        let metrics = MetricsSink::from_env(default_tags);
        let slow_tick_ms = env_f64("RTS_SLOW_TICK_MS").unwrap_or(config::TICK_MS as f64);
        let obs = Self {
            version,
            env: env_name,
            metrics,
            active_connections: AtomicI64::new(0),
            connected_players: AtomicI64::new(0),
            active_rooms: AtomicI64::new(0),
            active_matches: AtomicI64::new(0),
            slow_tick_ms,
            slow_client_fps: env_f32("RTS_SLOW_CLIENT_FPS").unwrap_or(DEFAULT_SLOW_CLIENT_FPS),
            slow_client_frame_ms: env_f32("RTS_SLOW_CLIENT_FRAME_MS")
                .unwrap_or(DEFAULT_SLOW_CLIENT_FRAME_MS),
            slow_client_snapshot_gap_ms: env_f32("RTS_SLOW_CLIENT_SNAPSHOT_GAP_MS")
                .unwrap_or(DEFAULT_SLOW_CLIENT_SNAPSHOT_GAP_MS),
            slow_client_rtt_ms: env_f32("RTS_SLOW_CLIENT_RTT_MS")
                .unwrap_or(DEFAULT_SLOW_CLIENT_RTT_MS),
        };
        info!(
            service = SERVICE_NAME,
            env = %obs.env,
            version = %obs.version,
            metrics_sink = obs.metrics.as_ref().map(MetricsSink::name).unwrap_or("disabled"),
            slow_tick_ms = obs.slow_tick_ms,
            "observability configured"
        );
        obs
    }

    fn disabled(version: String) -> Self {
        Self {
            version,
            env: "local".to_string(),
            metrics: None,
            active_connections: AtomicI64::new(0),
            connected_players: AtomicI64::new(0),
            active_rooms: AtomicI64::new(0),
            active_matches: AtomicI64::new(0),
            slow_tick_ms: config::TICK_MS as f64,
            slow_client_fps: DEFAULT_SLOW_CLIENT_FPS,
            slow_client_frame_ms: DEFAULT_SLOW_CLIENT_FRAME_MS,
            slow_client_snapshot_gap_ms: DEFAULT_SLOW_CLIENT_SNAPSHOT_GAP_MS,
            slow_client_rtt_ms: DEFAULT_SLOW_CLIENT_RTT_MS,
        }
    }

    pub fn connection_opened(&self) {
        self.gauge_atomic("rts.connections.active", &self.active_connections, 1);
        self.count("rts.connections.opened", 1.0, &[]);
    }

    pub fn connection_closed(&self) {
        self.gauge_atomic("rts.connections.active", &self.active_connections, -1);
        self.count("rts.connections.closed", 1.0, &[]);
    }

    pub fn player_joined(&self) {
        self.gauge_atomic("rts.players.connected", &self.connected_players, 1);
    }

    pub fn player_left(&self) {
        self.gauge_atomic("rts.players.connected", &self.connected_players, -1);
    }

    pub fn room_activated(&self) {
        self.gauge_atomic("rts.rooms.active", &self.active_rooms, 1);
    }

    pub fn room_deactivated(&self) {
        self.gauge_atomic("rts.rooms.active", &self.active_rooms, -1);
    }

    pub fn match_started(&self, mode: &str, player_count: usize) {
        self.gauge_atomic("rts.matches.active", &self.active_matches, 1);
        self.count("rts.matches.started", 1.0, &[("mode", mode)]);
        self.gauge(
            "rts.match.players",
            player_count as f64,
            &[("mode", mode), ("state", "started")],
        );
    }

    pub fn match_ended(&self, mode: &str) {
        self.gauge_atomic("rts.matches.active", &self.active_matches, -1);
        self.count("rts.matches.ended", 1.0, &[("mode", mode)]);
    }

    pub fn outbound_dropped(&self, reason: &str) {
        self.count("rts.outbound.dropped", 1.0, &[("reason", reason)]);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_tick(
        &self,
        room: &str,
        mode: &str,
        tick: u32,
        player_count: usize,
        tick_ms: f64,
        snapshot_ms: f64,
        recipients: usize,
        entities_sent: usize,
    ) {
        self.timing_ms("rts.tick.duration_ms", tick_ms, &[("mode", mode)]);
        self.timing_ms(
            "rts.snapshot.fanout_duration_ms",
            snapshot_ms,
            &[("mode", mode)],
        );
        self.gauge(
            "rts.snapshot.recipients",
            recipients as f64,
            &[("mode", mode)],
        );
        self.gauge(
            "rts.snapshot.entities_sent",
            entities_sent as f64,
            &[("mode", mode)],
        );

        if tick_ms >= self.slow_tick_ms {
            self.count("rts.tick.slow", 1.0, &[("mode", mode)]);
            warn!(
                room = %room,
                mode,
                tick,
                player_count,
                tick_ms,
                tick_budget_ms = config::TICK_MS,
                snapshot_ms,
                recipients,
                entities_sent,
                "excessively slow tick"
            );
        }
    }

    pub fn record_client_perf(
        &self,
        room: &str,
        mode: &str,
        player_id: u32,
        report: &ClientPerfReport,
    ) {
        self.gauge("rts.client.fps", f64::from(report.fps), &[("mode", mode)]);
        self.gauge(
            "rts.client.avg_frame_ms",
            f64::from(report.avg_frame_ms),
            &[("mode", mode)],
        );
        self.gauge(
            "rts.client.max_frame_ms",
            f64::from(report.max_frame_ms),
            &[("mode", mode)],
        );
        self.count(
            "rts.client.slow_frames",
            f64::from(report.slow_frames),
            &[("mode", mode)],
        );
        if let Some(ms) = report.snapshot_gap_ms {
            self.gauge(
                "rts.client.snapshot_gap_ms",
                f64::from(ms),
                &[("mode", mode)],
            );
        }
        if let Some(ms) = report.rtt_ms {
            self.gauge("rts.client.rtt_ms", f64::from(ms), &[("mode", mode)]);
        }

        let slow_snapshot = report
            .snapshot_gap_ms
            .map(|ms| ms >= self.slow_client_snapshot_gap_ms)
            .unwrap_or(false);
        let slow_rtt = report
            .rtt_ms
            .map(|ms| ms >= self.slow_client_rtt_ms)
            .unwrap_or(false);
        if report.fps <= self.slow_client_fps
            || report.max_frame_ms >= self.slow_client_frame_ms
            || slow_snapshot
            || slow_rtt
        {
            self.count("rts.client.lag_report", 1.0, &[("mode", mode)]);
            warn!(
                room = %room,
                mode,
                player_id,
                fps = report.fps,
                avg_frame_ms = report.avg_frame_ms,
                max_frame_ms = report.max_frame_ms,
                slow_frames = report.slow_frames,
                snapshot_gap_ms = report.snapshot_gap_ms,
                rtt_ms = report.rtt_ms,
                "client lag report"
            );
        }
    }

    fn gauge_atomic(&self, metric: &str, atomic: &AtomicI64, delta: i64) {
        let value = adjust_nonnegative(atomic, delta);
        self.gauge(metric, value as f64, &[]);
    }

    fn gauge(&self, metric: &str, value: f64, tags: &[(&str, &str)]) {
        self.metric(metric, value, MetricKind::Gauge, tags);
    }

    fn count(&self, metric: &str, value: f64, tags: &[(&str, &str)]) {
        self.metric(metric, value, MetricKind::Count, tags);
    }

    fn timing_ms(&self, metric: &str, value: f64, tags: &[(&str, &str)]) {
        self.metric(metric, value, MetricKind::Timer, tags);
    }

    fn metric(&self, metric: &str, value: f64, kind: MetricKind, tags: &[(&str, &str)]) {
        if !value.is_finite() {
            return;
        }
        let Some(metrics) = &self.metrics else {
            return;
        };
        metrics.submit(MetricPoint {
            metric: metric.to_string(),
            value,
            kind,
            tags: build_tags(&metrics.default_tags, tags),
            timestamp: unix_timestamp_secs(),
        });
    }
}

fn adjust_nonnegative(atomic: &AtomicI64, delta: i64) -> i64 {
    let mut current = atomic.load(Ordering::Relaxed);
    loop {
        let next = current.saturating_add(delta).max(0);
        match atomic.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(actual) => current = actual,
        }
    }
}

struct MetricsSink {
    name: &'static str,
    tx: mpsc::Sender<MetricPoint>,
    default_tags: Vec<String>,
}

impl MetricsSink {
    fn from_env(default_tags: Vec<String>) -> Option<Self> {
        if let Some(addr) = nonempty_env("RTS_DOGSTATSD_ADDR") {
            let tx = spawn_dogstatsd_sink(addr.trim().to_string());
            return Some(Self {
                name: "dogstatsd",
                tx,
                default_tags,
            });
        }

        if env_flag("RTS_DATADOG_METRICS") {
            let Some(api_key) = nonempty_env("DD_API_KEY") else {
                warn!("RTS_DATADOG_METRICS is enabled but DD_API_KEY is not set; metrics disabled");
                return None;
            };
            let site = env::var("DD_SITE").unwrap_or_else(|_| "datadoghq.com".to_string());
            let endpoint = datadog_api_endpoint(&site);
            let tx = spawn_datadog_api_sink(endpoint, api_key);
            return Some(Self {
                name: "datadog_api",
                tx,
                default_tags,
            });
        }

        None
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn submit(&self, point: MetricPoint) {
        if self.tx.try_send(point).is_err() {
            debug!("dropping metric because observability channel is full");
        }
    }
}

#[derive(Clone)]
struct MetricPoint {
    metric: String,
    value: f64,
    kind: MetricKind,
    tags: Vec<String>,
    timestamp: i64,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum MetricKind {
    Gauge,
    Count,
    Timer,
}

impl MetricKind {
    fn dogstatsd_type(self) -> &'static str {
        match self {
            MetricKind::Gauge => "g",
            MetricKind::Count => "c",
            MetricKind::Timer => "ms",
        }
    }

    fn api_type(self) -> &'static str {
        match self {
            MetricKind::Count => "count",
            MetricKind::Gauge | MetricKind::Timer => "gauge",
        }
    }
}

fn spawn_dogstatsd_sink(addr: String) -> mpsc::Sender<MetricPoint> {
    let (tx, mut rx) = mpsc::channel::<MetricPoint>(METRIC_CHANNEL_CAP);
    tokio::spawn(async move {
        let socket = match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
            Ok(socket) => socket,
            Err(err) => {
                warn!(%err, "failed to bind DogStatsD UDP socket; metrics disabled");
                while rx.recv().await.is_some() {}
                return;
            }
        };
        while let Some(point) = rx.recv().await {
            let line = dogstatsd_line(&point);
            if let Err(err) = socket.send_to(line.as_bytes(), &addr).await {
                debug!(%err, addr = %addr, "failed to send DogStatsD metric");
            }
        }
    });
    tx
}

fn spawn_datadog_api_sink(endpoint: String, api_key: String) -> mpsc::Sender<MetricPoint> {
    let (tx, mut rx) = mpsc::channel::<MetricPoint>(METRIC_CHANNEL_CAP);
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let mut ticker = tokio::time::interval(DATADOG_API_FLUSH_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut points = Vec::new();
        let mut last_flush = Instant::now();

        loop {
            tokio::select! {
                maybe_point = rx.recv() => {
                    match maybe_point {
                        Some(point) => {
                            points.push(point);
                            if points.len() >= DATADOG_API_MAX_BATCH_POINTS {
                                flush_datadog_api(&client, &endpoint, &api_key, &mut points, &mut last_flush).await;
                            }
                        }
                        None => {
                            flush_datadog_api(&client, &endpoint, &api_key, &mut points, &mut last_flush).await;
                            return;
                        }
                    }
                }
                _ = ticker.tick() => {
                    flush_datadog_api(&client, &endpoint, &api_key, &mut points, &mut last_flush).await;
                }
            }
        }
    });
    tx
}

async fn flush_datadog_api(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
    points: &mut Vec<MetricPoint>,
    last_flush: &mut Instant,
) {
    if points.is_empty() {
        *last_flush = Instant::now();
        return;
    }
    let interval_secs = last_flush.elapsed().as_secs().max(1) as i64;
    *last_flush = Instant::now();
    let payload = api_payload(std::mem::take(points), interval_secs);
    let series_count = payload.series.len();
    match client
        .post(endpoint)
        .header("DD-API-KEY", api_key)
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            debug!(series_count, "submitted Datadog metric series");
        }
        Ok(resp) => {
            warn!(
                status = %resp.status(),
                series_count,
                "Datadog metric submission failed"
            );
        }
        Err(err) => {
            warn!(%err, series_count, "Datadog metric submission failed");
        }
    }
}

#[derive(Hash, Eq, PartialEq)]
struct SeriesKey {
    metric: String,
    kind: MetricKind,
    tags: Vec<String>,
}

#[derive(Serialize)]
struct ApiPayload {
    series: Vec<ApiSeries>,
}

#[derive(Serialize)]
struct ApiSeries {
    metric: String,
    points: Vec<(i64, f64)>,
    #[serde(rename = "type")]
    metric_type: &'static str,
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interval: Option<i64>,
}

fn api_payload(points: Vec<MetricPoint>, interval_secs: i64) -> ApiPayload {
    let mut grouped: HashMap<SeriesKey, Vec<(i64, f64)>> = HashMap::new();
    for point in points {
        let key = SeriesKey {
            metric: point.metric,
            kind: point.kind,
            tags: point.tags,
        };
        grouped
            .entry(key)
            .or_default()
            .push((point.timestamp, point.value));
    }
    let series = grouped
        .into_iter()
        .map(|(key, points)| ApiSeries {
            metric: key.metric,
            points,
            metric_type: key.kind.api_type(),
            tags: key.tags,
            interval: (key.kind == MetricKind::Count).then_some(interval_secs),
        })
        .collect();
    ApiPayload { series }
}

fn dogstatsd_line(point: &MetricPoint) -> String {
    let mut line = format!(
        "{}:{}|{}",
        point.metric,
        point.value,
        point.kind.dogstatsd_type()
    );
    if !point.tags.is_empty() {
        line.push_str("|#");
        line.push_str(&point.tags.join(","));
    }
    line
}

fn default_tags(env_name: &str) -> Vec<String> {
    let mut tags = vec![
        datadog_tag(
            "service",
            &env::var("DD_SERVICE").unwrap_or_else(|_| SERVICE_NAME.to_string()),
        ),
        datadog_tag("env", env_name),
    ];
    if let Some(app) = nonempty_env("FLY_APP_NAME") {
        tags.push(datadog_tag("app", app.trim()));
    }
    if let Some(region) = nonempty_env("FLY_REGION") {
        tags.push(datadog_tag("region", region.trim()));
    }
    tags
}

fn build_tags(default_tags: &[String], extra: &[(&str, &str)]) -> Vec<String> {
    let mut tags = Vec::with_capacity(default_tags.len() + extra.len());
    tags.extend(default_tags.iter().cloned());
    tags.extend(extra.iter().map(|(key, value)| datadog_tag(key, value)));
    tags
}

fn datadog_tag(key: &str, value: &str) -> String {
    format!("{}:{}", sanitize_tag_key(key), sanitize_tag_value(value))
}

fn sanitize_tag_key(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "unknown".to_string()
    } else {
        cleaned
    }
}

fn sanitize_tag_value(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':') {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "unknown".to_string()
    } else {
        cleaned
    }
}

fn datadog_api_endpoint(site: &str) -> String {
    let site = site
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    format!("https://api.{site}/api/v1/series")
}

fn unix_timestamp_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn env_f64(name: &str) -> Option<f64> {
    env::var(name)
        .ok()?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

fn env_f32(name: &str) -> Option<f32> {
    env::var(name)
        .ok()?
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn nonempty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datadog_api_endpoint_defaults_to_api_subdomain() {
        assert_eq!(
            datadog_api_endpoint("datadoghq.com"),
            "https://api.datadoghq.com/api/v1/series"
        );
        assert_eq!(
            datadog_api_endpoint("https://datadoghq.eu/"),
            "https://api.datadoghq.eu/api/v1/series"
        );
    }

    #[test]
    fn api_payload_groups_points_by_series_identity() {
        let points = vec![
            MetricPoint {
                metric: "rts.tick.duration_ms".to_string(),
                value: 1.0,
                kind: MetricKind::Gauge,
                tags: vec!["env:test".to_string()],
                timestamp: 10,
            },
            MetricPoint {
                metric: "rts.tick.duration_ms".to_string(),
                value: 2.0,
                kind: MetricKind::Gauge,
                tags: vec!["env:test".to_string()],
                timestamp: 11,
            },
        ];
        let payload = api_payload(points, 10);
        assert_eq!(payload.series.len(), 1);
        assert_eq!(payload.series[0].points.len(), 2);
        assert_eq!(payload.series[0].interval, None);
    }

    #[test]
    fn dogstatsd_line_includes_tags_and_type() {
        let point = MetricPoint {
            metric: "rts.tick.slow".to_string(),
            value: 1.0,
            kind: MetricKind::Count,
            tags: vec!["env:test".to_string(), "mode:normal".to_string()],
            timestamp: 10,
        };
        assert_eq!(
            dogstatsd_line(&point),
            "rts.tick.slow:1|c|#env:test,mode:normal"
        );
    }
}
