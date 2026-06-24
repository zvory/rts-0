use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::build_info::build_id;
use crate::lobby::ConnectionReportStats;
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
/// Payload byte budget chosen below the common Ethernet MSS to leave room for WebSocket/TLS/TCP/IP
/// overhead that client payload-byte measurements do not include.
pub const SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES: u32 = 1_280;
pub const NET_REPORT_SNAPSHOT_PACKET_BUDGET_MIN_SAMPLES: u32 = 120;
pub const NET_REPORT_SNAPSHOT_PACKET_BUDGET_OVER_PCT_X100: u16 = 5_000;
pub const NET_REPORT_SNAPSHOT_PAYLOAD_MAX_ISSUE_BYTES: u32 = 256 * 1024;
pub const NET_REPORT_SNAPSHOT_PAYLOAD_AVG_ISSUE_BYTES: u32 = 128 * 1024;
pub const NET_REPORT_SNAPSHOT_PARSE_ISSUE_MS: u16 = 16;
pub const NET_REPORT_SNAPSHOT_PARSE_P95_ISSUE_MS: u16 = 8;
pub const NET_REPORT_SNAPSHOT_DECODE_ISSUE_MS: u16 = 16;
pub const NET_REPORT_SNAPSHOT_DECODE_P95_ISSUE_MS: u16 = 8;
pub const NET_REPORT_SNAPSHOT_APPLY_ISSUE_MS: u16 = 16;
pub const NET_REPORT_SNAPSHOT_APPLY_P95_ISSUE_MS: u16 = 8;
pub const NET_REPORT_SNAPSHOT_TICK_GAP_ISSUE: u32 = 3;
pub const NET_REPORT_SNAPSHOT_BURST_ISSUE: u32 = 3;
pub const NET_REPORT_FRAME_GAP_ISSUE_MS: u16 = 100;
pub const NET_REPORT_FRAME_WORK_ISSUE_MS: u16 = 33;
pub const NET_REPORT_FRAME_WORK_P95_ISSUE_MS: u16 = 24;
pub const NET_REPORT_RENDERER_ISSUE_MS: u16 = 33;
pub const NET_REPORT_RENDERER_P95_ISSUE_MS: u16 = 16;
pub const NET_REPORT_WS_BUFFERED_BYTES_ISSUE: u32 = 64 * 1024;
pub const NET_REPORT_SERVER_TICK_ISSUE_MS: u16 = 33;
pub const NET_REPORT_SERVER_LAG_ISSUE_MS: u16 = 33;
pub const NET_REPORT_PENDING_COMMAND_ISSUE: u16 = 8;
pub const NET_REPORT_COMMAND_UPLOAD_ISSUE_MS: u16 = 180;
pub const NET_REPORT_COMMAND_SERVER_QUEUE_ISSUE_MS: u16 = 66;
pub const NET_REPORT_COMMAND_ACK_APPLY_ISSUE_MS: u16 = 16;
pub const NET_REPORT_COMMAND_BURST_ISSUE: u16 = 6;
pub const NET_REPORT_CORRECTION_ISSUE_PX: u16 = 32;
pub const NET_REPORT_REPLAY_TICK_ISSUE: u16 = 8;
pub const NET_REPORT_REPLAY_MS_ISSUE: u16 = 8;
pub const NET_REPORT_SERVER_RELIABLE_BEFORE_SNAPSHOT_ISSUE: u32 = 1;
pub const NET_REPORT_SERVER_SNAPSHOT_SEND_AGE_ISSUE_MS: u32 = 100;
pub const NET_REPORT_SERVER_SNAPSHOT_REPLACED_ISSUE: u32 = 1;

pub fn new_match_run_id(room: &str) -> String {
    let seq = NEXT_MATCH_RUN_ID.fetch_add(1, Ordering::Relaxed);
    let millis = chrono::Utc::now().timestamp_millis();
    format!("{}-{millis}-{seq:06x}", sanitize_id_segment(room))
}

pub fn log_client_net_report(
    player_id: u32,
    current_room_name: Option<&str>,
    report: ClientNetReport,
    outbound: ConnectionReportStats,
) {
    if !is_notable_net_report(&report, &outbound) {
        return;
    }

    let room = current_room_name.unwrap_or("");
    let primary_issue = classify_client_net_report(&report, &outbound);
    let mut line = String::from("client network report");
    macro_rules! field {
        ($key:literal, $value:expr) => {
            append_log_field(&mut line, $key, $value);
        };
    }
    macro_rules! text_field {
        ($key:literal, $value:expr) => {
            append_text_log_field(&mut line, $key, $value);
        };
    }

    text_field!("event", "client_net_report");
    field!("schema_version", report.schema_version);
    text_field!("build_id", build_id());
    text_field!("room", room);
    text_field!("match_run_id", &report.match_run_id);
    field!("player_id", player_id);
    text_field!("primary_issue", primary_issue);
    field!("elapsed_ms", report.elapsed_ms);
    field!("match_tick", report.match_tick);
    field!("rtt_ms", report.rtt_ms);
    field!("rtt_max_ms", report.rtt_max_ms);
    field!("bad_rtt_samples", report.bad_rtt_samples);
    field!("snapshot_jitter_ms", report.snapshot_jitter_ms);
    field!("snapshot_gap_max_ms", report.snapshot_gap_max_ms);
    field!("jitter_samples", report.jitter_samples);
    field!("snapshots", report.snapshots);
    field!(
        "snapshot_late_frame_count",
        report.snapshot_late_frame_count
    );
    field!(
        "predicted_snapshot_late_frame_count",
        report.predicted_snapshot_late_frame_count
    );
    field!("snapshot_bytes_total", report.snapshot_bytes_total);
    field!("snapshot_bytes_max", report.snapshot_bytes_max);
    field!("snapshot_bytes_avg", report.snapshot_bytes_avg);
    field!("snapshot_message_count", report.snapshot_message_count);
    text_field!("snapshot_byte_source", &report.snapshot_byte_source);
    text_field!("snapshot_codec", &report.snapshot_codec);
    field!("snapshot_codec_version", report.snapshot_codec_version);
    text_field!("snapshot_frame_kind", &report.snapshot_frame_kind);
    field!("snapshot_bytes_p95", report.snapshot_bytes_p95);
    field!(
        "snapshot_segment_budget_bytes",
        report.snapshot_segment_budget_bytes
    );
    field!(
        "snapshot_over_segment_budget_count",
        report.snapshot_over_segment_budget_count
    );
    field!(
        "snapshot_over_segment_budget_pct_x100",
        report.snapshot_over_segment_budget_pct_x100
    );
    field!("snapshot_parse_max_ms", report.snapshot_parse_max_ms);
    field!("snapshot_parse_p95_ms", report.snapshot_parse_p95_ms);
    field!("snapshot_decode_max_ms", report.snapshot_decode_max_ms);
    field!("snapshot_decode_p95_ms", report.snapshot_decode_p95_ms);
    text_field!("websocket_extensions", &report.websocket_extensions);
    text_field!("websocket_compression", &report.websocket_compression);
    field!("snapshot_apply_max_ms", report.snapshot_apply_max_ms);
    field!("snapshot_apply_p95_ms", report.snapshot_apply_p95_ms);
    field!("prediction_apply_max_ms", report.prediction_apply_max_ms);
    field!("prediction_apply_p95_ms", report.prediction_apply_p95_ms);
    field!("snapshot_tick_gap_max", report.snapshot_tick_gap_max);
    field!("stale_snapshot_count", report.stale_snapshot_count);
    field!("duplicate_snapshot_count", report.duplicate_snapshot_count);
    field!("skipped_snapshot_count", report.skipped_snapshot_count);
    field!("snapshot_burst_count", report.snapshot_burst_count);
    field!("snapshot_burst_max", report.snapshot_burst_max);
    field!("frame_gap_max_ms", report.frame_gap_max_ms);
    field!("fps_estimate", report.fps_estimate);
    field!("frame_work_max_ms", report.frame_work_max_ms);
    field!("frame_work_p95_ms", report.frame_work_p95_ms);
    field!("slow_frame_count", report.slow_frame_count);
    text_field!("worst_frame_phase", &report.worst_frame_phase);
    field!("worst_frame_phase_ms", report.worst_frame_phase_ms);
    field!("renderer_max_ms", report.renderer_max_ms);
    field!("renderer_p95_ms", report.renderer_p95_ms);
    field!("entity_count", report.entity_count);
    field!("selected_count", report.selected_count);
    field!("visible_tile_count", report.visible_tile_count);
    field!("viewport_width", report.viewport_width);
    field!("viewport_height", report.viewport_height);
    field!("device_pixel_ratio_x100", report.device_pixel_ratio_x100);
    field!("command_burst_bucket_ms", report.command_burst_bucket_ms);
    field!("command_burst_max", report.command_burst_max);
    field!(
        "command_burst_frame_gap_max_ms",
        report.command_burst_frame_gap_max_ms
    );
    text_field!(
        "command_burst_worst_frame_phase",
        &report.command_burst_worst_frame_phase
    );
    field!(
        "command_burst_worst_frame_phase_ms",
        report.command_burst_worst_frame_phase_ms
    );
    field!("hidden", report.hidden);
    field!("focused", report.focused);
    field!("desktop_runtime_present", report.desktop_runtime_present);
    field!(
        "native_cursor_bridge_present",
        report.native_cursor_bridge_present
    );
    field!("native_cursor_supported", report.native_cursor_supported);
    field!("native_cursor_active", report.native_cursor_active);
    text_field!(
        "native_cursor_last_reason",
        &report.native_cursor_last_reason
    );
    text_field!("native_cursor_last_error", &report.native_cursor_last_error);
    field!("tauri_internals_present", report.tauri_internals_present);
    field!("tauri_global_present", report.tauri_global_present);
    text_field!("tauri_globals", &report.tauri_globals);
    field!("ws_buffered_bytes", report.ws_buffered_bytes);
    field!("server_tick_ms", report.server_tick_ms);
    field!("server_lag_ms", report.server_lag_ms);
    field!("slow_tick_count", report.slow_tick_count);
    field!("head_of_line_count", report.head_of_line_count);
    text_field!("prediction_mode", &report.prediction_mode);
    field!("pending_command_count", report.pending_command_count);
    field!(
        "acknowledged_command_latency_ms",
        report.acknowledged_command_latency_ms
    );
    field!("commands_issued", report.commands_issued);
    field!(
        "command_socket_send_accepted",
        report.command_socket_send_accepted
    );
    field!("command_server_received", report.command_server_received);
    field!("command_sim_acknowledged", report.command_sim_acknowledged);
    field!("command_rejected", report.command_rejected);
    field!(
        "command_issue_to_server_receipt_latest_ms",
        report.command_issue_to_server_receipt_latest_ms
    );
    field!(
        "command_issue_to_server_receipt_max_ms",
        report.command_issue_to_server_receipt_max_ms
    );
    field!(
        "command_issue_to_server_receipt_p95_ms",
        report.command_issue_to_server_receipt_p95_ms
    );
    field!(
        "command_server_receipt_to_sim_ack_latest_ms",
        report.command_server_receipt_to_sim_ack_latest_ms
    );
    field!(
        "command_server_receipt_to_sim_ack_max_ms",
        report.command_server_receipt_to_sim_ack_max_ms
    );
    field!(
        "command_server_receipt_to_sim_ack_p95_ms",
        report.command_server_receipt_to_sim_ack_p95_ms
    );
    field!(
        "command_issue_to_sim_ack_latest_ms",
        report.command_issue_to_sim_ack_latest_ms
    );
    field!(
        "command_issue_to_sim_ack_max_ms",
        report.command_issue_to_sim_ack_max_ms
    );
    field!(
        "command_issue_to_sim_ack_p95_ms",
        report.command_issue_to_sim_ack_p95_ms
    );
    field!(
        "command_ack_snapshot_received_to_applied_latest_ms",
        report.command_ack_snapshot_received_to_applied_latest_ms
    );
    field!(
        "command_ack_snapshot_received_to_applied_max_ms",
        report.command_ack_snapshot_received_to_applied_max_ms
    );
    field!(
        "command_ack_snapshot_received_to_applied_p95_ms",
        report.command_ack_snapshot_received_to_applied_p95_ms
    );
    field!(
        "oldest_pending_command_age_ms",
        report.oldest_pending_command_age_ms
    );
    field!(
        "max_pending_command_count",
        report.max_pending_command_count
    );
    field!("correction_distance_px", report.correction_distance_px);
    field!("correction_count", report.correction_count);
    field!("prediction_disable_count", report.prediction_disable_count);
    field!(
        "prediction_disable_user_count",
        report.prediction_disable_user_count
    );
    field!(
        "prediction_disable_replay_count",
        report.prediction_disable_replay_count
    );
    field!(
        "prediction_disable_spectator_count",
        report.prediction_disable_spectator_count
    );
    field!(
        "prediction_disable_compatibility_count",
        report.prediction_disable_compatibility_count
    );
    field!(
        "prediction_disable_wasm_count",
        report.prediction_disable_wasm_count
    );
    field!(
        "prediction_disable_other_count",
        report.prediction_disable_other_count
    );
    field!("wasm_tick_ms", report.wasm_tick_ms);
    field!("wasm_memory_bytes", report.wasm_memory_bytes);
    field!("prediction_replay_ticks", report.prediction_replay_ticks);
    field!("prediction_replay_max_ms", report.prediction_replay_max_ms);
    field!(
        "prediction_replay_max_ticks",
        report.prediction_replay_max_ticks
    );
    field!(
        "prediction_replay_budget_exceeded_count",
        report.prediction_replay_budget_exceeded_count
    );
    field!(
        "server_command_receipts_accepted",
        outbound.command_receipts_accepted
    );
    field!(
        "server_command_receipts_rejected",
        outbound.command_receipts_rejected
    );
    field!(
        "server_reliable_drained_before_snapshot",
        outbound.reliable_drained_before_snapshot
    );
    field!(
        "server_reliable_drained_before_snapshot_max",
        outbound.reliable_drained_before_snapshot_max
    );
    field!(
        "server_snapshot_waited_behind_reliable",
        outbound.snapshot_waited_behind_reliable
    );
    field!("server_snapshot_sent", outbound.snapshot_sent);
    field!(
        "server_snapshot_send_age_latest_ms",
        outbound.snapshot_send_age_latest_ms
    );
    field!(
        "server_snapshot_send_age_max_ms",
        outbound.snapshot_send_age_max_ms
    );
    field!(
        "server_snapshot_send_age_avg_ms",
        outbound.snapshot_send_age_avg_ms
    );
    field!("server_snapshot_slot_stored", outbound.snapshot_slot_stored);
    field!(
        "server_snapshot_slot_replaced",
        outbound.snapshot_slot_replaced
    );
    field!("server_snapshot_slot_closed", outbound.snapshot_slot_closed);

    tracing::info!("{}", line);
}

fn append_log_field(line: &mut String, key: &str, value: impl std::fmt::Display) {
    let _ = write!(line, " {key}={value}");
}

fn append_text_log_field(line: &mut String, key: &str, value: &str) {
    let escaped = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string());
    append_log_field(line, key, escaped);
}

pub fn is_notable_net_report(report: &ClientNetReport, outbound: &ConnectionReportStats) -> bool {
    report.rtt_ms >= NET_REPORT_LATENCY_ISSUE_MS
        || report.rtt_max_ms >= NET_REPORT_LATENCY_ISSUE_MS
        || report.bad_rtt_samples > 0
        || report.snapshot_jitter_ms >= NET_REPORT_JITTER_ISSUE_MS
        || report.jitter_samples > 0
        || report.snapshot_gap_max_ms >= NET_REPORT_SNAPSHOT_GAP_ISSUE_MS
        || report.snapshot_late_frame_count > 0
        || has_packet_budget_pressure(report)
        || report.snapshot_bytes_max >= NET_REPORT_SNAPSHOT_PAYLOAD_MAX_ISSUE_BYTES
        || report.snapshot_bytes_avg >= NET_REPORT_SNAPSHOT_PAYLOAD_AVG_ISSUE_BYTES
        || report.snapshot_parse_max_ms >= NET_REPORT_SNAPSHOT_PARSE_ISSUE_MS
        || report.snapshot_parse_p95_ms >= NET_REPORT_SNAPSHOT_PARSE_P95_ISSUE_MS
        || report.snapshot_decode_max_ms >= NET_REPORT_SNAPSHOT_DECODE_ISSUE_MS
        || report.snapshot_decode_p95_ms >= NET_REPORT_SNAPSHOT_DECODE_P95_ISSUE_MS
        || report.snapshot_apply_max_ms >= NET_REPORT_SNAPSHOT_APPLY_ISSUE_MS
        || report.snapshot_apply_p95_ms >= NET_REPORT_SNAPSHOT_APPLY_P95_ISSUE_MS
        || report.prediction_apply_max_ms >= NET_REPORT_SNAPSHOT_APPLY_ISSUE_MS
        || report.prediction_apply_p95_ms >= NET_REPORT_SNAPSHOT_APPLY_P95_ISSUE_MS
        || report.snapshot_tick_gap_max >= NET_REPORT_SNAPSHOT_TICK_GAP_ISSUE
        || report.stale_snapshot_count > 0
        || report.duplicate_snapshot_count > 0
        || report.skipped_snapshot_count > 0
        || report.snapshot_burst_max >= NET_REPORT_SNAPSHOT_BURST_ISSUE
        || report.frame_gap_max_ms >= NET_REPORT_FRAME_GAP_ISSUE_MS
        || report.frame_work_max_ms >= NET_REPORT_FRAME_WORK_ISSUE_MS
        || report.frame_work_p95_ms >= NET_REPORT_FRAME_WORK_P95_ISSUE_MS
        || report.slow_frame_count > 0
        || report.renderer_max_ms >= NET_REPORT_RENDERER_ISSUE_MS
        || report.renderer_p95_ms >= NET_REPORT_RENDERER_P95_ISSUE_MS
        || report.ws_buffered_bytes >= NET_REPORT_WS_BUFFERED_BYTES_ISSUE
        || report.server_tick_ms >= NET_REPORT_SERVER_TICK_ISSUE_MS
        || report.server_lag_ms >= NET_REPORT_SERVER_LAG_ISSUE_MS
        || report.pending_command_count >= NET_REPORT_PENDING_COMMAND_ISSUE
        || report.acknowledged_command_latency_ms >= NET_REPORT_LATENCY_ISSUE_MS
        || report.command_rejected > 0
        || report.command_issue_to_server_receipt_max_ms >= NET_REPORT_COMMAND_UPLOAD_ISSUE_MS
        || report.command_server_receipt_to_sim_ack_max_ms
            >= NET_REPORT_COMMAND_SERVER_QUEUE_ISSUE_MS
        || report.command_issue_to_sim_ack_max_ms >= NET_REPORT_COMMAND_UPLOAD_ISSUE_MS
        || report.command_ack_snapshot_received_to_applied_max_ms
            >= NET_REPORT_COMMAND_ACK_APPLY_ISSUE_MS
        || report.oldest_pending_command_age_ms >= NET_REPORT_COMMAND_UPLOAD_ISSUE_MS
        || report.max_pending_command_count >= NET_REPORT_PENDING_COMMAND_ISSUE
        || report.command_burst_max >= NET_REPORT_COMMAND_BURST_ISSUE
        || report.command_burst_frame_gap_max_ms >= NET_REPORT_FRAME_GAP_ISSUE_MS
        || report.correction_distance_px >= NET_REPORT_CORRECTION_ISSUE_PX
        || report.correction_count > 0
        || report.prediction_disable_count > 0
        || report.wasm_tick_ms >= NET_REPORT_FRAME_GAP_ISSUE_MS
        || report.prediction_replay_ticks >= NET_REPORT_REPLAY_TICK_ISSUE
        || report.prediction_replay_max_ticks >= NET_REPORT_REPLAY_TICK_ISSUE
        || report.prediction_replay_max_ms >= NET_REPORT_REPLAY_MS_ISSUE
        || report.prediction_replay_budget_exceeded_count > 0
        || outbound.reliable_drained_before_snapshot
            >= NET_REPORT_SERVER_RELIABLE_BEFORE_SNAPSHOT_ISSUE
        || outbound.snapshot_waited_behind_reliable > 0
        || outbound.snapshot_send_age_max_ms >= NET_REPORT_SERVER_SNAPSHOT_SEND_AGE_ISSUE_MS
        || outbound.snapshot_slot_replaced >= NET_REPORT_SERVER_SNAPSHOT_REPLACED_ISSUE
}

pub fn classify_client_net_report(
    report: &ClientNetReport,
    outbound: &ConnectionReportStats,
) -> &'static str {
    if report.command_rejected > 0 {
        "command_rejected"
    } else if report.command_issue_to_server_receipt_max_ms >= NET_REPORT_COMMAND_UPLOAD_ISSUE_MS {
        "command_upload_delay"
    } else if report.command_server_receipt_to_sim_ack_max_ms
        >= NET_REPORT_COMMAND_SERVER_QUEUE_ISSUE_MS
    {
        "command_server_queue"
    } else if report.command_ack_snapshot_received_to_applied_max_ms
        >= NET_REPORT_COMMAND_ACK_APPLY_ISSUE_MS
    {
        "command_ack_apply"
    } else if report.command_issue_to_sim_ack_max_ms >= NET_REPORT_COMMAND_UPLOAD_ISSUE_MS
        || report.oldest_pending_command_age_ms >= NET_REPORT_COMMAND_UPLOAD_ISSUE_MS
    {
        "command_response_delay"
    } else if outbound.reliable_drained_before_snapshot
        >= NET_REPORT_SERVER_RELIABLE_BEFORE_SNAPSHOT_ISSUE
        || outbound.snapshot_waited_behind_reliable > 0
        || outbound.snapshot_send_age_max_ms >= NET_REPORT_SERVER_SNAPSHOT_SEND_AGE_ISSUE_MS
        || outbound.snapshot_slot_replaced >= NET_REPORT_SERVER_SNAPSHOT_REPLACED_ISSUE
    {
        "server_snapshot_outbound"
    } else if report.command_burst_max >= NET_REPORT_COMMAND_BURST_ISSUE {
        "command_density"
    } else if report.prediction_disable_count > 0 {
        "prediction_disabled"
    } else if report.correction_distance_px >= NET_REPORT_CORRECTION_ISSUE_PX
        || report.correction_count > 0
    {
        "prediction_correction"
    } else if report.wasm_tick_ms >= NET_REPORT_FRAME_GAP_ISSUE_MS
        || report.prediction_replay_ticks >= NET_REPORT_REPLAY_TICK_ISSUE
        || report.prediction_replay_max_ticks >= NET_REPORT_REPLAY_TICK_ISSUE
        || report.prediction_replay_max_ms >= NET_REPORT_REPLAY_MS_ISSUE
        || report.prediction_replay_budget_exceeded_count > 0
    {
        "wasm_budget"
    } else if report.snapshot_bytes_max >= NET_REPORT_SNAPSHOT_PAYLOAD_MAX_ISSUE_BYTES
        || report.snapshot_bytes_avg >= NET_REPORT_SNAPSHOT_PAYLOAD_AVG_ISSUE_BYTES
    {
        "payload_pressure"
    } else if has_packet_budget_pressure(report) {
        "packet_budget_pressure"
    } else if report.snapshot_apply_max_ms >= NET_REPORT_SNAPSHOT_APPLY_ISSUE_MS
        || report.snapshot_apply_p95_ms >= NET_REPORT_SNAPSHOT_APPLY_P95_ISSUE_MS
        || report.prediction_apply_max_ms >= NET_REPORT_SNAPSHOT_APPLY_ISSUE_MS
        || report.prediction_apply_p95_ms >= NET_REPORT_SNAPSHOT_APPLY_P95_ISSUE_MS
    {
        "client_snapshot_apply"
    } else if report.snapshot_decode_max_ms >= NET_REPORT_SNAPSHOT_DECODE_ISSUE_MS
        || report.snapshot_decode_p95_ms >= NET_REPORT_SNAPSHOT_DECODE_P95_ISSUE_MS
    {
        "client_snapshot_decode"
    } else if report.snapshot_parse_max_ms >= NET_REPORT_SNAPSHOT_PARSE_ISSUE_MS
        || report.snapshot_parse_p95_ms >= NET_REPORT_SNAPSHOT_PARSE_P95_ISSUE_MS
    {
        "client_snapshot_parse"
    } else if report.renderer_max_ms >= NET_REPORT_RENDERER_ISSUE_MS
        || report.renderer_p95_ms >= NET_REPORT_RENDERER_P95_ISSUE_MS
    {
        "client_renderer"
    } else if report.frame_work_max_ms >= NET_REPORT_FRAME_WORK_ISSUE_MS
        || report.frame_work_p95_ms >= NET_REPORT_FRAME_WORK_P95_ISSUE_MS
    {
        "client_frame_work"
    } else if report.frame_gap_max_ms >= NET_REPORT_FRAME_GAP_ISSUE_MS
        || report.command_burst_frame_gap_max_ms >= NET_REPORT_FRAME_GAP_ISSUE_MS
        || report.slow_frame_count > 0
    {
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
    } else if report.snapshot_gap_max_ms >= NET_REPORT_SNAPSHOT_GAP_ISSUE_MS
        || report.snapshot_late_frame_count > 0
    {
        "snapshot_gap"
    } else if report.snapshot_tick_gap_max >= NET_REPORT_SNAPSHOT_TICK_GAP_ISSUE
        || report.stale_snapshot_count > 0
        || report.duplicate_snapshot_count > 0
        || report.skipped_snapshot_count > 0
        || report.snapshot_burst_max >= NET_REPORT_SNAPSHOT_BURST_ISSUE
    {
        "snapshot_cadence"
    } else if report.snapshot_jitter_ms >= NET_REPORT_JITTER_ISSUE_MS || report.jitter_samples > 0 {
        "snapshot_jitter"
    } else {
        "other"
    }
}

fn has_packet_budget_pressure(report: &ClientNetReport) -> bool {
    let budget = if report.snapshot_segment_budget_bytes > 0 {
        report.snapshot_segment_budget_bytes
    } else {
        SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES
    };
    report.snapshot_message_count >= NET_REPORT_SNAPSHOT_PACKET_BUDGET_MIN_SAMPLES
        && report.snapshot_bytes_p95 > budget
        && report.snapshot_over_segment_budget_pct_x100
            >= NET_REPORT_SNAPSHOT_PACKET_BUDGET_OVER_PCT_X100
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
    pub winner_team_id: Option<u32>,
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
        ?ctx.winner_team_id,
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
            match_run_id: "main-1".to_string(),
            elapsed_ms: 10_000,
            match_tick: 300,
            rtt_ms: 40,
            rtt_max_ms: 70,
            bad_rtt_samples: 0,
            snapshot_jitter_ms: 3,
            snapshot_gap_max_ms: 45,
            jitter_samples: 0,
            snapshots: 300,
            snapshot_late_frame_count: 0,
            predicted_snapshot_late_frame_count: 0,
            snapshot_bytes_total: 1_200_000,
            snapshot_bytes_max: 5_000,
            snapshot_bytes_avg: 4_000,
            snapshot_message_count: 300,
            snapshot_byte_source: "messagepack-application-payload".to_string(),
            snapshot_codec: "messagepack-compact".to_string(),
            snapshot_codec_version: 1,
            snapshot_frame_kind: "binary".to_string(),
            snapshot_bytes_p95: SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES,
            snapshot_segment_budget_bytes: SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES,
            snapshot_over_segment_budget_count: 0,
            snapshot_over_segment_budget_pct_x100: 0,
            snapshot_parse_max_ms: 1,
            snapshot_parse_p95_ms: 1,
            snapshot_decode_max_ms: 2,
            snapshot_decode_p95_ms: 1,
            websocket_extensions: String::new(),
            websocket_compression: "none".to_string(),
            snapshot_apply_max_ms: 4,
            snapshot_apply_p95_ms: 2,
            prediction_apply_max_ms: 3,
            prediction_apply_p95_ms: 2,
            snapshot_tick_gap_max: 1,
            stale_snapshot_count: 0,
            duplicate_snapshot_count: 0,
            skipped_snapshot_count: 0,
            snapshot_burst_count: 0,
            snapshot_burst_max: 1,
            frame_gap_max_ms: 18,
            fps_estimate: 60,
            frame_work_max_ms: 10,
            frame_work_p95_ms: 8,
            slow_frame_count: 0,
            worst_frame_phase: String::new(),
            worst_frame_phase_ms: 0,
            renderer_max_ms: 6,
            renderer_p95_ms: 4,
            entity_count: 120,
            selected_count: 0,
            visible_tile_count: 500,
            viewport_width: 1280,
            viewport_height: 720,
            device_pixel_ratio_x100: 100,
            command_burst_bucket_ms: 250,
            command_burst_max: 0,
            command_burst_frame_gap_max_ms: 0,
            command_burst_worst_frame_phase: String::new(),
            command_burst_worst_frame_phase_ms: 0,
            hidden: false,
            focused: true,
            desktop_runtime_present: false,
            native_cursor_bridge_present: false,
            native_cursor_supported: false,
            native_cursor_active: false,
            native_cursor_last_reason: String::new(),
            native_cursor_last_error: String::new(),
            tauri_internals_present: false,
            tauri_global_present: false,
            tauri_globals: String::new(),
            ws_buffered_bytes: 0,
            server_tick_ms: 4,
            server_lag_ms: 0,
            slow_tick_count: 0,
            head_of_line_count: 0,
            prediction_mode: String::new(),
            pending_command_count: 0,
            acknowledged_command_latency_ms: 0,
            commands_issued: 0,
            command_socket_send_accepted: 0,
            command_server_received: 0,
            command_sim_acknowledged: 0,
            command_rejected: 0,
            command_issue_to_server_receipt_latest_ms: 0,
            command_issue_to_server_receipt_max_ms: 0,
            command_issue_to_server_receipt_p95_ms: 0,
            command_server_receipt_to_sim_ack_latest_ms: 0,
            command_server_receipt_to_sim_ack_max_ms: 0,
            command_server_receipt_to_sim_ack_p95_ms: 0,
            command_issue_to_sim_ack_latest_ms: 0,
            command_issue_to_sim_ack_max_ms: 0,
            command_issue_to_sim_ack_p95_ms: 0,
            command_ack_snapshot_received_to_applied_latest_ms: 0,
            command_ack_snapshot_received_to_applied_max_ms: 0,
            command_ack_snapshot_received_to_applied_p95_ms: 0,
            oldest_pending_command_age_ms: 0,
            max_pending_command_count: 0,
            correction_distance_px: 0,
            correction_count: 0,
            prediction_disable_count: 0,
            prediction_disable_user_count: 0,
            prediction_disable_replay_count: 0,
            prediction_disable_spectator_count: 0,
            prediction_disable_compatibility_count: 0,
            prediction_disable_wasm_count: 0,
            prediction_disable_other_count: 0,
            wasm_tick_ms: 0,
            wasm_memory_bytes: 0,
            prediction_replay_ticks: 0,
            prediction_replay_max_ms: 0,
            prediction_replay_max_ticks: 0,
            prediction_replay_budget_exceeded_count: 0,
        }
    }

    fn notable(report: &ClientNetReport) -> bool {
        is_notable_net_report(report, &ConnectionReportStats::default())
    }

    fn classify(report: &ClientNetReport) -> &'static str {
        classify_client_net_report(report, &ConnectionReportStats::default())
    }

    #[test]
    fn clean_net_reports_are_not_notable() {
        let report = clean_report();
        assert!(!notable(&report));
        assert_eq!(classify(&report), "other");
    }

    #[test]
    fn net_report_classification_prioritizes_actionable_issue() {
        let mut report = clean_report();
        report.snapshot_jitter_ms = NET_REPORT_JITTER_ISSUE_MS;
        assert_eq!(classify(&report), "snapshot_jitter");
        report.frame_work_max_ms = NET_REPORT_FRAME_WORK_ISSUE_MS;
        assert_eq!(classify(&report), "client_frame_work");
        report.snapshot_decode_max_ms = NET_REPORT_SNAPSHOT_DECODE_ISSUE_MS;
        assert_eq!(classify(&report), "client_snapshot_decode");
        report.snapshot_apply_max_ms = NET_REPORT_SNAPSHOT_APPLY_ISSUE_MS;
        assert_eq!(classify(&report), "client_snapshot_apply");
        report.snapshot_bytes_max = NET_REPORT_SNAPSHOT_PAYLOAD_MAX_ISSUE_BYTES;
        assert_eq!(classify(&report), "payload_pressure");
        report.renderer_max_ms = NET_REPORT_RENDERER_ISSUE_MS;
        assert_eq!(classify(&report), "payload_pressure");
        report.correction_count = 1;
        assert_eq!(classify(&report), "prediction_correction");
        report.prediction_disable_count = 1;
        assert_eq!(classify(&report), "prediction_disabled");
    }

    #[test]
    fn net_report_classifies_packet_budget_pressure_separately_from_large_payloads() {
        let mut report = clean_report();
        report.snapshot_bytes_p95 = SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES + 1;
        report.snapshot_over_segment_budget_count = 200;
        report.snapshot_over_segment_budget_pct_x100 =
            NET_REPORT_SNAPSHOT_PACKET_BUDGET_OVER_PCT_X100;
        assert!(notable(&report));
        assert_eq!(classify(&report), "packet_budget_pressure");

        report.snapshot_bytes_max = NET_REPORT_SNAPSHOT_PAYLOAD_MAX_ISSUE_BYTES;
        assert_eq!(classify(&report), "payload_pressure");
    }

    #[test]
    fn net_report_classifies_frame_gap_separately_from_work() {
        let mut report = clean_report();
        report.frame_gap_max_ms = NET_REPORT_FRAME_GAP_ISSUE_MS;
        assert!(notable(&report));
        assert_eq!(classify(&report), "client_frame_stall");
    }

    #[test]
    fn net_report_classifies_slow_frame_count_as_frame_stall_without_work_cost() {
        let mut report = clean_report();
        report.slow_frame_count = 1;
        assert!(notable(&report));
        assert_eq!(classify(&report), "client_frame_stall");
    }

    #[test]
    fn net_report_classifies_snapshot_cadence_when_transport_timing_is_clean() {
        let mut report = clean_report();
        report.snapshot_tick_gap_max = NET_REPORT_SNAPSHOT_TICK_GAP_ISSUE;
        assert!(notable(&report));
        assert_eq!(classify(&report), "snapshot_cadence");
    }

    #[test]
    fn net_report_classifies_command_milestones_before_generic_network() {
        let mut report = clean_report();
        report.rtt_max_ms = NET_REPORT_LATENCY_ISSUE_MS;
        report.command_issue_to_server_receipt_max_ms = NET_REPORT_COMMAND_UPLOAD_ISSUE_MS;
        assert!(notable(&report));
        assert_eq!(classify(&report), "command_upload_delay");

        let mut report = clean_report();
        report.command_server_receipt_to_sim_ack_max_ms = NET_REPORT_COMMAND_SERVER_QUEUE_ISSUE_MS;
        assert_eq!(classify(&report), "command_server_queue");

        let mut report = clean_report();
        report.command_ack_snapshot_received_to_applied_max_ms =
            NET_REPORT_COMMAND_ACK_APPLY_ISSUE_MS;
        assert_eq!(classify(&report), "command_ack_apply");
    }

    #[test]
    fn net_report_classifies_command_density_without_other_lag() {
        let mut report = clean_report();
        report.command_burst_max = NET_REPORT_COMMAND_BURST_ISSUE;
        assert!(notable(&report));
        assert_eq!(classify(&report), "command_density");
    }

    #[test]
    fn net_report_classifies_server_outbound_snapshot_pressure() {
        let report = clean_report();
        let outbound = ConnectionReportStats {
            reliable_drained_before_snapshot: 2,
            reliable_drained_before_snapshot_max: 2,
            snapshot_waited_behind_reliable: 1,
            snapshot_sent: 1,
            snapshot_send_age_latest_ms: 120,
            snapshot_send_age_max_ms: 120,
            snapshot_send_age_avg_ms: 120,
            snapshot_slot_stored: 1,
            snapshot_slot_replaced: 1,
            ..ConnectionReportStats::default()
        };
        assert!(is_notable_net_report(&report, &outbound));
        assert_eq!(
            classify_client_net_report(&report, &outbound),
            "server_snapshot_outbound"
        );
    }

    #[test]
    fn match_run_ids_are_distinct_and_room_scoped() {
        let a = new_match_run_id("Main Room");
        let b = new_match_run_id("Main Room");
        assert_ne!(a, b);
        assert!(a.starts_with("main-room-"));
    }
}
