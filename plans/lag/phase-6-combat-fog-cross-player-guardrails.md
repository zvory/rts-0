# Phase 6 - Combat, Fog, and Cross-Player Guardrails

Status: Done. Combat remains authoritative-only; Phase 6 adds guardrails and negative coverage
without expanding prediction into enemy state, fog reconstruction, damage, deaths, or combat
outcomes.

## Objective

Prevent prediction from creating desync-prone or unfair information paths before considering any
broader simulation prediction. This phase is mainly hardening and negative testing.

## Guardrails

- Hidden enemy state must never enter the browser prediction baseline.
- Predicted fog must not reveal hidden enemies or imply hidden enemy absence.
- Predicted combat must not create authoritative-looking kills, damage, or target reveals unless
  confirmed by the server.
- Enemy entities from authoritative snapshots remain projections, not local simulation truth.
- Client-predicted state must never be serialized back to the server except as ordinary player
  commands.

## Possible Limited Expansion

After guardrails pass, consider narrowly predicting:

- owned weapon facing and local wind-up animation
- local muzzle/ability launch anticipation for commands that are already valid locally
- client-only animation easing for expected movement/combat posture

Do not predict actual enemy HP, death, resource denial, or win/loss state.

## Verification

Use the following fog, desync, and security checks as the required verification surface for this
phase.

## Fog Verification

- Add tests that construct a map with hidden enemies just outside visibility.
- Export prediction baselines for the owning player.
- Assert hidden entity ids, positions, kinds, orders, and target ids are absent.
- Run the same test for:
  - live fog
  - lingering death vision
  - smoke-obscured visibility
  - spectator snapshots
  - replay viewer snapshots

## Desync Verification

- Native-vs-WASM parity tests for every prediction-enabled system.
- Fuzz command streams with random valid and invalid local commands.
- Simulate remote snapshots with:
  - 100 ms latency
  - 250 ms latency
  - 500 ms latency
  - burst delivery
  - latest-only coalescing
- Assert correction converges and pending command buffers do not grow without bound.
- Add checksums for owner-visible predicted state so mismatch rates can be tracked in tests.

## Security Verification

- Static architecture check that browser prediction code cannot import server-only replay,
  full-world snapshot, AI, match-history, SQL, or dev-watch full-vision helpers.
- Regression test that normal clients cannot request full-world baselines.
- Regression test that command metadata cannot be forged to mark commands accepted or skip server
  validation.

## Manual Testing Focus

Play or replay a fog-heavy encounter with prediction enabled and confirm hidden enemies remain
hidden until the authoritative reveal. Manual review should focus on corrections, attack feedback,
and spectator/replay behavior rather than broad balance or combat tuning.

## Handoff Expectations

At handoff, state which combat and fog behaviors are predicted, which remain authoritative-only, and
the negative fog-leak scenarios that passed. Call out any residual desync or correction cases that
Phase 7 must keep behind the rollout flag.

## Player-Facing Outcome

Prediction remains fast without becoming misleading or leaky. Players may see smoother local
animations, but authoritative combat/fog outcomes still come only from the server.

## Implementation Notes

- `rts-sim-wasm` still predicts only owned movement/order state. Combat, fog reconstruction,
  hidden/enemy authoritative state, economy gathering, production, construction, resource node
  state, and abilities remain explicitly unsupported in diagnostics.
- Attack commands are accepted into the local pending-command stream for reconciliation tracking
  but only record `commandUnsupported`; they do not change predicted positions, HP, events, kills,
  or render snapshots.
- Local render snapshots emitted by the WASM predictor contain owned entities only. Visible enemies
  imported from an authoritative snapshot remain anonymous obstacle data and are not serialized into
  predicted render state.
- Added `scripts/check-prediction-guardrails.mjs` and wired it into the architecture gate to keep
  prediction-facing JS/WASM code away from replay/full-world/dev-watch imports and server-only
  dependencies such as AI, SQL, Axum, Tokio, and the server shell.
- Added live regression coverage proving forged full-world prediction-baseline requests produce no
  baseline/full-world payload and forged command metadata does not bypass server command validation.
- Added Phase 6 tri-state coverage for combat staying authoritative-only and reused the spectator
  no-prediction scenario in the Phase 6 group.

## Verification Notes

- Passed: `node scripts/check-prediction-guardrails.mjs`
- Passed: `node scripts/check-crate-boundaries.mjs`
- Passed: `node tests/select-suites.mjs --verify`
- Passed: `node tests/prediction_controller.mjs`
- Passed: `node tests/tri_state/self_test.mjs`
- Passed: `cargo fmt --manifest-path server/Cargo.toml --check`
- Passed: `cargo test --manifest-path server/Cargo.toml -p rts-sim-wasm`
- Generated WASM assets with `scripts/build-sim-wasm.sh`
- Passed against local server on `127.0.0.1:8098`: `RTS_WS=ws://127.0.0.1:8098/ws node
  tests/regression.mjs`
- Passed against local server on `127.0.0.1:8098`: `RTS_URL=http://127.0.0.1:8098/
  RTS_WS=ws://127.0.0.1:8098/ws node tests/tri_state/run.mjs --scenario phase-6`
