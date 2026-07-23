use serde::{Deserialize, Serialize};
use std::fmt;

const MAX_COMMAND_LIFECYCLE_EXEMPLARS: usize = 5;
const MAX_CLIENT_FRAME_PHASES: usize = 5;
const MAX_CLIENT_RENDER_COUNTERS: usize = 5;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CommandLifecycleExemplar {
    pub client_seq: u32,
    pub family: String,
    pub issued_elapsed_ms: u32,
    pub stage: String,
    pub stage_ms: u16,
}

fn deserialize_command_lifecycle_exemplars<'de, D>(
    deserializer: D,
) -> Result<Vec<CommandLifecycleExemplar>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct BoundedExemplarsVisitor;

    impl<'de> serde::de::Visitor<'de> for BoundedExemplarsVisitor {
        type Value = Vec<CommandLifecycleExemplar>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an optional command lifecycle exemplar array")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Vec::new())
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Vec::new())
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut exemplars = Vec::with_capacity(MAX_COMMAND_LIFECYCLE_EXEMPLARS);
            while exemplars.len() < MAX_COMMAND_LIFECYCLE_EXEMPLARS {
                match seq.next_element::<CommandLifecycleExemplar>()? {
                    Some(entry) => exemplars.push(entry),
                    None => return Ok(exemplars),
                }
            }
            while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
            Ok(exemplars)
        }
    }

    deserializer.deserialize_any(BoundedExemplarsVisitor)
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientFramePhaseReport {
    pub label: String,
    pub count: u32,
    pub max_ms: u16,
    pub p95_ms: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientRenderCounterReport {
    pub label: String,
    pub samples: u32,
    pub frames: u32,
    pub total: u32,
    pub max_frame: u32,
}

fn deserialize_client_frame_phases<'de, D>(
    deserializer: D,
) -> Result<Vec<ClientFramePhaseReport>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct BoundedFramePhasesVisitor;

    impl<'de> serde::de::Visitor<'de> for BoundedFramePhasesVisitor {
        type Value = Vec<ClientFramePhaseReport>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an optional client frame phase array")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Vec::new())
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Vec::new())
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut phases = Vec::with_capacity(MAX_CLIENT_FRAME_PHASES);
            while phases.len() < MAX_CLIENT_FRAME_PHASES {
                match seq.next_element::<ClientFramePhaseReport>()? {
                    Some(entry) => phases.push(entry),
                    None => return Ok(phases),
                }
            }
            while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
            Ok(phases)
        }
    }

    deserializer.deserialize_any(BoundedFramePhasesVisitor)
}

fn deserialize_client_render_counters<'de, D>(
    deserializer: D,
) -> Result<Vec<ClientRenderCounterReport>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct BoundedRenderCountersVisitor;

    impl<'de> serde::de::Visitor<'de> for BoundedRenderCountersVisitor {
        type Value = Vec<ClientRenderCounterReport>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an optional client render diagnostic counter array")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Vec::new())
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Vec::new())
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut counters = Vec::with_capacity(MAX_CLIENT_RENDER_COUNTERS);
            while counters.len() < MAX_CLIENT_RENDER_COUNTERS {
                match seq.next_element::<ClientRenderCounterReport>()? {
                    Some(entry) => counters.push(entry),
                    None => return Ok(counters),
                }
            }
            while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
            Ok(counters)
        }
    }

    deserializer.deserialize_any(BoundedRenderCountersVisitor)
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientNetReport {
    pub schema_version: u8,
    #[serde(default)]
    pub match_run_id: String,
    pub elapsed_ms: u32,
    pub match_tick: u32,
    pub rtt_ms: u16,
    pub rtt_max_ms: u16,
    pub bad_rtt_samples: u32,
    pub snapshot_jitter_ms: u16,
    pub snapshot_gap_max_ms: u16,
    pub jitter_samples: u32,
    pub snapshots: u32,
    #[serde(default)]
    pub snapshot_late_frame_count: u32,
    #[serde(default)]
    pub predicted_snapshot_late_frame_count: u32,
    #[serde(default)]
    pub predicted_snapshot_late_frame_pct_x100: u16,
    #[serde(default)]
    pub prediction_active_late_frame_count: u32,
    #[serde(default)]
    pub snapshot_bytes_total: u32,
    #[serde(default)]
    pub snapshot_bytes_max: u32,
    #[serde(default)]
    pub snapshot_bytes_avg: u32,
    #[serde(default)]
    pub snapshot_message_count: u32,
    #[serde(default)]
    pub snapshot_byte_source: String,
    #[serde(default)]
    pub snapshot_codec: String,
    #[serde(default)]
    pub snapshot_codec_version: u16,
    #[serde(default)]
    pub snapshot_frame_kind: String,
    #[serde(default)]
    pub snapshot_bytes_p95: u32,
    #[serde(default)]
    pub snapshot_segment_budget_bytes: u32,
    #[serde(default)]
    pub snapshot_over_segment_budget_count: u32,
    #[serde(default)]
    pub snapshot_over_segment_budget_pct_x100: u16,
    #[serde(default)]
    pub snapshot_parse_max_ms: u16,
    #[serde(default)]
    pub snapshot_parse_p95_ms: u16,
    #[serde(default)]
    pub snapshot_decode_max_ms: u16,
    #[serde(default)]
    pub snapshot_decode_p95_ms: u16,
    #[serde(default)]
    pub websocket_extensions: String,
    #[serde(default)]
    pub websocket_compression: String,
    #[serde(default)]
    pub snapshot_apply_max_ms: u16,
    #[serde(default)]
    pub snapshot_apply_p95_ms: u16,
    #[serde(default)]
    pub prediction_apply_max_ms: u16,
    #[serde(default)]
    pub prediction_apply_p95_ms: u16,
    #[serde(default)]
    pub snapshot_tick_gap_max: u32,
    #[serde(default)]
    pub stale_snapshot_count: u32,
    #[serde(default)]
    pub duplicate_snapshot_count: u32,
    #[serde(default)]
    pub skipped_snapshot_count: u32,
    #[serde(default)]
    pub snapshot_burst_count: u32,
    #[serde(default)]
    pub snapshot_burst_max: u32,
    pub frame_gap_max_ms: u16,
    pub fps_estimate: u16,
    #[serde(default)]
    pub frame_work_max_ms: u16,
    #[serde(default)]
    pub frame_work_p95_ms: u16,
    #[serde(default)]
    pub frame_raf_dispatch_max_ms: u16,
    #[serde(default)]
    pub frame_raf_dispatch_p95_ms: u16,
    #[serde(default)]
    pub frame_unattributed_max_ms: u16,
    #[serde(default)]
    pub frame_unattributed_p95_ms: u16,
    #[serde(default)]
    pub slow_frame_count: u32,
    #[serde(default)]
    pub frame_work_budget_miss_count: u32,
    #[serde(default)]
    pub present_budget_miss_count: u32,
    #[serde(default)]
    pub worst_frame_phase: String,
    #[serde(default)]
    pub worst_frame_phase_ms: u16,
    #[serde(default)]
    pub renderer_max_ms: u16,
    #[serde(default)]
    pub renderer_p95_ms: u16,
    #[serde(default)]
    pub renderer_update_max_ms: u16,
    #[serde(default)]
    pub renderer_update_p95_ms: u16,
    #[serde(default)]
    pub renderer_present_max_ms: u16,
    #[serde(default)]
    pub renderer_present_p95_ms: u16,
    #[serde(default)]
    pub top_renderer_phase: String,
    #[serde(default)]
    pub top_renderer_phase_ms: u16,
    #[serde(default)]
    pub top_render_diagnostic_group: String,
    #[serde(default)]
    pub top_render_diagnostic_group_count: u32,
    #[serde(default, deserialize_with = "deserialize_client_frame_phases")]
    pub client_frame_phases: Vec<ClientFramePhaseReport>,
    #[serde(default, deserialize_with = "deserialize_client_frame_phases")]
    pub renderer_frame_phases: Vec<ClientFramePhaseReport>,
    #[serde(default, deserialize_with = "deserialize_client_render_counters")]
    pub render_diagnostic_counters: Vec<ClientRenderCounterReport>,
    #[serde(default)]
    pub render_worker_mode: String,
    #[serde(default)]
    pub render_worker_submitted: u32,
    #[serde(default)]
    pub render_worker_presented: u32,
    #[serde(default)]
    pub render_worker_failure_count: u32,
    #[serde(default)]
    pub render_worker_context_lost_count: u32,
    #[serde(default)]
    pub render_worker_in_flight: bool,
    #[serde(default)]
    pub render_worker_in_flight_frame_id: u32,
    #[serde(default)]
    pub render_worker_in_flight_age_ms: u32,
    #[serde(default)]
    pub render_worker_pending: bool,
    #[serde(default)]
    pub render_worker_pending_frame_id: u32,
    #[serde(default)]
    pub render_worker_last_presented_frame_id: u32,
    #[serde(default)]
    pub render_worker_last_presented_age_ms: u32,
    #[serde(default)]
    pub render_worker_last_message_age_ms: u32,
    #[serde(default)]
    pub render_worker_error_code: String,
    #[serde(default)]
    pub render_worker_error_message: String,
    #[serde(default)]
    pub render_worker_error_stack: String,
    #[serde(default)]
    pub render_worker_error_source: String,
    #[serde(default)]
    pub render_worker_error_line: u32,
    #[serde(default)]
    pub render_worker_error_column: u32,
    #[serde(default)]
    pub render_worker_backend: String,
    #[serde(default)]
    pub render_worker_pixi_version: String,
    #[serde(default)]
    pub render_worker_gl_vendor: String,
    #[serde(default)]
    pub render_worker_gl_renderer: String,
    #[serde(default)]
    pub render_worker_gl_version: String,
    #[serde(default)]
    pub render_worker_user_agent: String,
    #[serde(default)]
    pub entity_count: u32,
    #[serde(default)]
    pub selected_count: u16,
    #[serde(default)]
    pub visible_tile_count: u32,
    #[serde(default)]
    pub viewport_width: u16,
    #[serde(default)]
    pub viewport_height: u16,
    #[serde(default)]
    pub device_pixel_ratio_x100: u16,
    #[serde(default)]
    pub command_burst_bucket_ms: u16,
    #[serde(default)]
    pub command_burst_max: u16,
    #[serde(default)]
    pub command_burst_frame_gap_max_ms: u16,
    #[serde(default)]
    pub command_burst_worst_frame_phase: String,
    #[serde(default)]
    pub command_burst_worst_frame_phase_ms: u16,
    pub hidden: bool,
    pub focused: bool,
    #[serde(default)]
    pub desktop_runtime_present: bool,
    #[serde(default)]
    pub native_cursor_bridge_present: bool,
    #[serde(default)]
    pub native_cursor_supported: bool,
    #[serde(default)]
    pub native_cursor_active: bool,
    #[serde(default)]
    pub native_cursor_last_reason: String,
    #[serde(default)]
    pub native_cursor_last_error: String,
    #[serde(default)]
    pub tauri_internals_present: bool,
    #[serde(default)]
    pub tauri_global_present: bool,
    #[serde(default)]
    pub tauri_globals: String,
    pub ws_buffered_bytes: u32,
    pub server_tick_ms: u16,
    pub server_lag_ms: u16,
    pub slow_tick_count: u32,
    pub head_of_line_count: u32,
    #[serde(default)]
    pub prediction_mode: String,
    #[serde(default)]
    pub pending_command_count: u16,
    #[serde(default)]
    pub acknowledged_command_latency_ms: u16,
    #[serde(default)]
    pub commands_issued: u32,
    #[serde(default)]
    pub command_socket_send_accepted: u32,
    #[serde(default)]
    pub command_server_received: u32,
    #[serde(default)]
    pub command_sim_acknowledged: u32,
    #[serde(default)]
    pub command_rejected: u32,
    #[serde(default)]
    pub command_issue_to_socket_send_accepted_latest_ms: u16,
    #[serde(default)]
    pub command_issue_to_socket_send_accepted_max_ms: u16,
    #[serde(default)]
    pub command_issue_to_socket_send_accepted_p95_ms: u16,
    #[serde(default)]
    pub command_issue_to_server_receipt_latest_ms: u16,
    #[serde(default)]
    pub command_issue_to_server_receipt_max_ms: u16,
    #[serde(default)]
    pub command_issue_to_server_receipt_p95_ms: u16,
    #[serde(default)]
    pub command_server_receipt_to_sim_ack_latest_ms: u16,
    #[serde(default)]
    pub command_server_receipt_to_sim_ack_max_ms: u16,
    #[serde(default)]
    pub command_server_receipt_to_sim_ack_p95_ms: u16,
    #[serde(default)]
    pub command_issue_to_sim_ack_latest_ms: u16,
    #[serde(default)]
    pub command_issue_to_sim_ack_max_ms: u16,
    #[serde(default)]
    pub command_issue_to_sim_ack_p95_ms: u16,
    #[serde(default)]
    pub command_ack_snapshot_received_to_applied_latest_ms: u16,
    #[serde(default)]
    pub command_ack_snapshot_received_to_applied_max_ms: u16,
    #[serde(default)]
    pub command_ack_snapshot_received_to_applied_p95_ms: u16,
    #[serde(default)]
    pub oldest_pending_command_age_ms: u16,
    #[serde(default)]
    pub max_pending_command_count: u16,
    #[serde(default)]
    pub command_family_move: u32,
    #[serde(default)]
    pub command_family_attack_move: u32,
    #[serde(default)]
    pub command_family_build: u32,
    #[serde(default)]
    pub command_family_train: u32,
    #[serde(default)]
    pub command_family_other: u32,
    #[serde(default, deserialize_with = "deserialize_command_lifecycle_exemplars")]
    pub command_lifecycle_exemplars: Vec<CommandLifecycleExemplar>,
    #[serde(default)]
    pub correction_distance_px: u16,
    #[serde(default)]
    pub correction_count: u32,
    #[serde(default)]
    pub prediction_disable_count: u32,
    #[serde(default)]
    pub prediction_disable_user_count: u32,
    #[serde(default)]
    pub prediction_disable_replay_count: u32,
    #[serde(default)]
    pub prediction_disable_spectator_count: u32,
    #[serde(default)]
    pub prediction_disable_compatibility_count: u32,
    #[serde(default)]
    pub prediction_disable_wasm_count: u32,
    #[serde(default)]
    pub prediction_disable_other_count: u32,
    #[serde(default)]
    pub wasm_tick_ms: u16,
    #[serde(default)]
    pub wasm_memory_bytes: u32,
    #[serde(default)]
    pub prediction_replay_ticks: u16,
    #[serde(default)]
    pub prediction_replay_max_ms: u16,
    #[serde(default)]
    pub prediction_replay_max_ticks: u16,
    #[serde(default)]
    pub prediction_replay_budget_exceeded_count: u32,
}
