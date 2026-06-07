//! Compatibility re-export for the extracted protocol crate.
//!
//! Phase 1 keeps existing `rts_server::protocol` call sites stable while the wire protocol and
//! semantic DTOs live in narrower crates.

pub use rts_protocol::*;
