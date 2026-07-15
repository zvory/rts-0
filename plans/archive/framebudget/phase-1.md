# Phase 1 - Own and Measure the Complete Frame

## Phase Status

- [x] Done.

## Objective

Make `Match` the sole animation-frame owner during a match, disable Pixi's automatic ticker, and
explicitly present exactly once inside the measured Match frame. Separate scene-update cost from
actual Pixi/Babylon presentation cost in local diagnostics and bounded Mainline telemetry, with
measurements that expose misses against the 16.67 ms 60 FPS work budget.

Establish deterministic local 200- and 300-supply workloads through the server-authoritative
dev-scenario path. The measured browser must be an active player with normal prediction enabled;
spectator, replay, client-mutated, or prediction-disabled evidence is insufficient for later phases.
Use the existing client-only `supply-300-hellhole-stream` for renderer-ceiling comparisons without
making the server simulation the limiting factor. Server-heavy Lab runs remain a separate
simulation/projection stress lane and do not replace the active-player evidence.

## Constraints and Non-Goals

- Preserve the detached, frozen `PresentationFrameV1` boundary and
  `renderer.render(frame) -> {presented}` Match-facing seam.
- Keep `match.renderer` as end-to-end update plus present time. Add nested update/present phases and
  do not double-count them in `frame.work` or `frame.unattributed`.
- Publish `SelectionSceneV1` and acknowledge reconciled ground decals only after a successful actual
  present. One failed frame must not stop later RAFs.
- Normal Match rendering, fixed capture, and Map Editor must each present exactly once per frame
  owned by their respective loops. No path may restart Pixi's ticker.
- Keep the existing 33 ms legacy slow-frame meaning. Add explicit 60 FPS work-budget metrics rather
  than redefining every ordinary 60 Hz interval as slow.
- Do not optimize rigs, fog, frame-entity copies, minimap, HP bars, selection geometry, or trenches
  in this phase.
- Do not change the production supply cap or add a production command/query parameter. The 200/300
  setup is bounded local test infrastructure only.
- Do not alter or regenerate the client-only Hellhole stream while comparing renderer changes.
- Do not claim device certification or that 300 supply is safe from the Phase 1 baseline.

## Expected Touch Points

- `client/src/renderer/index.js`
- `client/src/renderer/pixi_compatibility_adapter.js`
- `client/src/renderer/babylon/presentation_adapter.js`
- `client/src/frame_recovery.js`
- `client/src/match_fixed_capture.js`
- `client/src/map_editor_viewport.js`
- `client/src/frame_profiler.js`
- `client/src/client_perf_report.js`
- `client/src/match_net_reporter.js`
- `server/crates/protocol/src/client_net_report.rs`
- `server/src/structured_log.rs`
- `scripts/parse-net-report-logs.mjs`
- `scripts/client-perf/workloads.mjs`
- `scripts/client-perf-harness.mjs`
- `server/src/dev_scenarios.rs`
- `server/src/lobby/room_task/dev.rs`
- `server/crates/sim/src/game/setup/dev_scenarios/`
- focused client, protocol, parser, structured-log, dev-scenario, and harness tests
- `docs/design/client-rendering.md`
- `docs/design/client-ui.md`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`

## Implementation Work

### 1. Establish one explicit present

- Construct Pixi with `autoStart: false` and assert that construction, fixed capture, and exit from
  capture never start its ticker.
- Separate the Pixi adapter operation into scene translation/update and a synchronous private
  `present()` that calls `app.render()` exactly once.
- Record `renderer.update` and `renderer.present` while keeping `match.renderer` around the complete
  adapter call.
- Return `presented: true` only after update and present succeed. Keep failures bounded to the frame
  and preserve the last successfully presented selection scene.
- Give Babylon equivalent update/present attribution without introducing a backend-owned loop.
- Count a renderer frame only after successful presentation so Interact readiness cannot advance on
  an update-only or failed frame.

### 2. Reconcile fixed capture, Map Editor, and teardown

- Remove the current fixed-capture double-present opportunity. Each requested capture frame must
  perform one update and one present, then resume exactly one Match RAF.
- Keep fixed-capture clock swapping but delete ticker stop/start behavior that becomes obsolete.
- Add one explicit Pixi present to each Map Editor RAF after its scene/camera updates.
- Ensure `stop()` and `destroy()` cancel owned RAFs and destroy Pixi idempotently. Teardown during
  capture or after a failed present must leave no ticker or hidden RAF callback.

### 3. Make the 60 FPS evidence honest and bounded

- Add stable local and uploaded measurements for update max/p95 and present max/p95 so neither can
  disappear from a capped top-five phase list.
- Add report-window counts for complete frame work above `1000 / 60` and present work above the same
  budget. Keep actual frame gaps and pre-callback `frame.rafDispatch` separate.
- Add a profiler bucket boundary near 16.67 ms so p95 does not jump directly from 16 to 24 ms.
- Preserve existing combined renderer fields for historical comparisons and add serde defaults for
  new scalars so old clients/logs remain parseable.
- Extend structured logs and the incident parser to distinguish scene update, actual present,
  RAF-dispatch pressure, unattributed work, and 60 FPS work-budget misses.
- Document units, rounding, reset windows, nested-phase interpretation, and the difference between
  CPU-throttle evidence and real-device certification.

### 4. Add exact active-player 200/300 workloads

- Add one bounded local dev scenario accepting only target `200` or `300` and rejecting other
  values. Build the same deterministic two-player mixed late-game composition and placement at both
  targets, scaling only the mobile force.
- Create all entities through simulation-owned setup. The 300 fixture may exceed normal cap
  validation locally, but must not change global balance, production validation, or normal lobby
  behavior.
- Launch the measured browser as an active player and retain the ordinary compatible prediction and
  WASM path.
- Store exact expected per-player supply, per-owner/per-kind unit counts, total projected regular
  entity count, player identity, and prediction mode in the workload descriptor.
- Fail before sampling if any expected value differs. Reset profiler/report windows only after the
  assertions pass and at least two explicit frames have presented successfully.
- Record workload assertions, `supplyUsed`, production `supplyCap`, player/spectator status,
  prediction mode, composition, and projected entity count in local artifacts only.
- Add a paired command that runs 200 and 300 with the same seed, viewport, DPR, CPU throttle,
  duration, and repeat count.

### 5. Pin failure and lifecycle contracts

Add focused tests proving:

- Pixi receives `autoStart: false`, its ticker never starts, and one successful Match RAF calls
  `app.render()` exactly once.
- Update failure and present failure both return `presented: false`, publish no selection scene or
  decal acknowledgement, reschedule the loop, and allow the next frame to present.
- Fixed capture presents exactly once per requested frame and resumes one Match RAF.
- Map Editor presents once per editor RAF and teardown cancels its loop.
- Update and present measurements reach local summary, bounded upload, structured logs, and parser
  output with correct reset behavior.
- The dev scenario produces exact authoritative 200/300 supply and unit-kind counts.
- The harness refuses wrong supply/entity counts, spectator mode, disabled prediction, or
  client-mutated setup.

## Focused Verification

```bash
node scripts/check-client-architecture.mjs
node tests/client_contracts.mjs
node tests/net_report_log_parser.mjs
node tests/select-suites.mjs --verify
cargo test --manifest-path server/Cargo.toml -p rts-protocol
cargo test --manifest-path server/Cargo.toml -p rts-sim dev_scenario
cargo test --manifest-path server/Cargo.toml structured_log
node scripts/client-perf-harness.mjs --workload supply-200-active --seconds 10
node scripts/client-perf-harness.mjs --workload supply-300-active --seconds 10
node scripts/client-perf-harness.mjs --workload supply-300-hellhole-stream --seconds 10
node scripts/check-docs-health.mjs
git diff --check
```

Retain paired 200/300 `summary.json` artifacts from identical default settings plus the client-only
Hellhole stream summary as the baseline for Phase 2. GitHub's `Main test gate` remains the
authoritative full suite.

## Interact Lab Manual Test

Use the project-local `interact` skill from the implementation worktree. Open an `interact lab`
Pixi session, run `catalog`, arrange one small authoritative scene with a Tank, infantry, a
building, and visible fog/feedback, and confirm it with `inspect`. Capture one clean 1000x700 DPR 1
PNG, inspect the artifact once, reject blank/stale/missing-texture output, close the session, and
include only the returned Tailnet Preview URL in the handoff.

Also manually exercise camera movement, a normal live frame, fixed capture, and one leave/re-enter
cycle. The screenshot supports visual review but does not replace exact present-count tests.

## Acceptance Evidence

- One live Match RAF equals one adapter call and exactly one Pixi `app.render()`.
- Pixi ticker starts zero times across normal frames, capture enter/exit, and teardown.
- Fixed-capture and Map Editor present counts equal their executed owned-frame counts.
- `frame.work` and `match.renderer` include presentation, while update and present remain separately
  attributable locally and in Mainline reports.
- A failed present cannot publish new interaction state and the next frame can recover.
- Both workloads prove exact authoritative composition, `spectator: false`, compatible active
  prediction, expected supply, and expected projected-entity count before sampling.
- The paired baseline uses identical settings and makes no hardware-certification claim.
- The Interact capture is visually correct and its inspected Tailnet URL is preserved in the
  handoff.

## PR and Handoff Requirements

- This phase is complete; its former Phase 0 dependency was later dispensed in favor of separate
  client- and server-saturation lanes.
- Mark this phase Done in the implementation commit.
- Run `scripts/agent-pr.sh --verification "<focused checks and paired baseline passed>"`, then
  `scripts/wait-pr.sh <pr>` and verify the phase head is reachable from `origin/main`.
- The handoff must list the one-frame/one-present contract, every new metric and reset window, exact
  workload composition/counts, paired artifact paths, proof of active prediction, the Interact
  Preview URL, the unchanged client-only Hellhole id and recaptured summary, remaining unattributed time,
  and the core visual/lifecycle tests Phase 2 should repeat.
