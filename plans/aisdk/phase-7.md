# Phase 7 - Cut Jeff Over to the SDK

## Phase Status

- [ ] Ready for implementation after Phase 6 merges.

## Objective

Move Jeff and the shared profile decision engine onto the completed semantic SDK surfaces while
preserving the immutable Phase 1 transcript exactly. This is a parity-only cutover: compatibility
projections and wrappers may remain even when they look redundant. Cleanup, conformance expansion,
and architecture enforcement belong to Phase 8 after this migration has merged and been rerun
independently.

## Entry Criteria

- The unchanged Phase 1 normal and full transcript suites pass on current `origin/main`.
- Live, offline, and the transcript runner use the same canonical Phase 2 tick driver.
- The typed frame, narrow tasks, rulebook/queries, and action planner have public integration tests
  and documented fog/authority limits.
- There is no accepted parity exception. A mismatch is an implementation bug, not grounds to
  regenerate the fixture.

## Work

- Port the shared profile engine, including Jeff, to consume:
  - `AiFrame` for current knowledge and economy;
  - `AiRulebook` for rules and capabilities;
  - `WorldQueries` for known-world placement, geometry, and static connectivity;
  - the private task compatibility owner for pending construction and combat-stage lifecycle;
  - `AiActionPlanner` for ordered emission, budgets, and reservations;
  - `UnitGroup` only where the old call site already sorted and deduplicated tactical IDs.
- `AiFacts` may remain as an internal strategic projection, but build it from `AiFrame`, not raw
  `Snapshot`. Profile records and Jeff-specific policy remain internal.
- Keep raw translation at the established boundaries: only the adapter parses raw start/snapshot
  DTOs and strings, and only the planner emitter constructs ordinary `SimCommand` output for
  strategies. `AiController::think` may retain `Vec<SimCommand>` as its server-facing return type.
- Migrate one bounded slice at a time: observation projection, facts, placement/query delegation,
  production/economy actions, then combat actions. Run the normal transcript after every slice and
  the full transcript before committing.
- Preserve exact cadence, map-cache timing, retreat position, command and unit order,
  budget/reservation order, placement scans and floating-point arithmetic, task compatibility
  update points, pending-build behavior, stage/attack suppression, trace construction/truncation,
  and all synthetic legacy knowledge quirks.
- Do not let richer task, rulebook, query, or knowledge semantics change a Jeff choice. Keep narrow
  compatibility projections where the public truthful answer differs from historical policy input.
- Limit deletions to code that must move mechanically to establish a single owner in this cutover.
  Leave old adapters, wrappers, aliases, and helper files for Phase 8 if removing them is not
  necessary for compilation or would enlarge the parity review.
- Update `docs/design/ai.md` with the actual post-cutover data and action flow, explicitly marking
  retained compatibility seams for Phase 8.

## Expected Touch Points

- `server/crates/ai/src/live.rs` and completed `sdk/**` modules.
- `server/crates/ai/src/ai_core/{observation,facts,actions,map_analysis}.rs`.
- `server/crates/ai/src/ai_core/decision/**` and Jeff/profile modules.
- Focused compatibility tests and `docs/design/ai.md`.

Avoid changes to replay/scorecard/synthetic harness infrastructure, policy-check scripts, suite
selection, protocol, client, balance, and simulation command processing.

## Implementation Checklist

- [ ] Rerun and record the Phase 1 fixture checksum before editing.
- [ ] Move profile/Jeff inputs onto the public semantic SDK surfaces slice by slice.
- [ ] Move strategy action construction behind the planner emitter.
- [ ] Preserve all legacy projections, memory, arithmetic, ordering, and traces.
- [ ] Keep nonessential compatibility wrappers and cleanup out of this PR.
- [ ] Pass the unchanged normal transcript after each bounded migration slice.
- [ ] Pass the unchanged full transcript and full focused AI suite before delivery.
- [ ] Document the post-cutover flow and retained shims.
- [ ] Mark this phase done in the implementation commit.

## Verification

```bash
cargo nextest run --config-file .config/nextest.toml \
  --manifest-path server/Cargo.toml --profile default -p rts-ai
RTS_FULL_AI_TESTS=1 cargo nextest run --config-file .config/nextest.toml \
  --manifest-path server/Cargo.toml --profile default -p rts-ai
cargo clippy --manifest-path server/Cargo.toml -p rts-ai --all-targets
```

- Run the exact Phase 1 transcript and verify its checked-in bytes/checksum are unchanged.
- Run focused SDK public-integration, fog A/B, task, rulebook/query, planner, crate-boundary, sim
  architecture, docs-health, and diff checks.
- Inspect the PR specifically as a semantic call-site migration; defer unrelated cleanup findings to
  Phase 8 rather than folding them into this parity PR.

## Manual Test Focus

Run a deterministic Jeff-versus-AI-2.1 matchup through the cut-over runtime and inspect the opening
through first armored attack: two-Machine-Gunner screen, fast Tank path, two Tanks plus Scout Car
launch, stage/hold behavior, retreat reflexes, and decision traces. This is a sanity check only and
cannot authorize transcript differences.

## Non-Goals

- No adapter/helper purge, broad renames, directory reorganization, or aesthetic rewrite.
- No new architecture checker, suite-selection changes, or expanded conformance strategy.
- No balance, timing, targeting, production, placement, or tactical improvement.
- No transcript regeneration or accepted parity exception.
- No new task families, authoritative receipts, behavior tree, GOAP, goal scheduler, tactical
  solver, plugin ABI, or non-Rust interface.

## Handoff Expectations

Report the merged head, unchanged fixture path/checksum, per-slice and full parity commands, exact
SDK surfaces now used by Jeff, compatibility projections/wrappers deliberately retained, any
mechanical deletions required for the cutover, and the manual replay artifact. Phase 8 must begin by
checking out the merged head and independently rerunning the oracle before deleting anything.
