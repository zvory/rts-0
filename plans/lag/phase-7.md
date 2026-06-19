# Phase 7 - Prediction Worker and Frame-Pacing Budget

## Phase Status

- [ ] Planned.

## Objective

Keep the command-cadence fix from becoming a frame-pacing problem on weaker clients. Prediction and
replay work should stay within explicit browser budgets, degrade gracefully, and preserve
accepted-intent feedback even when full WASM visual prediction is temporarily reduced.

## Scope

- Evaluate moving WASM prediction/replay work to a Web Worker or equivalent isolated scheduler.
- Keep the no-JS-build-step development model unless a generated WASM worker wrapper is explicitly
  checked in and documented.
- Preserve the existing Movement prediction setting as the gate for worker-backed prediction.
- Add budgeted modes:
  - full visual prediction
  - reduced horizon or lower-frequency prediction
  - accepted-intent overlay only
  - authoritative-only when the Movement prediction setting is off or compatibility fails
- Report:
  - prediction worker startup time
  - replay ticks per frame/window
  - worker round-trip delay
  - main-thread prediction apply time
  - dropped/degraded prediction windows
  - frame gaps during command bursts
- Do not hide frame stalls behind prediction. If the client cannot paint the provisional response
  promptly, the lag requirement is not satisfied.

## Expected Touch Points

- `client/src/sim_wasm_adapter.js`
- new prediction worker module if needed
- `client/src/match.js`
- `client/src/frame_profiler.js`
- `client/src/client_perf_report.js`
- `client/src/protocol.js`
- `server/crates/protocol/src/lib.rs`
- `server/src/structured_log.rs`
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs`
- browser perf harness files, if present or added

## Verification

- Unit/contract tests for worker lifecycle:
  - init success
  - init failure fallback
  - match teardown frees worker/WASM resources
  - prediction toggle off stops worker-backed prediction
  - reduced mode keeps accepted-intent overlays while clearing full predicted snapshots
- Perf harness checks for:
  - representative command burst
  - high entity count
  - CPU throttled browser profile if supported
  - prediction worker startup and steady-state budget
- Net report/structured log tests if fields change.
- Run:
  - `node tests/client_contracts.mjs`
  - `node tests/prediction_controller.mjs`
  - `node tests/sim_wasm_smoke.mjs` when WASM assets are present
  - browser perf harness command added or updated by this phase
  - protocol/logging tests if report fields change

## Manual Testing Focus

Play or replay a busy local match on a weaker machine or throttled browser profile. Movement
prediction on should still paint provisional command response promptly; if prediction degrades, it
should degrade to accepted-intent feedback before falling all the way back to remote-feeling
authoritative-only behavior.

## Handoff Expectations

The handoff must include the chosen execution model, measured budgets, fallback thresholds, new
report fields if any, and whether a Worker is required before broad rollout.
