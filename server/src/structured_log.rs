use std::sync::atomic::{AtomicU64, Ordering};

use crate::build_info::build_id;
use crate::protocol::ClientNetReport;

static NEXT_MATCH_RUN_ID: AtomicU64 = AtomicU64::new(1);

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        tracing::debug!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        tracing::info!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        tracing::warn!($($arg)*)
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        tracing::error!($($arg)*)
    };
}

pub const NET_REPORT_LATENCY_ISSUE_MS: u16 = 180;
pub const NET_REPORT_JITTER_ISSUE_MS: u16 = 20;
pub const NET_REPORT_SNAPSHOT_GAP_ISSUE_MS: u16 = 100;
pub const NET_REPORT_FRAME_GAP_ISSUE_MS: u16 = 100;
pub const NET_REPORT_WS_BUFFERED_BYTES_ISSUE: u32 = 64 * 1024;
pub const NET_REPORT_SERVER_TICK_ISSUE_MS: u16 = 33;
pub const NET_REPORT_SERVER_LAG_ISSUE_MS: u16 = 33;
pub const NET_REPORT_PENDING_COMMAND_ISSUE: u16 = 8;
pub const NET_REPORT_CORRECTION_ISSUE_PX: u16 = 32;
pub const NET_REPORT_REPLAY_TICK_ISSUE: u16 = 8;

pub fn new_match_run_id(room: &str) -> String {
    let seq = NEXT_MATCH_RUN_ID.fetch_add(1, Ordering::Relaxed);
    let millis = chrono::Utc::now().timestamp_millis();
    format!("{}-{millis}-{seq:06x}", sanitize_id_segment(room))
}

pub fn log_client_net_report(
    player_id: u32,
    current_room_name: Option<&str>,
    report: ClientNetReport,
) {
    if !is_notable_net_report(&report) {
        return;
    }

    let room = current_room_name.unwrap_or("");
    let primary_issue = classify_client_net_report(&report);
    tracing::info!(
        event = "client_net_report",
        schema_version = report.schema_version,
        build_id = %build_id(),
        room = %room,
        player_id,
        primary_issue,
        elapsed_ms = report.elapsed_ms,
        match_tick = report.match_tick,
        rtt_ms = report.rtt_ms,
        rtt_max_ms = report.rtt_max_ms,
        bad_rtt_samples = report.bad_rtt_samples,
        snapshot_jitter_ms = report.snapshot_jitter_ms,
        snapshot_gap_max_ms = report.snapshot_gap_max_ms,
        jitter_samples = report.jitter_samples,
        snapshots = report.snapshots,
        frame_gap_max_ms = report.frame_gap_max_ms,
        fps_estimate = report.fps_estimate,
        hidden = report.hidden,
        focused = report.focused,
        ws_buffered_bytes = report.ws_buffered_bytes,
        server_tick_ms = report.server_tick_ms,
        server_lag_ms = report.server_lag_ms,
        slow_tick_count = report.slow_tick_count,
        head_of_line_count = report.head_of_line_count,
        prediction_mode = %report.prediction_mode,
        pending_command_count = report.pending_command_count,
        acknowledged_command_latency_ms = report.acknowledged_command_latency_ms,
        correction_distance_px = report.correction_distance_px,
        correction_count = report.correction_count,
        prediction_disable_count = report.prediction_disable_count,
        wasm_tick_ms = report.wasm_tick_ms,
        wasm_memory_bytes = report.wasm_memory_bytes,
        prediction_replay_ticks = report.prediction_replay_ticks,
        "client network report"
    );
}

pub fn is_notable_net_report(report: &ClientNetReport) -> bool {
    report.rtt_ms >= NET_REPORT_LATENCY_ISSUE_MS
        || report.rtt_max_ms >= NET_REPORT_LATENCY_ISSUE_MS
        || report.bad_rtt_samples > 0
        || report.snapshot_jitter_ms >= NET_REPORT_JITTER_ISSUE_MS
        || report.jitter_samples > 0
        || report.snapshot_gap_max_ms >= NET_REPORT_SNAPSHOT_GAP_ISSUE_MS
        || report.frame_gap_max_ms >= NET_REPORT_FRAME_GAP_ISSUE_MS
        || report.ws_buffered_bytes >= NET_REPORT_WS_BUFFERED_BYTES_ISSUE
        || report.server_tick_ms >= NET_REPORT_SERVER_TICK_ISSUE_MS
        || report.server_lag_ms >= NET_REPORT_SERVER_LAG_ISSUE_MS
        || report.pending_command_count >= NET_REPORT_PENDING_COMMAND_ISSUE
        || report.acknowledged_command_latency_ms >= NET_REPORT_LATENCY_ISSUE_MS
        || report.correction_distance_px >= NET_REPORT_CORRECTION_ISSUE_PX
        || report.correction_count > 0
        || report.prediction_disable_count > 0
        || report.wasm_tick_ms >= NET_REPORT_FRAME_GAP_ISSUE_MS
        || report.prediction_replay_ticks >= NET_REPORT_REPLAY_TICK_ISSUE
}

pub fn classify_client_net_report(report: &ClientNetReport) -> &'static str {
    if report.prediction_disable_count > 0 {
        "prediction_disabled"
    } else if report.correction_distance_px >= NET_REPORT_CORRECTION_ISSUE_PX
        || report.correction_count > 0
    {
        "prediction_correction"
    } else if report.wasm_tick_ms >= NET_REPORT_FRAME_GAP_ISSUE_MS
        || report.prediction_replay_ticks >= NET_REPORT_REPLAY_TICK_ISSUE
    {
        "wasm_budget"
    } else if report.frame_gap_max_ms >= NET_REPORT_FRAME_GAP_ISSUE_MS {
        "client_frame_stall"
    } else if report.server_tick_ms >= NET_REPORT_SERVER_TICK_ISSUE_MS {
        "server_tick"
    } else if report.server_lag_ms >= NET_REPORT_SERVER_LAG_ISSUE_MS {
        "server_scheduler_lag"
    } else if report.ws_buffered_bytes >= NET_REPORT_WS_BUFFERED_BYTES_ISSUE {
        "websocket_backlog"
    } else if report.pending_command_count >= NET_REPORT_PENDING_COMMAND_ISSUE {
        "pending_commands"
    } else if report.rtt_ms >= NET_REPORT_LATENCY_ISSUE_MS
        || report.rtt_max_ms >= NET_REPORT_LATENCY_ISSUE_MS
        || report.bad_rtt_samples > 0
        || report.acknowledged_command_latency_ms >= NET_REPORT_LATENCY_ISSUE_MS
    {
        "network_rtt"
    } else if report.snapshot_gap_max_ms >= NET_REPORT_SNAPSHOT_GAP_ISSUE_MS {
        "snapshot_gap"
    } else if report.snapshot_jitter_ms >= NET_REPORT_JITTER_ISSUE_MS || report.jitter_samples > 0 {
        "snapshot_jitter"
    } else {
        "other"
    }
}

pub struct MatchStartedLog<'a> {
    pub room: &'a str,
    pub match_run_id: &'a str,
    pub mode: &'a str,
    pub map: &'a str,
    pub seed: u32,
    pub players: usize,
    pub humans: usize,
    pub ai: usize,
    pub quickstart: bool,
    pub participants: &'a [String],
}

pub fn log_match_started(ctx: MatchStartedLog<'_>) {
    tracing::info!(
        event = "match_started",
        build_id = %build_id(),
        room = %ctx.room,
        match_run_id = %ctx.match_run_id,
        mode = ctx.mode,
        map = %ctx.map,
        seed = ctx.seed,
        players = ctx.players,
        humans = ctx.humans,
        ai = ctx.ai,
        quickstart = ctx.quickstart,
        participants = ?ctx.participants,
        "match started"
    );
}

pub struct MatchEndedLog<'a> {
    pub room: &'a str,
    pub match_run_id: Option<&'a str>,
    pub map: &'a str,
    pub participants: &'a [String],
    pub winner_id: Option<u32>,
    pub duration_ms: Option<i64>,
    pub duration_ticks: Option<u32>,
    pub slow_tick_count: u32,
    pub max_head_of_line_count: u32,
    pub score_count: usize,
    pub replay_captured: bool,
    pub will_record_history: bool,
}

pub fn log_match_ended(ctx: MatchEndedLog<'_>) {
    tracing::info!(
        event = "match_ended",
        build_id = %build_id(),
        room = %ctx.room,
        match_run_id = ctx.match_run_id.unwrap_or(""),
        map = %ctx.map,
        participants = ?ctx.participants,
        ?ctx.winner_id,
        duration_ms = ctx.duration_ms.unwrap_or(0),
        duration_ticks = ctx.duration_ticks.unwrap_or(0),
        slow_tick_count = ctx.slow_tick_count,
        max_head_of_line_count = ctx.max_head_of_line_count,
        score_count = ctx.score_count,
        replay_captured = ctx.replay_captured,
        will_record_history = ctx.will_record_history,
        "match ended"
    );
}

fn sanitize_id_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len().min(24));
    for ch in value.chars().take(24) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    if out.is_empty() {
        "room".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_report() -> ClientNetReport {
        ClientNetReport {
            schema_version: 1,
            elapsed_ms: 10_000,
            match_tick: 300,
            rtt_ms: 40,
            rtt_max_ms: 70,
            bad_rtt_samples: 0,
            snapshot_jitter_ms: 3,
            snapshot_gap_max_ms: 45,
            jitter_samples: 0,
            snapshots: 300,
            frame_gap_max_ms: 18,
            fps_estimate: 60,
            hidden: false,
            focused: true,
            ws_buffered_bytes: 0,
            server_tick_ms: 4,
            server_lag_ms: 0,
            slow_tick_count: 0,
            head_of_line_count: 0,
            prediction_mode: String::new(),
            pending_command_count: 0,
            acknowledged_command_latency_ms: 0,
            correction_distance_px: 0,
            correction_count: 0,
            prediction_disable_count: 0,
            wasm_tick_ms: 0,
            wasm_memory_bytes: 0,
            prediction_replay_ticks: 0,
        }
    }

    #[test]
    fn clean_net_reports_are_not_notable() {
        let report = clean_report();
        assert!(!is_notable_net_report(&report));
        assert_eq!(classify_client_net_report(&report), "other");
    }

    #[test]
    fn net_report_classification_prioritizes_actionable_issue() {
        let mut report = clean_report();
        report.snapshot_jitter_ms = NET_REPORT_JITTER_ISSUE_MS;
        assert_eq!(classify_client_net_report(&report), "snapshot_jitter");
        report.frame_gap_max_ms = NET_REPORT_FRAME_GAP_ISSUE_MS;
        assert_eq!(classify_client_net_report(&report), "client_frame_stall");
        report.correction_count = 1;
        assert_eq!(classify_client_net_report(&report), "prediction_correction");
        report.prediction_disable_count = 1;
        assert_eq!(classify_client_net_report(&report), "prediction_disabled");
    }

    #[test]
    fn match_run_ids_are_distinct_and_room_scoped() {
        let a = new_match_run_id("Main Room");
        let b = new_match_run_id("Main Room");
        assert_ne!(a, b);
        assert!(a.starts_with("main-room-"));
    }
}
