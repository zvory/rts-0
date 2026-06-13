//! Shared server crate surface used by the server shell and developer tools.
//!
//! The lower-level contracts, protocol, rules, simulation, and AI live in
//! dedicated workspace crates. This crate is the server shell and developer-tool
//! surface around those packages.

pub mod build_info;
pub(crate) mod config;
pub mod db;
pub mod dev_scenarios;
pub mod lobby;
pub mod protocol;
pub mod structured_log;
pub mod tools;

pub use rts_ai as ai;
pub use rts_sim::game;
pub use rts_sim::perf;
