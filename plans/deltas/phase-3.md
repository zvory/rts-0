# Phase 3 - Measure and Close the Byte Gap

## Phase Status

- [ ] Ready after Phase 2 is merged and its live active-player correctness gate passes.

## Objective

Get snapshot application-payload P95 to 1,280 bytes or below on the two named workloads using the
smallest measured follow-up changes. Do not add another compression project, reduce state or
presentation cadence, quantize visible values, or optimize sections that are not demonstrated P95
contributors.

If Phase 2 already meets the target, record the evidence, perform the correctness gate, update the
docs, and stop without changing the codec again.

## Entry Gate

Require all of the following before measurement-driven work:

- Phase 2 is merged and reachable from `origin/main`.
- Active-player deltas reconstruct the exact semantic snapshot passed to `GameState.applySnapshot`.
- Writer last-sent baseline ownership, pending replacement, skipped ticks, periodic keyframes,
  reconnect recovery, sequences, explicit entity removals, and fog/privacy tests pass.
- Diagnostics distinguish delta/keyframe frames and expose payload P95/over-budget rate, keyframe
  and fallback reasons, patch counts, server diff/serialize cost, client reconstruct/apply cost, and
  current payload composition.
- Transient-event behavior is explicitly documented and events are excluded from durable baselines.
- No unresolved stale entity, optional-field retention, hidden-target, or fog-leak bug remains.

## Measurement Work

- Measure a normal fog-filtered active-player AI/live workload in a release build through the shared
  writer encoder and a separate sent baseline for every player. If `scripts/ai-perf-harness.sh`
  still directly serializes semantic snapshots, add the smallest mode needed to exercise this path.
- Measure a player-projected `supply-300-hellhole` release workload through the actual latest-only
  slot, writer encoder, browser decoder, and reconstructor. Extend the existing Hellhole harness with
  this network mode if its current integrated Lab route does not represent an active-player delta
  connection.
- Keep the offline `.rtsstream` as a presentation regression input only, not as live delta payload
  evidence.
- Run a focused coalescing workload/test that repeatedly replaces the pending full snapshot and
  proves the eventual patch still uses the last successfully sent baseline.
- For each lane record payload p50/p95/p99/max, over-1,280 percentage, keyframe/delta ratio and
  reasons, fallback reasons, section/entity-kind bytes, entity patch counts, server diff/serialize
  P95, client decode/reconstruct/apply P95, slot replacements, and any event overflow/loss counter.
- Preserve the exact commands, revision, host/build mode, and artifacts needed to repeat the
  comparison in the handoff and `docs/perf-tracing.md`.

## Measured Decision Tree

Apply one branch at a time, rerun both workloads, and stop as soon as the target is met:

1. **Both P95 values already fit:** make no codec change; record the accepted evidence.
2. **A ticking countdown drives repeated records:** replace only the demonstrated fields with exact
   absolute target ticks in the compact transport state. Derive the existing semantic remaining
   value for the snapshot tick before `GameState.applySnapshot`; do not convert progress whose
   pause/rate semantics cannot be represented exactly.
3. **One durable auxiliary section dominates:** send that whole section only when changed if rare,
   or add full-record add/update/remove patches for that measured section. Retain explicit clears and
   keyframe/full-section fallback; do not generalize all auxiliary collections.
4. **Non-transform entity full replacements still dominate:** first prove they mostly contain one or
   two changed slots, then add one bounded compact field-mask patch using existing slot order.
   Optional fields need explicit set/clear semantics; malformed masks and duplicate ids fail
   atomically. Keep complete replacement for uncommon changes.
5. **Float widening remains material because Phase 1 could not remove it:** replace the widening
   path with a typed compact serializer that preserves exact simulation `f32` values. This is
   lossless encoding work, not quantization.
6. **Pending replacement demonstrably loses required transient events:** add a small bounded
   per-recipient accumulator of already projected events. Attach accumulated events once to the next
   sent frame, keep them outside durable baseline/keyframe state, then clear them; add an overflow
   counter and documented drop policy rather than acknowledgements or a second event channel.

Do not continue down the tree for extra savings after the accepted target is met.

## Completion Gate

- The normal active-player lane reports P95 at or below 1,280 application bytes.
- The player-projected supply-scale lane also meets that target, or measured evidence proves the
  remaining gap requires a material design expansion and the user explicitly chooses that expansion
  or accepts a documented scale exception.
- Keyframes and legitimate mass-change frames may exceed the budget but remain below 5% so they do
  not define P95.
- No state fidelity, presentation cadence, fog authority, transient-event policy, or command
  acknowledgement semantics are reduced.
- Skipped ticks, removals, explicit clears, reconnects, and keyframes leave no stale state.
- Server diff/serialization and client reconstruction are not the measured tick/frame bottleneck.

If the target still misses after the simple measured choices, stop with the exact dominant section,
byte gap, and next design tradeoff. Do not mark the phase complete as successful or start Phase 4
without the required user decision.

## Expected Touch Points

Touch only the branches selected by the evidence:

- Phase 1/2 stream encoder and client reconstructor modules
- `server/crates/protocol/src/compact_snapshot.rs`
- `server/crates/protocol/src/messagepack_frame.rs` if typed encoding remains necessary
- `server/crates/protocol/src/contract_metadata.rs`
- `server/src/lobby/connection.rs` if event accumulation is selected
- relevant AI/Hellhole perf harness code and launch scripts
- `client/src/protocol_snapshot.js`
- `scripts/net-report-snapshot-payload.mjs`
- `scripts/parse-net-report-logs.mjs` only if diagnostics change
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- focused Rust/JS protocol, reconstruction, writer, harness, and parser tests

`client/src/state.js`, renderer, HUD, minimap, and input should remain unaware of transport deltas.

## Implementation Checklist

- [ ] Confirm the entry gate and preserve Phase 2's exact event policy.
- [ ] Make both named workloads exercise the real writer/reconstructor path.
- [ ] Capture comparable release-build baseline measurements and artifacts.
- [ ] Identify the dominant remaining P95 contributor using existing diagnostics.
- [ ] Apply only the necessary decision-tree branch and rerun both workloads after each change.
- [ ] Prove exact semantic reconstruction for every selected representation.
- [ ] Record final byte, keyframe/fallback, and processing-cost evidence.
- [ ] Stop for a user decision if the remaining gap requires material scope expansion.
- [ ] Update protocol/perf docs and focused tests.
- [ ] Mark this phase done only after its completion gate is satisfied.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-protocol`
- Focused Rust delta encoder, writer, coalescing, and selected-representation tests.
- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- Focused client reconstruction tests for every selected representation.
- Exact reconstructed-semantic comparisons across keyframes, ordinary deltas, skipped ticks,
  removals, optional clears, and malformed frames.
- Event replacement/overflow tests only if accumulation is implemented.
- Repeatable release runs for the normal active-player and player-projected supply-scale lanes.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Run a normal fog match through movement, combat, deaths, production, cooldowns, smoke/ability
objects, and enemies entering/leaving vision. Introduce writer delay to force pending replacements
and confirm the view catches up without ghosts; if event accumulation was added, confirm each effect
occurs once and in order. Inspect the supply-scale lane for smooth full-state presentation and make
sure periodic keyframes are visually invisible.

## Handoff Expectations

Provide a before/after table for both named workloads, the decision-tree branch taken, branches
skipped, final P95 and over-budget rate, keyframe/fallback reasons, dominant sections, server/client
timing, final compact version, and exact event policy. State plainly whether the target is met and
whether Phase 4 may default the active-player path, or name the user decision still required.

## Deferred Backlog

- Compression or transport migration.
- Position/angle quantization, bit packing, motion prediction, or lossy encoding.
- General-purpose schema negotiation or stale-client compatibility.
- Deltas for sections that do not materially affect P95.
- Lower snapshot/render cadence, intentional stale state, or cross-frame staggering.
- Event acknowledgements or a separate event channel.
- A hard portable CI failure on absolute payload bytes.
