# Phase 3 - Economy and Expansion Managers

Status: Not implemented.

## Objective

Move worker, resource, supply, and expansion decisions into explicit managers. The AI should make
economy growth and expansion timing legible while continuing to use ordinary command validation,
budget reservation, worker reservation, and placement helpers.

## Scope

- Add an economy manager for worker targets, steel/oil assignment, supply depots, and resource
  blocker reporting.
- Add an expansion manager for City Centre planning, site ranking, blocked-site reporting, and
  expansion worker assignment boundaries.
- Preserve current baseline behavior for `rifle_flood_full_saturation` unless a specific test
  fixture proves a safer bug fix is needed.
- Prepare the manager data model for the new AI 1.0 profile's earlier expansion requirement.
- Add authored mid-game scenario tests where expansion can be evaluated without simulating a full
  opening.

## Expected Touch Points

- `server/crates/ai/src/ai_core/decision/resources.rs`
- `server/crates/ai/src/ai_core/decision/expansion.rs`
- `server/crates/ai/src/ai_core/decision/production.rs`
- `server/crates/ai/src/ai_core/actions.rs`
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `server/crates/ai/src/selfplay/`
- `docs/design/ai.md` if expansion behavior or debug trace contracts change

## Verification

- Add fast tests for worker target calculation, steel/oil reassignment, supply blocker handling,
  expansion site selection, and blocked expansion reasons.
- Add at least one compact expansion scenario that starts near the expansion decision point.
- Run:

```bash
cd server && cargo test -p rts-ai
```

- Run a bounded matchup comparing current baseline behavior before and after the manager extraction.

## Manual Testing Focus

Inspect a replay or dev artifact where the AI expands and confirm the selected site covers the
intended resource line, workers do not abandon main-base resources incorrectly, and the trace names
why expansion was delayed or started.

## Handoff Expectations

The handoff must state how economy and expansion goals are represented, which blocked expansion
cases are covered by tests, and what Phase 4 should reuse for the new tech/production profile.

## Player-Facing Outcome

The baseline AI should remain familiar. Internally, economy and expansion become explicit enough
for AI 1.0 to expand earlier and more reliably.
