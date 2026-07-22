//! AI controllers, strategy profiles, and self-play harnesses.
//!
//! This crate depends on the public simulation API and emits ordinary [`SimCommand`]s. The
//! simulation crate does not depend on this crate.

pub mod ai_core;
pub mod selfplay;
pub mod tools;

mod ai_shared;
mod config {
    pub(crate) use rts_rules::balance::*;
}
mod live;

pub use live::{
    canonical_live_profile_id, is_player_live_profile_id, live_profile_label,
    random_live_profile_id, resolve_live_profile_id_for_match, AiController,
    AiDecisionTraceSnapshot, AiThinkContext, DEFAULT_LIVE_PROFILE_ID, LIVE_PROFILE_IDS,
};

#[cfg(test)]
const FULL_AI_TESTS_ENV: &str = "RTS_FULL_AI_TESTS";
#[cfg(test)]
const SELFPLAY_FULL_ENV: &str = "RTS_SELFPLAY_FULL";

#[cfg(test)]
fn env_flag_enabled(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
pub(crate) fn full_ai_tests_enabled() -> bool {
    env_flag_enabled(FULL_AI_TESTS_ENV) || env_flag_enabled(SELFPLAY_FULL_ENV)
}

#[cfg(test)]
pub(crate) fn skip_unless_full_ai(test_name: &str) -> bool {
    if full_ai_tests_enabled() {
        false
    } else {
        eprintln!("skipping {test_name}; set {FULL_AI_TESTS_ENV}=1 to run full AI coverage");
        true
    }
}
