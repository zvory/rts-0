# Phase 4.5 - Movement Prediction Scenario Backfill

## Objective

Backfill the delayed, dropped, jittered, burst, and coalesced network scenarios that should guard
owned-unit movement prediction. The phase should prove that the current player-visible movement
predictor responds immediately, reconciles to the authoritative server, records bounded
corrections, and does not leak hidden state.

## Scope

This phase covers owned movement, attack-move movement, stop, and queued movement stages only. It
does not expand prediction into combat damage, kills, resource income, construction completion,
production completion, fog expansion, or enemy movement.

## Network Profile Controller

Add deterministic transport profiles to the harness. Prefer a Node WebSocket proxy between the
browser and server so the real client and real server code paths remain unchanged.

Required profile features:

- fixed command latency
- fixed snapshot latency
- jitter with a seeded RNG
- snapshot drop
- snapshot burst delivery
- latest-only snapshot coalescing
- head-of-line style snapshot delay
- temporary disconnect and reconnect if the existing client lifecycle can tolerate it

The profile configuration must be serialized into every artifact so failures are reproducible.

## Required Scenarios

- `move_predicts_before_authoritative_echo`: delay authoritative snapshots, issue an owned move,
  and assert the predicted render position advances while authoritative reads remain unchanged.
- `move_converges_after_ack`: delay snapshots by 5, 10, and 20 ticks, then assert correction
  converges after sim-consumption ACK.
- `coalesced_snapshots_replay_pending`: coalesce several authoritative snapshots and assert
  unacknowledged commands replay against the latest snapshot.
- `dropped_snapshot_does_not_stick_pending`: drop non-final snapshots and assert pending commands
  clear once a later ACK arrives.
- `queued_move_order_stages_survive_replay`: issue queued movement stages under delayed snapshots
  and assert local, client, and remote order summaries converge.
- `stop_corrects_predicted_motion`: issue move, then stop before authoritative echo, and assert
  local prediction stops and remote correction converges.
- `hidden_blocker_correction_no_leak`: create or reuse a scenario where hidden or non-baselined
  state changes the authoritative path, assert correction is recorded, and assert hidden ids,
  positions, target ids, and reason strings are absent from client/local artifacts.
- `prediction_disabled_authoritative_only`: run the same move with prediction off and assert
  commands are sequenced but no predicted render snapshot is used.
- `spectator_replay_no_prediction`: assert spectator, replay, and passive dev-watch clients do not
  allocate gameplay prediction state or render predicted control.

## Correction Budgets

Document initial thresholds in the scenario definitions rather than burying them in harness code.
Start with generous budgets and ratchet them downward only after artifacts show stable behavior.
Every correction assertion should record:

- maximum correction distance
- correction count
- snap correction count
- pending command count at correction time
- latest acknowledged `clientSeq`
- network profile name

## Verification

- Run Phase 0.5, 2.5, and 3.5 scenarios.
- Run all movement prediction scenarios with generated WASM assets available.
- Run the existing JS prediction-controller tests and client smoke prediction check.
- Produce at least one saved success artifact and one forced-failure artifact for manual review.

## Manual Testing Focus

Run an artificial-latency movement scenario visibly or inspect a saved artifact. Confirm owned units
move immediately, authoritative correction is understandable, prediction-off fallback works, and
hidden blocker corrections do not explain hidden enemy state to the client.

## Handoff Expectations

At handoff, report the network profiles implemented, correction budgets observed, and scenarios
that remain flaky or too slow for the default gate. Phase 5 should not expand UI optimism until the
movement scenarios are stable enough to catch regressions.

## Player-Facing Outcome

No new prediction surface. This phase makes the already-enabled owned movement prediction
regression-testable under realistic network trouble.
