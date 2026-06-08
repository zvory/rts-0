//! Shared server crate surface used by the server shell and developer tools.
//!
//! Phase 0 keeps the existing source layout intact while giving binaries one
//! module tree to link against. Later crate-split phases can move these modules
//! into narrower packages behind this compatibility surface.

pub(crate) mod config;
pub mod dev_scenarios;
pub mod lobby;
pub mod perf;
pub mod protocol;

pub use rts_sim::game;
