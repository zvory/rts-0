# Phase 1 - Placement Classification And Build-Wait State

Status: done.

## Goal

Add the internal simulation concepts needed for build-site waiting without changing the full
player-visible behavior yet. The main outcome should be a classified placement probe that can tell
construction why a footprint is blocked, plus build-order execution state that can later remember
an arrived worker's wait and unit-block timeout progress.

## Scope

- Add a build placement status/classification helper in or near
  `server/crates/sim/src/game/services/standability.rs`.
- Preserve the existing boolean helpers as wrappers where useful so unrelated callers do not need
  broad churn.
- Classify at least these outcomes:
  - clear;
  - invalid footprint, out of bounds, or impassable terrain;
  - blocked by building or scaffold;
  - blocked by resource node;
  - blocked by relevant unit body.
- Preserve Tank Trap placement policy while classifying blockers. Infantry-like units should still
  be ignored for Tank Trap placement, and vehicle-body blockers should classify as unit blockers.
- Add build execution state capable of representing an arrived worker waiting for construction to
  start and the current unit-blocked tick count.
- Add a single timing constant for the unit-block grace, derived from `config::TICK_HZ * 3`.
- Keep current behavior through compatibility wrappers where possible. This phase should not make
  workers wait for resources yet.
- Add focused unit tests for blocker classification and state helpers.

## Expected Touch Points

- `server/crates/sim/src/game/services/standability.rs`
- `server/crates/sim/src/game/services/construction.rs`
- `server/crates/sim/src/game/entity/order.rs`
- `server/crates/sim/src/game/entity/entity.rs`
- Existing focused tests in:
  - `server/crates/sim/src/game/services/construction.rs`
  - `server/crates/sim/src/game/services/standability.rs`
  - `server/crates/sim/src/game/entity/tests.rs`

Avoid touching:

- Client code
- Wire protocol mirrors
- Balance numbers except for a sim-local grace constant tied to `TICK_HZ`
- AI strategy code

## Implementation Notes

- Prefer a small enum such as `BuildSiteStatus` or `BuildPlacementStatus` instead of overloading
  booleans with side channels.
- Keep the builder's own unit body ignored for ordinary build placement, matching current
  `building_site_clear_for_build_intent` behavior.
- Treat under-construction buildings as building blockers for classification. The player-facing
  rule says another building blocking the spot cancels the order; a scaffold has the same footprint
  consequence.
- If adding a new `BuildPhase` variant, update helper methods rather than open-coding variant
  matches across services.
- If the state shape would cause large mechanical churn, it is acceptable to keep `BuildPhase` small
  and add tick counters to `BuildExecution`, provided the later construction logic can clearly reset
  and increment them.
- Do not add snapshot or protocol fields for the waiting state in this phase.

## Verification

- Focused Rust tests for placement classification:
  - clear footprint;
  - building/scaffold overlap;
  - resource node overlap;
  - relevant unit overlap;
  - Tank Trap ignores infantry but sees vehicle blockers.
- Focused Rust tests for new build execution state/helper resets.
- Existing construction tests that encode current behavior should still pass unless intentionally
  adjusted only for the new internal state representation.
- `cargo test --manifest-path server/Cargo.toml -p rts-sim build_site`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim construction`
- `git diff --check`

Use narrower filters if implementation names make the suggested filters too broad.

## Manual Testing Focus

No manual gameplay testing is required if this phase keeps behavior compatible. If any visible
behavior changes despite the intended scaffolding-only scope, manually try one valid worker build,
one blocked-by-building build, and one blocked-by-unit build in a local match.

## Handoff

After implementation, mark this phase done and summarize the new placement classification API, the
new build execution state shape, the grace-tick constant, and any call sites still using boolean
placement wrappers. The handoff should tell the Phase 2 agent exactly which helper to call when it
needs to distinguish building blockers from unit blockers.
