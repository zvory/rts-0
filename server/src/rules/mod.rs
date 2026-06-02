//! Classification, formula, and projection rules. See `DESIGN.md §3.x`.
//!
//! Functions here never mutate state. Services in `game/services/` orchestrate; rules classify,
//! calculate, or project views.

pub mod combat;
pub mod defs;
pub mod economy;
pub mod projection;
pub mod terrain;
