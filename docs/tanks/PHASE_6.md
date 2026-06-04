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

## Implementation Audit

- `DESIGN.md` now records the implemented tank hull dimensions, standability contract, collision
  behavior, route-following lookahead, reverse/pivot thresholds, traffic braking, oil burn, and the
  current balance table.
- `server/src/config.rs` and `client/src/config.js` mirror the tank body values:
  `50.4px` length, `28.8px` width, and `1.5px` clearance.
- The current tank locomotion model has bounded hull turn and per-tick throttle scaling, but no
  persistent velocity or acceleration state. Alignment, traffic, and zero-oil checks are the
  implemented braking mechanisms.

## Patch Notes

- Tanks now use a documented oriented hull footprint for movement legality, selection affordances,
  placement previews, and collision instead of relying on a circular approximation.
- Tank hulls turn at a bounded rate, pivot in place when badly misaligned, reverse only for nearby
  behind-the-hull goals, and slow down for sharp alignment errors or frontal traffic.
- Tanks keep moving while their turret tracks and fires; plain move orders only opportunistically
  shoot enemies already in range, while attack-move can chase to a standoff firing point.
- Tank movement oil burn remains distance-based: crossing one full 96-tile map width costs about
  10 oil, and tanks stop advancing while their owner has zero oil.
- Current tank balance is unchanged in Phase 6: 390 hp, 60 damage, 3-tile range, 72-tick cooldown,
  2.0 px/tick speed, 7-tile sight, 200 steel plus 150 oil, 6 supply, and 750-tick build time.
- Strategic impact to watch in playtests: body-aware collision and slower turning make unsupported
  tanks less able to slide through congested bases or tight corners, while open lanes remain their
  intended strength.

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
