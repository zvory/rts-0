# Phase 6 - Combat, Fog, and Cross-Player Guardrails

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

## Player-Facing Outcome

Prediction remains fast without becoming misleading or leaky. Players may see smoother local
animations, but authoritative combat/fog outcomes still come only from the server.
