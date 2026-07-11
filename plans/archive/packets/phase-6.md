# Phase 6 - Auxiliary Section Deltas And Recovery

## Phase Status

- [ ] Ready for implementation after Phase 5 is merged and entity delta metrics identify the
      remaining payload/recovery gaps.

## Objective

Finish the stateful snapshot protocol around non-entity sections and recovery behavior. This phase
should decide which auxiliary sections deserve deltas, keep deliberately full sections documented,
and harden keyframe recovery across live, spectator, replay, branch, lab, and dev-watch paths.

## Design Constraints

- Do not add field-level deltas just because a section is stateful. Use full records plus remove ids
  unless measured payloads prove field-level patches are worth the complexity.
- Keep `netStatus` full every frame unless there is a measured reason to do otherwise. It is small
  and describes the frame/connection carrying it.
- Keep `events` transient and full for the current sent frame unless this phase deliberately adds a
  bounded, fog-safe event accumulator. Do not store events in the durable baseline by accident.
- Keep keyframe fallback available for every auxiliary section.
- Preserve replay/lab seek correctness over byte savings. Any time-control or projection reset may
  force a keyframe.
- Keep all recovery diagnostics bounded; do not upload raw snapshots, ids streams, or packet captures.

## Section Policy

Use this default policy unless implementation measurements justify a documented exception:

| Section | Phase 6 default |
| --- | --- |
| `smokes` | Record-level add/update/remove by smoke id. |
| `abilityObjects` | Record-level add/update/remove by object id, with owner-only state scrubbed by projection before diffing. |
| `rememberedBuildings` | Record-level add/update/remove by id; removal when memory disappears or becomes live-visible. |
| `upgrades` | Send full list only when it changes; otherwise reconstruct from baseline. |
| `playerResources` | Keep full in spectator/replay unless Phase 5 metrics show it dominates; if changed, use per-player full records. |
| `events` | Full current-frame array, not baseline state. |
| `netStatus` | Full current-frame array/object, not baseline state. |

## Work

- Implement auxiliary section deltas selected from the policy table:
  - define compact delta keys and caps in `docs/design/protocol.md`;
  - diff only projected records;
  - use remove-id arrays for durable sections with identities;
  - force keyframes when patches are larger or unsafe.
- Harden recovery:
  - finish or tighten the advisory keyframe request path from Phase 3;
  - rate-limit repeated requests and log counts, not raw frame data;
  - ensure malformed/stale/unsupported frames put the client into a wait-for-keyframe state without
    mutating `GameState`;
  - force keyframes after replay seek, replay vision change, branch promotion, lab import/reset/seek,
    lab vision change, dev-watch projection changes, and reconnect/start.
- Add compatibility checks:
  - live active player;
  - live spectator union view;
  - replay playback with vision changes and seek;
  - replay branch staging/promotion if available;
  - lab full-world and lab selected-team vision;
  - dev-watch/full-world scenarios.
- Update reporting:
  - keyframe reason counts;
  - client resync request count;
  - stale/unsupported/malformed delta count;
  - per-section full-vs-delta fallback counts;
  - reconstruction p95 if not already reported clearly.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- Phase 3-5 snapshot codec modules
- `server/src/lobby/projection.rs`
- `server/src/lobby/room_task.rs` tests for replay/lab/branch reset seams
- `server/src/main.rs`
- `client/src/protocol.js`
- `client/src/snapshot_reconstructor.js` or the Phase 3 equivalent
- `client/src/net.js`
- `client/src/client_perf_report.js`
- `scripts/parse-net-report-logs.mjs`
- `scripts/client-perf-harness.mjs`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- focused Rust and JS tests for each changed section and recovery seam

## Implementation Checklist

- [ ] Review Phase 5 metrics and identify which auxiliary sections need deltas.
- [ ] Implement selected auxiliary section deltas with keyframe fallbacks.
- [ ] Document deliberately full sections, especially `events` and `netStatus`.
- [ ] Harden stale/unsupported/malformed frame recovery and advisory keyframe requests.
- [ ] Force keyframes at replay, branch, lab, dev-watch, projection, start, and reconnect seams.
- [ ] Add bounded recovery and per-section fallback reporting.
- [ ] Update protocol/perf docs and focused tests.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- focused client reconstructor tests for every changed auxiliary section
- focused Rust codec tests for every changed auxiliary section
- focused replay/lab/branch room tests for keyframe resets
- `node tests/server_integration.mjs` if recovery or message handling changes the live connection
  contract
- `node scripts/check-docs-health.mjs`
- `git diff --check`

When practical, run at least one replay seek/vision-change flow and one lab/dev-watch flow locally
with delta mode enabled.

## Manual Test Focus

Inspect active smoke, Ekat ability objects, remembered enemy buildings, upgrades, spectator resource
views, replay seeking, replay vision switching, lab time controls, and dev-watch/full-world views.
Confirm malformed or stale delta diagnostics do not corrupt the visible game state and that the next
keyframe recovers the client.

## Handoff Expectations

List every auxiliary section that now uses deltas, every section intentionally kept full, and the
measured byte impact. Include recovery metrics and any mode that still forces frequent keyframes.
Tell Phase 7 whether delta mode is ready for default rollout, opt-in beta testing only, or deferral.
