# Phase 4 - Resource And Fog Delta Prototype

## Phase Status

- [ ] Ready for implementation after Phase 3 is merged and its keyframe-only path is verified.

## Objective

Implement the first real snapshot deltas for low-risk recurring sections: visible resource remaining
updates and `visibleTiles`. This phase should keep entities, smokes, ability objects, remembered
buildings, events, upgrades, player resources, and net status full/keyframed while proving that the
Phase 3 baseline/keyframe/recovery path can safely shrink section payloads.

## Design Constraints

- Diff only after per-recipient fog projection and after `compact_snapshot_for_wire` has applied the
  current resource/entity compaction semantics.
- Fall back to a keyframe when there is no baseline, the baseline does not match, the patch is larger
  than the full section, the section shape changes, or a safety limit is exceeded.
- Keep the reconstructed output compatible with `GameState.applySnapshot`.
- Preserve current resource semantics: start-map resources are durable client state, resource updates
  are remaining-value updates, and resource death/depletion must still reach the client even when
  pending snapshots are coalesced.
- Preserve current fog semantics: `visibleTiles` must reconstruct to the full row-major current
  visibility array for fogged views, and full-world/no-fog projections must remain an explicit empty
  array.
- Do not introduce entity deltas in this phase.

## Delta Shape

Use the Phase 3 delta frame body (`d`) and define these fields in `docs/design/protocol.md`:

- Resource updates: reuse compact resource rows for changed visible resource remaining values, for
  example `d.r = [[id, remaining], ...]`.
- Visible tile patches: encode changed contiguous tile runs against the baseline full tile array, for
  example `d.fg = [[startIndex, runLen, value], ...]`.
- Section fallback marker: if either section cannot be patched safely, send a keyframe instead of a
  partial full-section override unless implementation evidence shows an override is simpler and
  safer.

The client reconstructor should apply `d.r` to its resource baseline and `d.fg` to the baseline full
`visibleTiles` array before producing the semantic snapshot passed to `GameState`.

## Work

- Add server diff helpers for resource updates and visible tiles:
  - compare the current projected snapshot against the last sent baseline for this connection;
  - keep deterministic ordering for resource ids and tile patches;
  - include depletion/death-driven resource updates even when a pending snapshot was replaced;
  - choose a clear tile patch cap and keyframe when exceeded.
- Add client reconstruction for these sections:
  - validate ids, remaining values, tile indices, run lengths, and values;
  - reject patches that exceed map bounds or compact section limits;
  - reconstruct `visibleTiles` into the full array expected by rendering/state code;
  - leave default full/keyframe decoding untouched.
- Add measurement:
  - report keyframe bytes versus resource/fog delta bytes;
  - report keyframe reason counts and delta fallback counts;
  - measure reconstruction p95 separately when practical so it is not hidden inside apply cost.
- Update docs and tests:
  - document section delta shapes and fallback rules in `docs/design/protocol.md`;
  - update `docs/perf-tracing.md` and parser/harness output if metrics are emitted;
  - add property-style tests that applying resource/fog deltas to a baseline equals applying the
    current projected snapshot for these sections.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- `server/src/main.rs` or the Phase 3 snapshot codec module
- `server/src/lobby/connection.rs` if baseline structures live there
- `server/src/lobby/snapshots.rs`
- `client/src/protocol.js`
- `client/src/snapshot_reconstructor.js` or the Phase 3 equivalent
- `client/src/client_perf_report.js` if reconstruction metrics are reported
- `scripts/client-perf-harness.mjs`
- `scripts/parse-net-report-logs.mjs` if log parsing changes
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- focused Rust and JS tests for patch generation/reconstruction

## Implementation Checklist

- [ ] Confirm Phase 3 keyframe-only path is merged and default/fallback behavior is clear.
- [ ] Implement resource remaining-value deltas with coalescing-safe depletion behavior.
- [ ] Implement visible-tile run patches with map-bound validation.
- [ ] Add keyframe fallback when patches are absent, unsafe, larger than full sections, or over caps.
- [ ] Add client reconstruction and malformed-patch rejection.
- [ ] Add measurements for bytes, reconstruction cost, keyframe reasons, and fallback counts.
- [ ] Update protocol/perf docs and focused tests.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- focused client reconstructor tests for resource/fog patches and malformed bounds
- focused Rust protocol/codec tests for resource/fog delta generation
- a focused lobby/writer test proving replaced pending snapshots diff against last sent baseline
- `node scripts/check-docs-health.mjs`
- `git diff --check`

When practical, run the Phase 1/2 packet workloads with the resource/fog delta flag enabled and
record p95 payload bytes, over-budget rate, reconstruction p95, keyframe count, and fallback count.

## Manual Test Focus

Run a normal match with fog enabled. Watch resource nodes enter vision, get harvested, deplete, and
leave/re-enter vision while fog-of-war updates around moving units; confirm the minimap/fog overlay
and visible resource remaining values match the default compact path. Also check spectator/replay,
full-world lab/dev-watch, and replay vision changes because those paths intentionally have different
`visibleTiles` semantics.

## Handoff Expectations

State the final resource and tile patch shapes, caps, fallback rules, and measured byte savings. List
any modes where the phase intentionally forced keyframes instead of patching. Tell Phase 5 whether
the baseline API was sufficient for entity deltas or needs cleanup first.
