# Phase 0 - Measurements, Fixtures, and Acceptance Criteria

## Goal

Create a small test and replay baseline before changing movement. The current behavior is hard to
reason about by feel alone, so this phase defines what "better tank movement" means in code and in
playtest terms.

## Steps

1. Add deterministic movement fixtures for one tank on open ground, around a building corner,
   through a two-tile corridor, and through a small traffic cluster.
2. Capture current tank travel time, path length, final position error, facing changes per second,
   stuck ticks, repath count, collision displacement, and oil burned.
3. Add a dev self-play or scripted replay artifact that right-clicks tanks through representative
   battlefield situations.
4. Write acceptance criteria for tank feel:
   - no self-propelled sideways movement;
   - no full-speed movement while the hull is badly misaligned;
   - stable hull facing while the turret fires independently;
   - predictable reverse or pivot behavior near close goals;
   - no large invisible bubble beyond the visible hull once body geometry is refactored.
5. Identify tests that will intentionally change in later phases so failures are expected and
   explainable.

## Plain-Language Explanation

Before changing the model, make the problem visible. We need a few repeatable tank situations and
simple numbers that tell us whether tanks are sliding, over-turning, getting stuck, or being pushed
around too much.

## Expected Code Touches

- `server/src/game/services/movement.rs` tests
- `server/src/game/mod.rs` tests if command-level tank behavior is involved
- `server/src/game/selfplay.rs` only if a replay fixture is useful
- `docs/tanks/` for notes gathered during baseline capture

## Risks

- Overfitting to one fixture can make the controller worse in live matches. Keep fixtures small and
  varied.
- Travel time may rise once tanks obey realistic turning. Treat that as a balance change, not
  automatically as a bug.

## Done When

- Baseline tests/replays exist. See `server/src/game/services/movement.rs`
  `tank_phase0_baseline_*` tests and the replay command in `docs/tanks/BASELINE.md`.
- Current tank behavior has measurable reference values in `docs/tanks/BASELINE.md`.
- Later phases have concrete pass/fail criteria instead of relying only on subjective feel.
