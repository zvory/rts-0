//! Compatibility surface for extracted rules plus sim-owned projection.

pub mod projection;
mod projection_abilities;

#[allow(unused_imports)]
pub use rts_rules::{combat, defs, economy, faction, is_rifle_infantry, target, terrain};
