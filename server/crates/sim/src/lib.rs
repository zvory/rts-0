//! Authoritative RTS simulation crate.
//!
//! This crate owns `Game`, simulation systems, map loading, fog, deterministic replay, AI, and
//! self-play helpers. It deliberately has no server shell, WebSocket transport, Tokio, Axum, or
//! tracing-subscriber dependency.

pub mod game;
pub mod perf;
pub mod protocol;

pub(crate) mod config;
pub(crate) mod rules;
