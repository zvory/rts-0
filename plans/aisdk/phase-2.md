# Phase 2 - Canonicalize Live and Offline Execution

## Phase Status

- [ ] Ready for implementation after Phase 1 merges.

## Objective

Extract one canonical AI tick driver from the production host, make `AiController` the single normal
profile runtime, and remove the duplicated profile state machine from offline self-play. The live
room, Phase 1 oracle, matchup, arena, balance tools, and normal profile-backed tests must execute the
same alive-policy, observation, retreat, collect-before-enqueue, cadence, filtering, and trace
sequence. The Phase 1 Jeff transcript must remain exact through this production cutover; changes to
the old offline baseline are intentional.

## Design Constraints

- Keep `AiController::think` as the sole owner of profile decision memory, static map cache,
  pending-build tracking, staged/held/active-attack bookkeeping, nine-tick player staggering,
  production `BuildSearch`, command filtering, and trace bounding.
- Extract a small `CanonicalAiTickDriver` (name may vary) that is called by the live room, offline
  profile hosts, and Phase 1 transcript runner. It owns the AI-only orchestration sequence and its
  alive-policy choice, but not `Game::tick()`, WebSocket delivery, room teardown, or match outcome.
- Preserve the existing live room sequence mechanically: choose the same alive IDs for AI-only and
  mixed modes, obtain every immutable pre-tick input, invoke controllers in the same order, collect
  all results before enqueue, then enqueue in controller/result order.
- Each offline tick must obtain current start/snapshot/retreat inputs, collect every controller's
  commands before enqueueing any, and preserve stable player/controller order.
- AI-only matchup uses the same starting-primary-base alive policy as live AI-only rooms. Mixed live
  matches retain their existing normal alive-player policy; outcome policy remains a host concern.
- Retreat commands remain first and may be emitted on ticks when the strategic decision cadence is
  not due.
- Do not preserve the incorrect offline six-tick cadence, unconditional tick-zero think,
  away-from-center/default build search, missing retreat reflex, or duplicated stage filtering.

## Work

- Extract the current AI portion of `server/src/lobby/live_tick.rs` behind the canonical driver and
  make the production host call it. Move the Phase 1 runner to the same driver before changing
  offline callers, and require the immutable fixture to pass at that intermediate point.
- Replace normal `ProfileBackedScript` execution with a thin adapter around `AiController`, or drive
  controllers directly from the harness. The adapter may translate harness inputs and expose only a
  newly produced trace whose `trace_tick` matches the current tick; it must own no decision memory,
  placement logic, pending-build tracker, or combat-stage state.
- Route normal profile execution through this path in:
  - `run_profile_matchup_result`, and therefore `ai-matchup`, `ai-arena`, and balance matrix;
  - `LiveSelfPlay` and profile-backed self-play tests;
  - `real_ai_vs_real_ai`, live 2v2 tests, and relevant performance harnesses where they duplicate
    normal profile orchestration.
- On every offline tick, pass `game.worker_retreat_commands_for(player_id)` into the production
  context, including non-decision ticks.
- Remove profile-only duplicate imports, fields, helpers, and tests from `selfplay/scripts.rs`,
  including decision memory, map cache, pending builds, staged/held/active sets, build-search
  closure, cadence, and stage filtering.
- Retain purpose-built `WorkerRushScript`, `MineOnlyScript`, and other genuinely synthetic harness
  scripts. Rename any remaining six-tick constant so it is unmistakably scoped to those scripts.
- Handle economy-only coverage with either a clearly named command-filtering wrapper around the
  canonical controller or a non-hostile fixture; do not retain a copied profile runtime or add an
  economy-only production mode.
- Keep arena/matchup scoring, replay, metrics, and artifact responsibilities in the harness. Record
  and document the intentional old-to-new offline baseline changes rather than obscuring them.
- Update `docs/design/ai.md` and `docs/design/testing.md` so production controller semantics are the
  canonical profile semantics and offline tooling is described as an orchestrator, not a second AI.

## Expected Touch Points

- `server/crates/ai/src/selfplay/scripts.rs`.
- `server/crates/ai/src/live.rs` or a focused canonical driver module.
- `server/crates/ai/src/selfplay/replay.rs`.
- `server/crates/ai/src/selfplay/live.rs` and `selfplay/mod.rs`.
- Self-play harness, scripted, resource-regression, real-AI, and live-AI tests.
- `server/src/tools/ai_perf_harness.rs` if it duplicates normal profile execution.
- `server/src/lobby/live_tick.rs` for the mechanical production-host delegation.
- Focused driver and `server/crates/ai/src/live.rs` tests, without changing controller behavior.
- `docs/design/ai.md` and `docs/design/testing.md`.

## Implementation Checklist

- [ ] Extract one shared AI tick driver and route the live host and Phase 1 oracle through it first.
- [ ] Replace normal `ProfileBackedScript` state with a thin canonical-controller adapter.
- [ ] Inject retreat commands and preserve collect-before-enqueue ordering offline.
- [ ] Use production cadence, build search, stage suppression, and trace semantics everywhere.
- [ ] Keep synthetic scripts explicit and separate from normal profile execution.
- [ ] Remove duplicated profile state and helper code.
- [ ] Record deterministic old-to-new offline baseline changes.
- [ ] Pass the unchanged normal and full Phase 1 transcript fixture.
- [ ] Update AI/testing design documentation.
- [ ] Mark this phase done in the implementation commit.

## Verification

- Run the unchanged Phase 1 normal and `RTS_FULL_AI_TESTS=1` Jeff oracle; any fixture or production
  output change is a blocker.
- Add focused driver tests for AI-only versus mixed alive policy, player-staggered decision ticks,
  retreat passthrough on non-decision ticks, stable controller order, immutable pre-tick
  observation, eliminated-controller skips, collect-before-enqueue, and traces appearing only on
  decision ticks.
- Run the `rts-ai` nextest suite with and without `RTS_FULL_AI_TESTS=1`.
- Run a fixed-seed production-semantic matchup and arena lane:

```bash
cargo run --manifest-path server/Cargo.toml --bin ai-matchup -- \
  jeffs_ai ai_2_1 --seed 7 --ticks 9000 --json
cargo run --manifest-path server/Cargo.toml --bin ai-arena -- \
  --candidate jeffs_ai --baseline ai_2_1 --seeds 1 --ticks 9000
```

- Run replay verification, crate-boundary checks, simulation archcheck, docs health, and diff check.

## Manual Test Focus

Save and inspect one production-semantic Jeff-versus-AI-2.1 matchup replay and its decision trace,
checking normal macro, retreat reflexes, staging, and completion. The Phase 1 transcript is the live
behavior authority; visual similarity cannot excuse a transcript change.

## Non-Goals

- No typed `AiFrame`, public strategy trait, rulebook, task lifecycle, planner, or query surface.
- No simulation, protocol, balance, or profile-selection changes. The production live-loop change
  is limited to mechanical delegation into the shared driver guarded by the Phase 1 fixture.
- No cadence configurability or preservation of the old offline behavior.
- No cleanup of synthetic non-profile scripts beyond naming and boundary clarity.

## Handoff Expectations

Report the removed duplicate runtime state, final canonical driver/adapter/call sites, old-to-new offline
decision ticks and representative command/build-tile changes, retreat/trace changes, fixed-seed
determinism evidence, proof that the live host and oracle invoke the same driver, and unchanged
Phase 1 fixture identity. Tell Phase 3 exactly where raw
`StartPayload`/`Snapshot` construction and `AiController` ownership now live.
