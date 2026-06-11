# Phase 4 - Owned Unit Movement Prediction

## Objective

Use the WASM predictor for the first player-visible lag fix: selected owned units should begin
responding to movement orders immediately, before the remote server snapshot returns.

## Prediction Scope

Enable prediction for:

- owned unit move orders
- owned attack-move movement pathing, but not damage or kill outcomes
- owned stop commands
- queued owned movement stages
- local order markers and accepted path visuals

Do not predict yet:

- enemy movement
- hidden enemy reveal
- combat damage/deaths
- resource income
- production completion
- construction completion
- fog expansion from predicted movement unless explicitly safe and visually marked as predicted

## Client Work

- Wire `PredictionController` to the WASM adapter.
- On local command issue:
  - allocate `clientSeq`
  - enqueue into WASM immediately
  - send to remote server
  - render predicted owned-unit movement from the predicted snapshot
- On authoritative snapshot:
  - import authoritative baseline
  - drop acknowledged commands
  - replay unacknowledged commands
  - compare authoritative vs predicted owned entity positions
  - smooth small corrections over a short visual window
  - snap large corrections with metrics recorded
- Treat prediction divergence caused by hidden enemies, unseen blockers, server-side combat, or
  coalesced snapshots as a normal reconciliation path. The predictor may move owned units
  immediately, but authoritative snapshots must be able to correct positions without revealing why
  the correction happened.
- Add a visible or logged developer-only prediction status readout.

## Server Work

- Add owner-safe baseline fields if phase 3 proved ordinary snapshots are insufficient.
- Keep authoritative snapshots unchanged for non-prediction clients.
- Ensure server rejection notices are preserved and delivered even when command prediction was
  locally optimistic.

## Verification

- Native reconciliation test:
  - simulate client issuing move at local tick N
  - delay authoritative snapshot by 5, 10, and 20 ticks
  - assert predicted position advances immediately
  - assert reconciliation converges after server acknowledgement
- WASM parity test for the same movement command streams.
- Node reconciliation tests for dropped/coalesced snapshots.
- Reconciliation test where the authoritative server path differs because of hidden or
  non-baselined state; assert the client converges, records a bounded correction, and does not
  expose hidden ids, positions, or target data in diagnostics.
- Browser smoke test that clicks a move command and asserts owned unit render position changes
  before any mocked authoritative echo is delivered.
- Regression test that spectators and replay viewers never receive predicted control.
- Correction budget test:
  - record max correction distance
  - fail if simple movement corrections exceed a documented threshold under deterministic test
    conditions.

## Manual Testing Focus

Run a match under artificial latency and issue basic owned-unit move and queued move commands. The
manual check is that owned units begin moving immediately, corrections are visible only when needed,
and disabling prediction restores the old authoritative-only response.

## Handoff Expectations

At handoff, report correction-distance measurements from the movement scenarios, the prediction flag
used for manual testing, and any movement cases intentionally left authoritative-only. Note whether
Phase 5 can build on the same pending-command lifecycle or needs additional command result metadata.

## Player-Facing Outcome

Move commands feel immediate for owned units. The server still decides the real result, and
corrections are smoothed when prediction differs.
