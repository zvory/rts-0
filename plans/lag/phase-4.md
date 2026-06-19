# Phase 4 - Movement Prediction on Effective Ticks

## Phase Status

- [ ] Planned.

## Objective

Rebase existing owned-unit movement prediction onto the effective-tick command cadence. Healthy
clients should see owned movement begin after the two-tick lead, while authoritative snapshots
reconcile by replaying forward rather than causing repeated rubberbanding.

## Scope

- Update the WASM adapter and prediction controller so commands begin locally on their intended or
  accepted effective tick, not immediately on click.
- Reconcile authoritative snapshots by:
  - importing the owner-safe baseline at the authoritative tick
  - dropping commands consumed by authoritative sim ACK
  - replaying unacknowledged or late-corrected commands in effective-tick order
  - advancing prediction to the current display tick
- Keep prediction scoped to owned units for:
  - move
  - attack-move movement
  - stop
  - hold position
  - queued movement stages
- Track correction distance separately for:
  - ordinary authoritative drift
  - late-command correction
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
  - late authoritative application corrects once and converges
  - prediction-disabled path renders only authoritative snapshots
  - queued movement replays in effective-tick order after coalesced snapshots
- Add tri-state profiles for:
  - healthy two-tick lead
  - 5, 10, and 20 tick delayed authoritative snapshots
  - one late command followed by lead increase
  - burst delivery and latest-only snapshot coalescing
  - hidden blocker correction without hidden-state leak
- Run:
  - `node tests/prediction_controller.mjs`
  - `node tests/sim_wasm_smoke.mjs` when WASM assets are present
  - focused movement tri-state scenarios
  - `cargo test --manifest-path server/Cargo.toml -p rts-sim-wasm`
  - `node scripts/check-prediction-guardrails.mjs`

## Manual Testing Focus

Under normal local play, move commands should feel like a tiny stable delay instead of immediate
then corrected motion. Under artificial latency, one-off late commands may correct, but repeated
rubberbanding should quickly turn into a higher stable lead.

## Handoff Expectations

The handoff must include measured correction distances from the movement scenarios, the default
lead used, late-command behavior observed, and any movement cases intentionally left
authoritative-only.
