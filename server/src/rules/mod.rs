//! Pure classification and formula rules. See `DESIGN.md §3.x`.
//!
//! Functions here take `EntityKind` and context primitives, never mutate state, and never read fog.
//! Services in `game/services/` orchestrate; these rules classify.

pub mod combat;
pub mod economy;
