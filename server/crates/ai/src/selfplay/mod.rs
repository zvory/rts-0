//! Test-only API-driven self-play harness.
//!
//! This deliberately drives the public [`Game`] seam (`enqueue`, `tick`, `snapshot_for`) instead
//! of reaching into simulation internals. The scripted players behave like deterministic API
//! clients: observe a fog-filtered snapshot, issue ordinary commands, and let the authoritative
//! simulation validate every action.
#![allow(dead_code)]

mod live;
mod milestones;
pub(crate) mod pending_build;
pub(crate) mod player_view;
mod replay;
mod scripts;
mod validation;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use live::LiveSelfPlay;
#[allow(unused_imports)]
pub use replay::{
    assert_replay_matches_live, is_safe_artifact_name, server_build_sha, SelfPlayFailure,
};
#[allow(unused_imports)]
pub use replay::{
    available_profile_ids, canonical_profile_id, run_profile_matchup_result,
    ProfileMatchupEndReason, ProfileMatchupOptions, ProfileMatchupPlayerResult,
    ProfileMatchupResult, ProfileMatchupStartingCityCentreResult, ProfileMatchupTraceEntry,
    ProfileMatchupWinner,
};

const MAX_TICKS: u32 = 9_600;
const MAX_STALL_TICKS: u32 = 1_800;
const SAMPLE_EVERY_TICKS: u32 = 30;
const THINK_INTERVAL: u32 = 6;
const ATTACK_REISSUE_TICKS: u32 = 120;
pub(crate) const SELFPLAY_ATTACK_STAGE_SUPPRESSION_TICKS: u32 = 3_600;
const SELFPLAY_FAILURE_DIR: &str = "selfplay-failures";
const SELFPLAY_ARTIFACT_DIR: &str = "selfplay-artifacts";
const SAVE_REPLAY_ENV: &str = "RTS_SELFPLAY_SAVE_REPLAY";
