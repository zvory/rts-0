//! Shared AI world model, profiles, and decisions.
//!
//! The live AI and self-play harness observe the game through different surfaces, then translate
//! those observations into the same deterministic facts, profile policies, action helpers, and
//! command decisions.

pub(crate) mod actions;
pub(crate) mod decision;
pub(crate) mod facts;
pub(crate) mod map_analysis;
pub(crate) mod observation;
pub(crate) mod profile_manifest;
pub(crate) mod profiles;
pub(crate) mod resource_availability;
