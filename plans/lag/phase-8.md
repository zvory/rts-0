# Phase 8 - Rollback and Prediction Performance Budget

## Phase Status

- [ ] Planned.

## Objective

Prove the server can afford bounded rollback and the client can afford prediction/replay without
creating frame lag on weaker machines. If the 26-tick rollback target is too expensive, this phase
must identify the optimization work or temporary lower window needed before broad rollout.

## Scope

- Server rollback budgets:
  - history memory per active room
  - average and p95 restore cost
  - average and p95 replay cost for 5, 10, 20, and 26 ticks
  - worst-case command burst replay cost
  - snapshot fanout cost after rollback
  - fallback threshold when replay would exceed budget
- Treat this phase as a rollout gate. If Phase 4 data already shows 26-tick rollback exceeding the
  documented fallback threshold under representative loads, pause default enablement of broader
  prediction surfaces and either lower the window temporarily or split out an optimization phase.
- Choose and document numeric budgets before declaring success. Initial review thresholds should
  account for the 30 Hz room budget: rollback replay plus corrected snapshot fanout must leave
  enough headroom for normal tick work, and over-budget paths must fall back late instead of
  blocking the room task.
- Server optimization candidates if needed:
  - cheaper `Game` clone/keyframe representation
  - replay snapshots at fixed intervals inside the 26-tick ring
  - command-log compaction
  - avoiding unnecessary snapshot projection during replay
  - narrower rollback support for expensive room modes until optimized
- Lead/window tuning:
  - compare two-tick lead with the final rollback window under healthy, jittery, and bursty
    profiles
  - report how often commands fall back late at each tested lead/window combination
  - document the cost and feel tradeoff before lowering the rollback window or raising default lead
- Client prediction budgets:
  - evaluate moving WASM prediction/replay work to a Web Worker or equivalent isolated scheduler
  - keep the no-JS-build-step development model unless a generated WASM worker wrapper is
    explicitly checked in and documented
  - preserve the existing Movement prediction setting as the gate for worker-backed prediction
- Add budgeted client modes:
  - full visual prediction
  - reduced horizon or lower-frequency prediction
  - accepted-intent overlay only
  - authoritative-only when the Movement prediction setting is off or compatibility fails
- Do not hide frame stalls behind prediction. If the client cannot paint the provisional response
  promptly, the lag requirement is not satisfied.

## Expected Touch Points

- `server/src/lobby/live_tick.rs`
- `server/src/lobby/room_task.rs`
- `server/crates/sim/src/perf.rs`
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
- server perf harnesses and browser perf harness files, if present or added

## Verification

- Server perf tests or harness runs for:
  - no rollback baseline
  - 5, 10, 20, and 26 tick rollback replay
  - rollback during command bursts
  - rollback with representative entity counts
  - fallback path when budget is exceeded
  - human-only rooms and AI-backed rooms, or an explicit `rollbackUnsupported` result for AI-backed
    rooms if Phase 4 left them unsupported
  - corrected snapshot fanout cost after rollback under normal active-player fog filtering
- Unit/contract tests for client worker lifecycle if a worker is added:
  - init success
  - init failure fallback
  - match teardown frees worker/WASM resources
  - prediction toggle off stops worker-backed prediction
  - reduced mode keeps accepted-intent overlays while clearing full predicted snapshots
- Browser perf harness checks for:
  - representative command burst
  - high entity count
  - CPU throttled browser profile if supported
  - prediction worker startup and steady-state budget
- Net report/structured log tests if fields change.
- Run:
  - server rollback perf command added or updated by this phase
  - `node tests/client_contracts.mjs`
  - `node tests/prediction_controller.mjs`
  - `node tests/sim_wasm_smoke.mjs` when WASM assets are present
  - browser perf harness command added or updated by this phase
  - protocol/logging tests if report fields change

## Manual Testing Focus

Play or replay a busy local match on a weaker machine or throttled browser profile. Movement
prediction on should still paint provisional command response promptly; server rollback should not
cause long stalls or repeated visible correction.

## Handoff Expectations

The handoff must include measured server rollback costs, whether 26 ticks is viable, required
server optimizations if it is not, the chosen client execution model, fallback thresholds, final
lead/window tuning recommendation, new report fields if any, whether a Worker is required before
broad rollout, and whether later phases may enable full visual prediction or must stay in
accepted-intent-overlay mode.
