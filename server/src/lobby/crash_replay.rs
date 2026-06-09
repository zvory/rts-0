use super::*;

/// Persist a replayable artifact when a room's tick panics (a true crash or, in debug
/// builds, an `assert_invariants` failure). The path is logged and the full file contents
/// are emitted to the log so an operator can copy them out of terminal output even if the
/// disk write later disappears or the box is ephemeral.
pub(super) fn dump_crash_replay(room: &str, game: &Game, reason: &str) {
    let artifact =
        ReplayArtifactV1::capture_from_game(game, env!("COMMIT_HASH"), None, game.scores());
    let json = match serde_json::to_string_pretty(&artifact) {
        Ok(s) => s,
        Err(e) => {
            error!(room = %room, reason = %reason, error = %e, "tick panic: failed to serialize crash replay");
            return;
        }
    };
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let sanitized: String = room
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let dir_name = format!("crash-{sanitized}-{}-{now_ms}", std::process::id());
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("selfplay-failures")
        .join(&dir_name);
    let path = dir.join("replay.json");
    match fs::create_dir_all(&dir).and_then(|_| fs::write(&path, &json)) {
        Ok(_) => {
            error!(
                room = %room,
                tick = game.tick_count(),
                reason = %reason,
                path = %path.display(),
                "tick panic: crash replay written"
            );
        }
        Err(e) => {
            error!(
                room = %room,
                tick = game.tick_count(),
                reason = %reason,
                error = %e,
                "tick panic: failed to write crash replay; dumping inline only"
            );
        }
    }
    error!(
        room = %room,
        reason = %reason,
        "tick panic: full crash replay follows (artifact name: {dir_name})\n----BEGIN CRASH REPLAY----\n{json}\n----END CRASH REPLAY----"
    );
}

pub(super) fn panic_reason(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "panic with non-string payload".to_string()
}
