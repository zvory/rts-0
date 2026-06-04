# Phase 6 - Integration, Balance, and Documentation Audit

## Goal

Lock the new tank movement into the project contracts, tests, and balance notes after the movement
and geometry work has stabilized.

## Steps

1. Run the full server test suite for movement, pathing, combat, projection, self-play, and replay
   determinism.
2. Run live integration tests with a server:
   - `node tests/server_integration.mjs`
   - `node tests/regression.mjs`
   - `node tests/ai_integration.mjs`
   - `node tests/client_smoke.mjs`
3. Run or inspect self-play artifacts that include tank tech and tank combat.
4. Update `DESIGN.md` in the same implementation change for:
   - tank locomotion state;
   - body geometry;
   - standability;
   - collision;
   - path-following behavior;
   - balance constants.
5. Update mirrored client config if tank body/render values changed.
6. Collect patch-note bullets:
   - tank acceleration/braking;
   - turn/pivot/reverse behavior;
   - physical footprint changes;
   - oil burn changes, if any;
   - expected strategic impact.
7. Revisit tank balance after playtest. Slower turning and body-aware collision may require speed,
   range, cost, or build-time tuning.

## Plain-Language Explanation

This phase makes the work shippable. It confirms the game still works end to end, updates the design
contract, and records the gameplay impact so future changes do not accidentally undo the new tank
model.

## Expected Code Touches

- `DESIGN.md`
- `server/src/rules/defs.rs` or `server/src/config.rs` if balance is tuned
- `client/src/config.js` if mirrored values changed
- test fixtures or replay artifacts touched by earlier phases

## Refactor Depth

Low if earlier phases were completed cleanly. Medium if playtest reveals balance fallout or AI tank
movement assumptions.

## Done When

- Full relevant tests pass.
- `DESIGN.md` matches the implemented behavior.
- Patch notes explain the player-facing movement and balance impact.
- The remaining risks are explicit and assigned to follow-up work.

