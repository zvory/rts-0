//! Simulation compatibility shim for Rust-owned balance constants and stat helpers.
//!
//! `rts-rules` owns these values. `client/src/config.js` mirrors the UI/render/fog subset
//! (costs, supply, sight, sizes, colors), so keep that mirror in sync with player-facing balance
//! edits. Sim-only implementation constants should live beside the system that uses them instead
//! of being exported from this shim.

pub use rts_rules::balance::*;
