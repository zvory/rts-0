# Phase 8 - Rollout, Tuning, and Regression Matrix

## Phase Status

- [ ] Planned.

## Objective

Make adaptive command cadence the default behavior under the existing Movement prediction setting
after correctness and performance gates pass. This phase locks in tuning, documentation, and
regression coverage so future gameplay changes cannot quietly reintroduce remote-echo command feel.

## Scope

- Finalize tuning:
  - default `commandLeadTicks = 2`
  - maximum normal lead
  - late-arrival threshold for increasing lead
  - stable-window threshold for decay
  - correction distance budgets
  - worker/perf degradation thresholds
- Make the cadence path default for compatible live active-player sessions when Movement
  prediction is enabled.
- Keep spectators, replay viewers, unsupported factions, incompatible builds, and prediction-off
  sessions out of cadence prediction.
- Update docs and operator guidance:
  - `docs/design/protocol.md`
  - `docs/design/server-sim.md`
  - `docs/design/client-ui.md`
  - `docs/perf-tracing.md`
  - any relevant context capsules if section lists shift
- Add a concise regression matrix that maps each command family to:
  - predicted owned-world response
  - authoritative-only side effects
  - required tests
  - known unsupported cases

## Verification

- Run all focused prediction and cadence suites:
  - `node tests/prediction_controller.mjs`
  - `node tests/sim_wasm_smoke.mjs` when WASM assets are present
  - `node tests/tri_state/self_test.mjs`
  - all cadence/prediction tri-state scenario groups
  - `node scripts/check-prediction-guardrails.mjs`
  - `node scripts/check-client-architecture.mjs`
  - `node tests/protocol_parity.mjs`
  - focused Rust room/protocol/sim-wasm tests
- Run a browser perf harness covering:
  - healthy local profile
  - 100 ms RTT with jitter
  - 250 ms RTT with burst delivery
  - weaker client or CPU-throttled profile
- If practical, run one narrow live Node integration or smoke path with Movement prediction enabled
  and one with it disabled.
- Rely on the PR `./tests/run-all.sh` gate for full-suite coverage.

## Manual Testing Focus

Run short online-like matches with Movement prediction on and off. Confirm normal healthy play has
a small stable command delay, bad conditions adapt upward instead of repeatedly snapping, and
turning Movement prediction off returns to the old authoritative-only behavior.

## Handoff Expectations

The handoff must state the final rollout status, tuning values, verification commands and results,
known caveats, and which player-facing command surfaces should be watched in playtests.

## Done Criteria

- Adaptive cadence is default under the Movement prediction setting for compatible live active
  players.
- Prediction-off fallback is verified.
- Late arrivals, correction distances, and degraded prediction modes are visible in diagnostics.
- All enabled command families have tri-state coverage.
- Docs and context capsules match the implemented contract.
