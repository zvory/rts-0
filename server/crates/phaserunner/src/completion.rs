pub fn phase_marked_done(text: &str) -> bool {
    single_line_status_done(text) || heading_status_done(text) || phase_status_checklist_done(text)
}

fn single_line_status_done(text: &str) -> bool {
    text.lines().any(|line| {
        line.trim().eq_ignore_ascii_case("Status: Done.")
            || line.trim().eq_ignore_ascii_case("Status: Done")
    })
}

fn heading_status_done(text: &str) -> bool {
    let mut after_heading = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("## Status") {
            after_heading = true;
            continue;
        }
        if after_heading && trimmed.is_empty() {
            continue;
        }
        if after_heading {
            return trimmed.eq_ignore_ascii_case("Done.") || trimmed.eq_ignore_ascii_case("Done");
        }
    }
    false
}

fn phase_status_checklist_done(text: &str) -> bool {
    let mut after_heading = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("## Phase Status") {
            after_heading = true;
            continue;
        }
        if after_heading && trimmed.is_empty() {
            continue;
        }
        if after_heading {
            return trimmed.eq_ignore_ascii_case("- [x] Done.")
                || trimmed.eq_ignore_ascii_case("- [x] Done");
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_current_completion_marker_forms() {
        assert!(phase_marked_done("# Phase\n\nStatus: Done.\n"));
        assert!(phase_marked_done("# Phase\n\n## Status\n\nDone.\n"));
        assert!(phase_marked_done(
            "# Phase\n\n## Phase Status\n\n- [x] Done.\n"
        ));
    }

    #[test]
    fn rejects_draft_or_unchecked_markers() {
        assert!(!phase_marked_done("## Status\nDraft.\n"));
        assert!(!phase_marked_done("## Phase Status\n- [ ] Done.\n"));
        assert!(!phase_marked_done("Status: Blocked.\n"));
    }
}
