use super::{AiActions, AiFrame};

/// Object-safe lifecycle implemented by a custom Rust strategy.
///
/// [`AiStrategy::initialize`] is called exactly once, immediately before the first `step`. Both
/// lifecycle calls receive only a player-scoped [`AiFrame`], and `step` is invoked only on the
/// canonical nine-tick, player-staggered decision cadence.
pub trait AiStrategy: Send {
    fn initialize(&mut self, _frame: &AiFrame) {}

    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions);
}
