use std::fmt::Write as _;

use crate::protocol::ClientNetReport;

const STALL_ISSUE_MS: u32 = 2_000;

pub(super) fn is_notable(report: &ClientNetReport) -> bool {
    report.render_worker_failure_count > 0
        || report.render_worker_context_lost_count > 0
        || (report.render_worker_in_flight
            && !report.hidden
            && report.render_worker_in_flight_age_ms >= STALL_ISSUE_MS)
}

pub(super) fn classification(report: &ClientNetReport) -> Option<&'static str> {
    if report.render_worker_context_lost_count > 0
        || (report.render_worker_failure_count > 0
            && report.render_worker_error_code == "webglContextLost")
    {
        Some("client_render_worker_context_lost")
    } else if report.render_worker_failure_count > 0 {
        Some("client_render_worker_failure")
    } else if report.render_worker_in_flight
        && !report.hidden
        && report.render_worker_in_flight_age_ms >= STALL_ISSUE_MS
    {
        Some("client_render_worker_stall")
    } else {
        None
    }
}

pub(super) fn append_fields(line: &mut String, report: &ClientNetReport) {
    macro_rules! field {
        ($key:literal, $value:expr) => {
            let _ = write!(line, " {}={}", $key, $value);
        };
    }
    macro_rules! text {
        ($key:literal, $value:expr) => {
            field!(
                $key,
                serde_json::to_string($value).unwrap_or_else(|_| "\"\"".to_string())
            );
        };
    }

    field!("slow_frame_count", report.slow_frame_count);
    field!(
        "frame_work_budget_miss_count",
        report.frame_work_budget_miss_count
    );
    field!(
        "present_budget_miss_count",
        report.present_budget_miss_count
    );
    text!("worst_frame_phase", &report.worst_frame_phase);
    field!("worst_frame_phase_ms", report.worst_frame_phase_ms);
    field!("renderer_max_ms", report.renderer_max_ms);
    field!("renderer_p95_ms", report.renderer_p95_ms);
    field!("renderer_update_max_ms", report.renderer_update_max_ms);
    field!("renderer_update_p95_ms", report.renderer_update_p95_ms);
    field!("renderer_present_max_ms", report.renderer_present_max_ms);
    field!("renderer_present_p95_ms", report.renderer_present_p95_ms);
    text!("top_renderer_phase", &report.top_renderer_phase);
    field!("top_renderer_phase_ms", report.top_renderer_phase_ms);
    text!(
        "top_render_diagnostic_group",
        &report.top_render_diagnostic_group
    );
    field!(
        "top_render_diagnostic_group_count",
        report.top_render_diagnostic_group_count
    );
    text!(
        "client_frame_phases",
        &super::format_client_frame_phases(&report.client_frame_phases)
    );
    text!(
        "renderer_frame_phases",
        &super::format_client_frame_phases(&report.renderer_frame_phases)
    );
    text!(
        "render_diagnostic_counters",
        &super::format_client_render_counters(&report.render_diagnostic_counters)
    );
    text!("render_worker_mode", &report.render_worker_mode);
    field!("render_worker_submitted", report.render_worker_submitted);
    field!("render_worker_presented", report.render_worker_presented);
    field!(
        "render_worker_failure_count",
        report.render_worker_failure_count
    );
    field!(
        "render_worker_context_lost_count",
        report.render_worker_context_lost_count
    );
    field!("render_worker_in_flight", report.render_worker_in_flight);
    field!(
        "render_worker_in_flight_frame_id",
        report.render_worker_in_flight_frame_id
    );
    field!(
        "render_worker_in_flight_age_ms",
        report.render_worker_in_flight_age_ms
    );
    field!("render_worker_pending", report.render_worker_pending);
    field!(
        "render_worker_pending_frame_id",
        report.render_worker_pending_frame_id
    );
    field!(
        "render_worker_last_presented_frame_id",
        report.render_worker_last_presented_frame_id
    );
    field!(
        "render_worker_last_presented_age_ms",
        report.render_worker_last_presented_age_ms
    );
    field!(
        "render_worker_last_message_age_ms",
        report.render_worker_last_message_age_ms
    );
    text!("render_worker_error_code", &report.render_worker_error_code);
    text!(
        "render_worker_error_message",
        &report.render_worker_error_message
    );
    text!(
        "render_worker_error_stack",
        &report.render_worker_error_stack
    );
    text!(
        "render_worker_error_source",
        &report.render_worker_error_source
    );
    field!("render_worker_error_line", report.render_worker_error_line);
    field!(
        "render_worker_error_column",
        report.render_worker_error_column
    );
    text!("render_worker_backend", &report.render_worker_backend);
    text!(
        "render_worker_pixi_version",
        &report.render_worker_pixi_version
    );
    text!("render_worker_gl_vendor", &report.render_worker_gl_vendor);
    text!(
        "render_worker_gl_renderer",
        &report.render_worker_gl_renderer
    );
    text!("render_worker_gl_version", &report.render_worker_gl_version);
    text!("render_worker_user_agent", &report.render_worker_user_agent);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prioritizes_failures_and_ignores_hidden_stalls() {
        let mut report = ClientNetReport {
            render_worker_failure_count: 1,
            render_worker_context_lost_count: 1,
            ..ClientNetReport::default()
        };
        assert!(is_notable(&report));
        assert_eq!(
            classification(&report),
            Some("client_render_worker_context_lost")
        );

        report.render_worker_context_lost_count = 0;
        report.render_worker_error_code = "workerUncaughtError".to_string();
        assert_eq!(
            classification(&report),
            Some("client_render_worker_failure")
        );

        report.render_worker_failure_count = 0;
        report.render_worker_in_flight = true;
        report.render_worker_in_flight_age_ms = STALL_ISSUE_MS;
        assert_eq!(classification(&report), Some("client_render_worker_stall"));
        report.hidden = true;
        assert!(!is_notable(&report));
    }
}
