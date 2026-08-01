/// Trim and bound a player-supplied display name so it stays sane in lobby UIs and logs.
pub(crate) fn sanitize_name(name: String) -> String {
    const MAX_NAME_LEN: usize = 24;
    let mut cleaned = String::with_capacity(name.len().min(MAX_NAME_LEN));
    let mut char_count = 0;
    let mut pending_space = false;

    for ch in name.chars() {
        if ch.is_whitespace() {
            pending_space = !cleaned.is_empty();
            continue;
        }
        if pending_space {
            // Keep the bounded result trimmed when the limit lands between words.
            if char_count + 1 >= MAX_NAME_LEN {
                break;
            }
            cleaned.push(' ');
            char_count += 1;
            pending_space = false;
        }
        cleaned.push(ch);
        char_count += 1;
        if char_count == MAX_NAME_LEN {
            break;
        }
    }
    if cleaned.is_empty() {
        "Commander".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_commander_for_blank_names() {
        assert_eq!(sanitize_name(" \n\t ".to_string()), "Commander");
    }

    #[test]
    fn is_single_line_and_scalar_bounded() {
        assert_eq!(
            sanitize_name("  First\n\tCommander  ".to_string()),
            "First Commander"
        );
        assert_eq!(sanitize_name("x".repeat(30)).chars().count(), 24);
        assert_eq!(
            sanitize_name(format!("{} second", "x".repeat(23))),
            "x".repeat(23)
        );
    }
}
