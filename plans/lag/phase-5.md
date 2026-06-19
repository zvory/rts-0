# Phase 5 - Movement Prediction on Effective Ticks

## Phase Status

- [ ] Planned.

## Objective

Rebase existing owned-unit movement prediction onto the effective-tick command cadence and bounded
server rollback. Healthy clients should see owned movement begin after the two-tick lead, while
late commands inside the rollback window converge through server replay instead of repeated
rubberbanding.

## Scope

- Update the WASM adapter and prediction controller so commands begin locally on their intended or
  accepted effective tick, not immediately on click.
- Reconcile authoritative snapshots by:
  - importing the owner-safe baseline at the authoritative tick
  - dropping commands consumed by authoritative sim ACK
  - replaying unacknowledged, rolled-back, or fallback-late commands in effective-tick order
  - advancing prediction to the current display tick
- Keep prediction scoped to owned units for:
  - move
  - attack-move movement
  - stop
  - hold position
  - queued movement stages
- Track correction distance separately for:
  - ordinary authoritative drift
  - server rollback correction
  - outside-window late fallback correction
  - hidden blocker/path divergence
- Keep Movement prediction setting as the gate.

## Expected Touch Points

- `client/src/prediction_controller.js`
- `client/src/sim_wasm_adapter.js`
- `client/src/state.js`
- `server/crates/sim-wasm/src/lib.rs`
- `tests/prediction_controller.mjs`
- `tests/sim_wasm_smoke.mjs`
- `tests/tri_state/scenarios/move_*`
- `tests/tri_state/scenarios/stop_corrects_predicted_motion.mjs`
- `tests/tri_state/scenarios/hidden_blocker_correction_no_leak.mjs`

## Verification

- Add or update unit tests for:
  - local movement does not start before effective tick
  - movement starts on two-tick cadence when enabled
  - rolled-back authoritative application converges without repeated snapback
  - outside-window late fallback corrects once and raises future lead
  - prediction-disabled path renders only authoritative snapshots
  - queued movement replays in effective-tick order after rollback and coalesced snapshots
- Add tri-state profiles for:
  - healthy two-tick lead
  - 5, 10, 20, and 26 tick delayed authoritative command delivery
  - one outside-window late command followed by lead increase
  - burst delivery and latest-only snapshot coalescing
  - hidden blocker correction without hidden-state leak
- Run:
  - `node tests/prediction_controller.mjs`
  - `node tests/sim_wasm_smoke.mjs` when WASM assets are present
  - focused movement/rollback tri-state scenarios
  - `cargo test --manifest-path server/Cargo.toml -p rts-sim-wasm`
  - `node scripts/check-prediction-guardrails.mjs`

## Manual Testing Focus

Under normal local play, move commands should feel like a tiny stable delay. Under artificial
latency within the rollback window, commands should still settle as if they happened on the
intended tick; beyond the window, one-off correction is acceptable and should increase future lead.

## Handoff Expectations

The handoff must include measured correction distances from the movement scenarios, rollback-window
behavior observed, outside-window fallback behavior, the default lead used, and any movement cases
intentionally left authoritative-only.
