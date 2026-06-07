//! Compatibility surface for extracted rules plus sim-owned projection.
//!
//! Pure domain, balance, terrain, economy, and combat rules live in `rts-rules`. Projection still
//! lives here because it reads sim entities, fog, smoke, and constructs protocol DTOs.

pub mod projection;

#[allow(unused_imports)]
pub use rts_rules::{combat, defs, economy, terrain};
