//! The player wiki served at `/wiki/`, generated from the running build's own code.
//!
//! * Numbers come from the compiled `rts-rules` (and the simulation's research table) — see
//!   [`snapshot`]. Where each number is set and used comes from a code index built by `build.rs`.
//! * Hand-written notes, unit icons and the version history are data under `docs/wiki/`.
//!   They never hold numbers: notes reference constants with `{{NAME}}` templates.
//!
//! [`build_site`] returns every page as self-contained HTML keyed by file name.
#![recursion_limit = "512"]

mod client_text;
mod compat;
pub mod render;
pub mod snapshot;

use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;

pub const REPO_URL: &str = "https://github.com/zvory/rts-0";

/// What the generator reads besides the compiled rules.
pub struct Inputs<'a> {
    /// client/src/config/rules_mirror.js (names, tooltips)
    pub client_rules_js: &'a str,
    /// docs/wiki/content
    pub content_dir: &'a Path,
    /// docs/wiki/icons.json
    pub icons: Option<&'a str>,
    /// docs/wiki/history.json
    pub history: Option<&'a str>,
    /// the server's build id (`/version`), e.g. "e6038c9d8e4e" or "e6038c9d8e4e [v1]"
    pub build_id: &'a str,
}

/// Every page of the wiki: file name ("index.html", "unit-tank.html", ...) → HTML.
pub fn build_site(inputs: &Inputs) -> BTreeMap<String, String> {
    let commit = inputs
        .build_id
        .split_whitespace()
        .next()
        .unwrap_or("unknown")
        .to_string();
    let short: String = commit.chars().take(9).collect();
    // the commit date is known only when this build is the newest recorded version
    let date = inputs
        .history
        .and_then(|h| serde_json::from_str::<serde_json::Value>(h).ok())
        .and_then(|h| {
            h.get("versions")
                .and_then(|v| v.as_array())
                .and_then(|v| v.last())
                .cloned()
        })
        .filter(|v| {
            v["game"]["commit"]
                .as_str()
                .is_some_and(|c| !short.is_empty() && c.starts_with(&short))
        })
        .and_then(|v| v["game"]["date"].as_str().map(str::to_string))
        .unwrap_or_default();
    let game = json!({ "commit": commit, "short": short, "date": date, "repo": REPO_URL });
    let cur = snapshot::build(inputs.client_rules_js, game);
    let content = render::content_load(inputs.content_dir);
    render::render(cur, content, inputs.icons, inputs.history)
}
