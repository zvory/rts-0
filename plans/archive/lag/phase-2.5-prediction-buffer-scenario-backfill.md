# Phase 2.5 - Prediction Buffer Scenario Backfill

Status: Done.

## Objective

Backfill artifact-backed scenario coverage for the command sequencing and prediction-buffer work
that already exists. The goal is to make ACK handling, pending command drops, stale snapshots, and
client prediction diagnostics visible in two-lane remote/client artifacts before relying on them for
broader prediction.

## Scope

This phase builds on the Phase 0.5 runner. It should not require a local WASM lane. The scenarios
must run with the local lane unavailable and still explain the command lifecycle clearly from the
remote authoritative lane and browser client lane.

## Harness Additions

- Capture every locally issued `clientSeq`, command kind, issue step, and latest known
  authoritative tick.
- Capture each snapshot's `netStatus.predictionVersion`,
  `netStatus.lastSimConsumedClientSeq`, and `netStatus.lastSimConsumedClientTick`.
- Capture `PredictionController.debugSummary()` from `window.__rtsPredictionDebug.controller`.
- Add timeline assertions for pending command counts, acknowledged counts, stale snapshot counts,
  duplicate snapshot counts, skipped tick counts, rejection counts, and timeout counts.
- Add a remote-lane command burst helper that can issue several commands in deterministic order.
- Add a latest-only snapshot helper that can intentionally observe skipped authoritative ticks
  without treating them as a failure.

## Required Scenarios

- `client_seq_monotonic_all_paths`: issue representative command families through the browser
  command path and assert strictly increasing `clientSeq` values.
- `ack_drops_consumed_pending_commands`: issue commands 1, 2, and 3, wait for a snapshot
  acknowledging 1, and assert only 2 and 3 remain pending.
- `ack_three_leaves_four_five_pending`: issue five commands, observe ACK 3, and assert 4 and 5
  remain pending.
- `socket_receipt_not_reconciliation_ack`: if a receipt diagnostic is available, prove receipt does
  not drop pending commands until sim-consumption ACK appears.
- `duplicate_and_skipped_snapshots_are_diagnostic`: observe duplicate or skipped authoritative
  ticks and assert the prediction controller records diagnostics without corrupting pending state.
- `stale_snapshot_ignored`: inject or replay an older snapshot through a test hook and assert it
  cannot apply a newer ACK or rewind prediction-controller state.
- `rejection_notice_does_not_imply_ack`: trigger an invalid command, capture the local notice or
  rejection diagnostic, and assert pending state is cleared only by sim-consumption ACK or timeout.

## Test Hooks

Prefer harness-level transport controls before adding production hooks. If a stale or duplicate
snapshot cannot be produced through normal live transport, add a dev/test-only browser helper that
feeds a recorded snapshot into `PredictionController.applyAuthoritativeSnapshot` without touching
authoritative `GameState`.

Do not add server authority bypasses, full-world state endpoints, or client-controlled ACKs.

## Verification

- Run Phase 0.5 foundation scenarios.
- Run all Phase 2.5 scenarios against a live server and headless browser.
- Keep `node tests/prediction_controller.mjs` as focused unit coverage; do not replace it with the
  slower scenario suite.
- Add scenario output summaries that show issued sequences, pending sequences, latest ACK sequence,
  and first mismatch.

## Manual Testing Focus

Open one ACK scenario artifact and verify it reads like a protocol timeline: command issued,
command pending, snapshot arrives, ACK advances, pending drops. The artifact should make it obvious
that socket receipt and sim-consumption acknowledgement are different concepts.

## Handoff Expectations

At handoff, name any command paths that still lack scenario coverage and any ACK lifecycle behavior
that required a test-only hook. Phase 3.5 should be able to reuse these command timelines without
changing scenario definitions.

## Player-Facing Outcome

No gameplay change. This phase makes the existing prediction buffer explainable under real browser
and WebSocket conditions.
