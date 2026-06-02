//! Shared AI world model and derived facts.
//!
//! The live AI and self-play harness observe the game through different surfaces. This module is
//! the narrow compatibility layer that turns both surfaces into stable, deterministic summaries
//! before later phases synthesize commands.

pub(crate) mod facts;
pub(crate) mod observation;
