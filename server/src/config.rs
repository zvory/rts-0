//! Compatibility shim for balance constants and stat helpers.
//!
//! `rts-rules` owns these values. `client/src/config.js` mirrors the UI/render/fog subset
//! (costs, supply, sight, sizes, colors), so keep that mirror in sync with player-facing balance
//! edits.

pub use rts_rules::balance::*;
