use crate::build_info::build_id;
use crate::protocol::ClientNetReport;
use super::{append_log_field, append_text_log_field};

pub(super) fn log(player_id: u32, current_room_name: Option<&str>, report: &ClientNetReport) {
    if let Some(summary) = &report.performance_summary {
        let mut line = String::from("client performance summary");
        append_text_log_field(&mut line, "event", "client_performance_summary");
        append_text_log_field(&mut line, "build_id", build_id());
        append_text_log_field(&mut line, "client_build_id", &report.client_build_id.chars().take(64).collect::<String>());
        append_text_log_field(&mut line, "room", current_room_name.unwrap_or(""));
        append_text_log_field(&mut line, "match_run_id", &report.match_run_id);
        append_log_field(&mut line, "player_id", player_id);
        append_log_field(&mut line, "match_tick", report.match_tick);
        append_log_field(&mut line, "entity_count", report.entity_count);
        append_log_field(&mut line, "viewport_width", report.viewport_width);
        append_log_field(&mut line, "viewport_height", report.viewport_height);
        append_log_field(&mut line, "device_pixel_ratio_x100", report.device_pixel_ratio_x100);
        append_text_log_field(&mut line, "gl_renderer", &report.render_worker_gl_renderer.chars().take(200).collect::<String>());
        append_text_log_field(&mut line, "user_agent", &report.render_worker_user_agent.chars().take(300).collect::<String>());
        // Fixed numeric/boolean DTO: one bounded JSON object, even for a healthy report.
        if let Ok(json) = serde_json::to_string(summary) {
            append_text_log_field(&mut line, "summary", &json);
        }
        crate::log_info!("{}", line);
    }
}
