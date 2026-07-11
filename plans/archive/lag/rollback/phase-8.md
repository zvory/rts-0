# Phase 8 - Catch-up Replay and Prediction Observability

## Phase Status

- [ ] Planned.

## Objective

Make server catch-up replay and client prediction/replay observable enough to tune after rollout.
The initial server assumption is that the six-tick, roughly 200 ms rollback window is cheap enough on
the current simulation; this phase should log slow catch-up work and command-cap fallbacks instead of
blocking rollout on CPU timing proof.

## Scope

- Server catch-up diagnostics:
  - history memory per active room
  - average and p95 restore cost
  - average and p95 replay timing for 1, 2, 4, and 6 ticks
  - worst-case command burst replay cost
  - commands absorbed while replay is active
  - commands applied late because they arrived behind the active replay cursor
  - commands applied by clamped rollback at the oldest safe replayable tick
  - room metronome delay accumulated while catch-up replay runs
  - authoritative snapshot gap observed by clients during catch-up
  - snapshot fanout cost after rollback
  - replay command-count fuse hits
  - slow catch-up replay logs for later optimization
- Do not treat server CPU timing as a rollout gate in this phase. Slow catch-up passes should be
  structured-log evidence for follow-up optimization, not an automatic reason to disable the feature.
  The normal hard fallback path is the command-count fuse, unsupported deterministic history,
  unsafe clamped rollback, or behind-cursor command timing.
- Server optimization candidates if logs later show a real problem:
  - cheaper `Game` clone/keyframe representation
  - replay snapshots at fixed intervals inside the six-tick ring
  - command-log compaction
  - avoiding unnecessary snapshot projection during replay
  - yielding or chunking catch-up replay so command inbox drains happen between replay ticks
  - narrower rollback support for expensive room modes until optimized
- Lead/window tuning:
  - compare two-tick lead with the final rollback window under healthy, jittery, and bursty
    profiles
  - report how often commands fall back late at each tested lead/window combination
  - report how often outside-window commands use clamped rollback instead of live fallback
  - document the feel tradeoff before raising default lead or changing the six-tick window
- Client prediction diagnostics:
  - evaluate moving WASM prediction/replay work to a Web Worker or equivalent isolated scheduler
  - keep the no-JS-build-step development model unless a generated WASM worker wrapper is
    explicitly checked in and documented
  - preserve the existing Movement prediction setting as the gate for worker-backed prediction
- Add diagnosed client modes:
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

- Server diagnostic tests or harness runs for:
  - no rollback baseline
  - 1, 2, 4, and 6 tick rollback replay
  - rollback during command bursts
  - two-player alternating late commands during catch-up replay
  - command arriving behind the active replay cursor
  - outside-window command using clamped rollback
  - synthetic slow catch-up pass that records metronome delay and latest-snapshot gap
  - rollback with representative entity counts
  - fallback path when `MAX_REPLAY_COMMANDS` is exceeded
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
  - prediction worker startup and steady-state timing
- Net report/structured log tests if fields change.
- Run:
  - server rollback/catch-up diagnostic command added or updated by this phase
  - `node tests/client_contracts.mjs`
  - `node tests/prediction_controller.mjs`
  - `node tests/sim_wasm_smoke.mjs` when WASM assets are present
  - browser perf harness command added or updated by this phase
  - protocol/logging tests if report fields change

## Manual Testing Focus

Play or replay a busy local match on a weaker machine or throttled browser profile. Movement
prediction on should still paint provisional command response promptly; server catch-up logs should
show replay distance, absorbed commands, clamped rollback commands, behind-cursor late commands,
metronome delay, snapshot gap, and replay elapsed time.

## Handoff Expectations

The handoff must include measured server catch-up timing logs, whether the six-tick window produced
slow replay evidence worth follow-up, metronome-delay and snapshot-gap evidence, the replay
command-count fuse behavior, the chosen client execution model, final lead/window/clamped-fallback
tuning recommendation, new report fields if any, whether a Worker is required before broad rollout,
and whether later phases may enable full visual prediction or must stay in accepted-intent-overlay
mode.
