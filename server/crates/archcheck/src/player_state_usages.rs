use super::{
    code_before_comment, normalized_code, ArchitectureReport, PlayerStateUsage, SourceFile,
};

pub(super) fn collect(file: &SourceFile, text: &str, report: &mut ArchitectureReport) {
    let mut in_use_statement = false;

    for (index, line) in text.lines().enumerate() {
        let code = code_before_comment(line);
        let trimmed = code.trim_start();
        if in_use_statement || trimmed.starts_with("use ") || trimmed.starts_with("pub use ") {
            in_use_statement = !trimmed.ends_with(';');
            continue;
        }

        if code
            .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
            .any(|part| part == "PlayerState")
        {
            report.metrics.player_state_usages.push(PlayerStateUsage {
                path: file.rel_path.clone(),
                line: index + 1,
                code: normalized_code(code),
            });
        }
    }
}
