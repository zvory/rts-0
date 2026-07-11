# Phase 9 - Rollout, Tuning, and Regression Matrix

## Phase Status

- [ ] Planned.

## Objective

Make hybrid command cadence and bounded rollback the default behavior under the existing Movement
prediction setting after correctness gates and catch-up diagnostics are in place. This phase locks in tuning,
documentation, and regression coverage so future gameplay changes cannot quietly reintroduce
remote-echo command feel.

## Scope

- Finalize tuning:
  - default `commandLeadTicks = 2`
  - `ROLLBACK_WINDOW_TICKS = 6`
  - maximum normal lead
  - late-arrival threshold for increasing lead
  - stable-window threshold for decay
  - `MAX_REPLAY_COMMANDS = 1000` replay fuse and fallback reason
  - clamped rollback enablement and command-family clamp-safe rules
  - slow catch-up logging thresholds for later optimization
  - maximum tolerated metronome delay and authoritative snapshot gap before follow-up action
  - correction distance budgets
  - worker/perf degradation thresholds
- Make the cadence and rollback path default for compatible live active-player sessions when
  Movement prediction is enabled.
- Keep all tuning values centralized and documented with ownership:
  - protocol constants and compact versions in the protocol crate/mirror
  - server scheduling, rollback window, future-tick bounds, replay cursor policy, clamped fallback,
    and fallback thresholds in one live-room scheduling module
  - client lead defaults and degraded prediction modes in the prediction path
  - operator-facing thresholds in `docs/perf-tracing.md`
- Keep spectators, replay viewers, unsupported factions, incompatible builds, and prediction-off
  sessions out of cadence prediction.
- Do not roll out by silent partial behavior. If a command family remains authoritative-only, the
  regression matrix must say so and the UI/debug surfaces must not imply local world response for
  that family.
- Update docs and operator guidance:
  - `docs/design/protocol.md`
  - `docs/design/server-sim.md`
  - `docs/design/client-ui.md`
  - `docs/perf-tracing.md`
  - any relevant context capsules if section lists shift
- Add a concise regression matrix that maps each command family to:
  - predicted owned-world response
  - exact rollback behavior
  - clamped rollback behavior
  - authoritative-only side effects
  - required tests
  - known unsupported cases
- Add an explicit rollout decision record that names:
  - whether rollback is enabled for human-only rooms, AI-backed rooms, branch-live rooms, and lab
    rooms
  - whether the final mode is full visual prediction, reduced prediction, accepted-intent overlay,
    or authoritative-only for each command family
  - whether each command family is clamp-safe, exact-rollback-only, or live-fallback-only
  - what evidence would trigger reverting to a higher lead, smaller rollback window, or
    authoritative-only mode

## Verification

- Run all focused prediction, cadence, rollback, and perf suites:
  - `node tests/prediction_controller.mjs`
  - `node tests/sim_wasm_smoke.mjs` when WASM assets are present
  - `node tests/tri_state/self_test.mjs`
  - all cadence/prediction/rollback tri-state scenario groups
  - `node scripts/check-prediction-guardrails.mjs`
  - `node scripts/check-client-architecture.mjs`
  - `node tests/protocol_parity.mjs`
  - focused Rust room/protocol/sim-wasm tests
  - server rollback/catch-up diagnostic command from Phase 8
- Run a browser perf harness covering:
  - healthy local profile
  - 100 ms RTT with jitter
  - 250 ms RTT with burst delivery
  - 6-tick rollback command delivery
  - outside-window clamped rollback command delivery
  - command delivery behind an active replay cursor
  - synthetic slow catch-up that creates a measurable snapshot gap
  - weaker client or CPU-throttled profile
- Verify the regression matrix mechanically where practical: add or update a small checker or test
  fixture that fails when a command family is marked predicted without matching tri-state coverage
  and listed authoritative-only side effects.
- If practical, run one narrow live Node integration or smoke path with Movement prediction enabled
  and one with it disabled.
- Rely on the PR `./tests/run-all.sh` gate for full-suite coverage.

## Manual Testing Focus

Run short online-like matches with Movement prediction on and off. Confirm normal healthy play has
a small stable command delay, bad conditions inside the rollback window still honor the intended
command tick, outside-window clamp-safe commands use the oldest safe replayable tick, unsupported
outside-window commands degrade to late execution and lead adjustment, slow catch-up produces a
bounded authoritative snapshot gap while local owned prediction continues, and turning Movement
prediction off returns to the old authoritative-only behavior.

## Handoff Expectations

The handoff must state the final rollout status, tuning values, rollback window, verification
commands and results, known caveats, the regression matrix location, the rollback support matrix,
and which player-facing command surfaces should be watched in playtests.

## Done Criteria

- Hybrid cadence plus bounded rollback is default under the Movement prediction setting for
  compatible live active players.
- Prediction-off fallback is verified.
- Rollback, late fallback, correction distances, and degraded prediction modes are visible in
  diagnostics.
- Clamped rollback, metronome delay, and authoritative snapshot gaps are visible in diagnostics.
- All enabled command families have tri-state coverage.
- Every command family has an explicit matrix row saying whether it is predicted, intent-only,
  exact-rollback-supported, clamped-rollback-supported, or authoritative-only.
- Docs and context capsules match the implemented contract.
