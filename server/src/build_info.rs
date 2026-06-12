use std::process::Command;
use std::sync::OnceLock;

/// Runtime build identifier used for `/version`, asset cache-busting, and replay metadata.
///
/// Deployment passes `COMMIT_HASH` into the runtime container. Local runs fall back to git metadata
/// at process startup so the commit identity does not become part of the Rust compile output.
pub fn build_id() -> &'static str {
    static BUILD_ID: OnceLock<String> = OnceLock::new();
    BUILD_ID.get_or_init(resolve_build_id).as_str()
}

fn resolve_build_id() -> String {
    if let Some(id) = env_build_id() {
        return id;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let hash = git_output(manifest_dir, &["rev-parse", "--short=12", "HEAD"])
        .unwrap_or_else(|| "unknown".to_string());
    if hash == "unknown" {
        return hash;
    }

    match git_output(manifest_dir, &["tag", "--points-at", "HEAD"])
        .and_then(|text| text.lines().next().map(str::trim).map(str::to_string))
        .filter(|tag| !tag.is_empty())
    {
        Some(tag) => format!("{hash} [{tag}]"),
        None => hash,
    }
}

fn env_build_id() -> Option<String> {
    ["COMMIT_HASH", "RTS_BUILD_SHA", "RTS_BUILD_ID"]
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn git_output(current_dir: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(current_dir)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}
