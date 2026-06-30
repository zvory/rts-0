# Phase 4 - Pathing Slow-Tick Diagnostics

## Phase Status

- [ ] Not started.

## Objective

Make slow ticks dominated by pathing self-explanatory. This phase should turn coarse phase timings
such as `awaiting_paths=297ms` into bounded summaries of request volume, request source, complexity,
budget behavior, and worst-path characteristics.

## Work

- Add internal diagnostics around `MoveCoordinator::process_awaiting_paths` without changing pathing
  behavior.
- Summarize per slow tick:
  - awaiting request count at phase start
  - requests processed
  - requests deferred or still awaiting
  - worst request duration bucket
  - total pathing duration
  - path length or explored-node buckets if available
  - source command family or order source
  - selected/unit-count buckets for grouped orders
  - cache hit/miss or reuse signals if available
  - budget exhausted or fuse-triggered signals if applicable
- Include both first `awaiting_paths` and `promoted_awaiting_paths` passes, plus
  `promote_queued_orders` when it is the slowest phase.
- Keep diagnostics emitted only for slow ticks, sample mode, or bounded aggregates.
- Extend parser output to explain pathing slow ticks separately from generic server tick pressure.

## Expected Touch Points

- `server/crates/sim/src/game/systems.rs`
- `server/crates/sim/src/game/services/pathing.rs`
- `server/crates/sim/src/game/services/move_coordinator.rs`
- `server/crates/sim/src/game/services/order_queue.rs`
- `server/crates/sim/src/game/pathfinding.rs`
- `server/crates/sim/src/perf.rs`
- `server/src/lobby/live_tick.rs`
- `scripts/parse-net-report-logs.mjs`
- `docs/perf-tracing.md`
- focused movement/pathing tests

## Agent-Readable Output Requirements

- The digest should identify whether a slow tick was primarily path request volume, path complexity,
  queue promotion, or unknown.
- Pathing diagnostics must not include raw map paths, raw positions, or full unit id lists.
- Source labels should be stable command/order families.
- If the instrumentation cannot expose cache or complexity cheaply, it must report those fields as
  unavailable rather than inferring them from duration.
- The summary should include the game scale context already available: total entities, units,
  buildings, and resources.

## Implementation Checklist

- [ ] Define pathing diagnostic fields and privacy boundaries.
- [ ] Add bounded counters to `MoveCoordinator` and related order-promotion paths.
- [ ] Thread diagnostics into `TickPerf` for slow tick logging.
- [ ] Update parser classification and digest sections for pathing.
- [ ] Add focused tests for diagnostic counters without changing movement behavior.
- [ ] Update docs with interpretation examples.
- [ ] Mark this phase as done in this file in the implementation commit.

## Verification

- focused Rust tests under movement/pathing modules
- `cargo test --manifest-path server/Cargo.toml -p rts-sim pathing`
- focused parser fixture tests for slow tick rows with pathing detail
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if
  sim architecture boundaries change
- `git diff --check`

## Manual Test Focus

Run a local perf scenario or saved replay that produces at least one slow pathing tick. Confirm the
log/digest names pathing request counts and source families without changing movement outcomes.
Confirm ordinary ticks do not spam large pathing records.

## Handoff Expectations

List all pathing diagnostic fields and which pass they describe. State whether cache, path
complexity, and source command family are fully supported or still partial. Tell the next phase which
client-frame questions remain after server hitches are explainable.
