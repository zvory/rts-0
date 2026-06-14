# Phase 3 - Economy and Expansion Managers

Status: Implemented.

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

## Implementation Notes

- Added an `EconomyPlan` that centralizes target steel workers, desired oil workers, total worker
  cap, current resource assignments, occupied resource nodes, and post-expansion local assignment
  distance.
- Added an `ExpansionPlan` that centralizes due/save decisions, tech-path blocking, and blocked
  reasons for defensive panic, missing prerequisite buildings, missing defenders, unmet City Centre
  requirements, existing/pending expansions, missing resource clusters, and no valid build site.
- `decide_profile` now routes worker training, oil/steel assignment, expansion saving, and City
  Centre build attempts through those plans while still emitting final commands only through
  `AiActionContext` and `ai_core::actions`.
- Expansion traces now report specific blocked reasons, including `no_expansion_site`, instead of
  collapsing every due-but-unbuilt expansion into generic budget or prerequisite text.
- Added fast authored decision-state tests for worker target calculation, oil reassignment timing,
  supply blockers, expansion prerequisite blockers, and blocked expansion placement. The existing
  `midgame_expansion` self-play scenario metadata remains the compact matchup scenario for this
  phase; the repo does not yet have a separate authored mid-game self-play state runner.

## Verification Notes

- `cargo test --manifest-path server/Cargo.toml -p rts-ai`
- `cargo run --manifest-path server/Cargo.toml -p rts-ai --bin ai-matchup -- full tech --ticks 3000 --seed 0`
  completed at the tick cap with replay verification passing. No winner by 3,000 ticks; first
  attack-command timings stayed in the existing early-rifle window (`tech_to_tanks` 1649,
  `rifle_flood_full_saturation` 1704).

## Handoff to Phase 4

Phase 4 should reuse `EconomyPlan` and `ExpansionPlan` instead of recalculating worker, oil, or
expansion readiness inside tech/production code. The new tech/production manager can use
`expansion_plan.blocks_tech_path`, `save_for_unplanned_expansion`, and the plan blocker labels to
decide when to hold tech spending for an expansion-first profile. Manually inspect a replay where
`steel_expansion_tanks` expands, then techs after the expansion is planned, and confirm the trace
explains whether tech was delayed by expansion, budget, missing production buildings, or defensive
panic.
