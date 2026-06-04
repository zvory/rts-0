# Phase 5 - Audit and Removal of Legacy Hacks

Goal: remove stale special cases and prove that the new legality layer covers the old failure
class end to end.

Run this after Phases 0-4, or after Phases 0-3 if local steering is intentionally skipped.

## Scope

In scope:

- Audit all call sites that ask whether terrain, occupancy, spawn points, or building sites are
  clear.
- Remove duplicate center-tile checks where standability should be used.
- Tighten invariants.
- Update `DESIGN.md` and movement docs to describe the final model.
- Add missing regression coverage discovered during the audit.

Out of scope:

- No new movement features.
- No balance changes.
- No protocol changes unless another movement phase already requires them.

## Files To Audit

- `DESIGN.md`
- `docs/movement/PLAN.md`
- `docs/movement/PHASE_6_LOCAL_STEERING.md`
- `server/src/game/services/occupancy.rs`
- `server/src/game/services/move_coordinator.rs`
- `server/src/game/services/movement.rs`
- `server/src/game/services/production.rs`
- `server/src/game/services/construction.rs`
- `server/src/game/services/commands.rs`
- `server/src/game/services/pathing.rs`
- `server/src/game/invariants.rs`
- `server/src/game/selfplay.rs`
- `server/src/game/ai.rs`
- `server/src/game/ai_core/decision.rs`

## Removal Checklist

Look for and either remove or justify:

- local center-tile checks for unit body legality,
- spawn fallback that returns an invalid point,
- production queue removal before a legal spawn point exists,
- building placement checks that only compare unit center tiles,
- worker ejection as normal construction flow,
- comments claiming hard non-stacking when only best-effort overlap cleanup is guaranteed,
- tests that assert only center tile outside footprint for large units.

## Invariant Checklist

By the end of this phase, debug/test invariants should cover:

- no non-ghost unit body intersects a building body,
- no production-created unit begins in dynamic overlap,
- no building footprint overlaps any living unit body when created,
- no building overlaps another building,
- no building overlaps a resource node tile or body,
- no unit has non-finite position or body radius,
- tolerated unit-unit residual overlap is documented and tested.

## Regression Scenarios

Add direct tests or self-play assertions for:

- repeated tank production from one factory with the first exit occupied,
- factory fully surrounded by buildings or units,
- blocked production recovering after an exit opens,
- tank moving along a building corner,
- collision pushing near a new scaffold,
- construction attempted near a tank body at the footprint edge,
- build preview and command-time validation allow only the chosen builder's own body as the
  build-over-self exception,
- AI expansion building still finding legal sites,
- movement Phase 3 tank body tests still passing,
- movement Phase 6 steering tests still passing if steering landed.

## Documentation Updates

Update `DESIGN.md` after implementation lands. It should describe:

- unit bodies and building bodies,
- standability policies,
- production blocking on no legal spawn point,
- construction placement using body-aware footprint checks,
- client build preview mirroring server build-intent rules,
- movement using body-aware static legality,
- collision as deterministic overlap cleanup, not the only correctness layer.

Update `docs/movement/PLAN.md` if local steering now depends on exported footing or standability
helpers.

## Commands

Run:

```bash
cd server && cargo fmt && cargo test
node tests/server_integration.mjs
node tests/regression.mjs
node tests/ai_integration.mjs
cd tests && npm install && node client_smoke.mjs
```

If self-play fails and the cause is unclear, follow `CLAUDE.md`: start a fresh server and open the
local `/dev/selfplay?replay=...` URL with the macOS `open` command.

## Acceptance Criteria

- The known class of spawn/overlap/building-body bugs has direct regression coverage.
- Gameplay systems use one standability layer for body legality.
- Remaining overlap tolerance is narrow, documented, and not hiding ordinary body clipping.
- Documentation matches implementation.
- Broad tests pass or unrelated failures are documented.
