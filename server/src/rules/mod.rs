//! Classification, formula, and projection rules. See `docs/design/server-sim.md`.
//!
//! Functions here never mutate state. Services in `game/services/` orchestrate; rules classify,
//! calculate, or project views.

pub mod combat;
pub mod defs;
pub mod economy;
pub mod projection;
pub mod terrain;
