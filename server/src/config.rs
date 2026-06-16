//! Server-shell compatibility shim for Rust-owned balance constants and stat helpers.
//!
//! `rts-rules` owns these values. `client/src/config.js` mirrors the UI/render/fog subset
//! (costs, supply, sight, sizes, colors), so keep that mirror in sync with player-facing balance
//! edits. Server-only shell constants should stay in the server module that uses them.

pub use rts_rules::balance::*;
