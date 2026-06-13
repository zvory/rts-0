# Phase 2 - Client Prediction Buffer and Reconciliation Skeleton

Status: Done ahead of harness backfill. The browser prediction controller exists with focused JS
coverage, but Phase 2.5 must expose the same lifecycle in scenario artifacts.

## Objective

Build the browser-side prediction manager without running WASM yet. It should buffer local commands,
consume server acknowledgements, model reconciliation state transitions, and expose testable hooks
for delayed authoritative snapshots.

## Client Architecture Work

- Add a `PredictionController` or equivalent collaborator wired from `Match`.
- Keep it separate from `Net`, `Input`, and `Renderer`.
- Route every live gameplay command through a single controller method, such as
  `PredictionController.issueCommand(cmd)`, before `Net.command(cmd)` sends it. Do not let input,
  minimap, HUD, placement, or hotkey paths allocate or attach `clientSeq` independently.
- Responsibilities:
  - allocate or receive command sequence ids
  - record pending local commands with sequence, local issue time, and latest known server tick
  - receive authoritative snapshots and sim-consumption acknowledgement metadata
  - drop acknowledged commands
  - optionally record socket/room receipt diagnostics when available, without using them for
    reconciliation
  - expose pending command count and correction metrics for tests/debug UI
  - provide a single future seam where a WASM predictor can be plugged in
- Keep `GameState.applySnapshot` authoritative by default. The prediction controller may later feed
  predicted render snapshots into `GameState`, but this phase should not alter visible behavior.
- Keep replay viewers, spectators, dev-watch passive viewers, and any non-player role on the
  existing direct authoritative path with no command sequence allocation.

## Reconciliation Semantics

- Define states:
  - `disabled`: normal current behavior
  - `tracking`: command buffer and acknowledgements active, no predicted state
  - `predicting`: later phases produce predicted snapshots
  - `resyncing`: authoritative correction is being applied
- Define stale snapshot handling. Snapshots older than the latest applied authoritative tick must
  be ignored for prediction state unless they are part of an explicit replay/resync test.
- Define latest-only snapshot compatibility. The server may coalesce snapshots, so reconciliation
  must not require every tick's snapshot to arrive.
- Define command timeout handling for lost/disconnected cases without inventing client authority.

## Test Harness Work

- Extend the Phase 0 tri-state scenario harness with prediction-buffer state summaries. The local
  lane may still be unavailable in this phase, but remote/client artifacts must show issued
  command sequences, pending command counts, latest authoritative snapshot tick, and acknowledgement
  handling after each scripted step.
- Add a pure Node test harness for prediction buffer behavior.
- Feed scripted sequences:
  - command 1, 2, 3 issued; snapshot acknowledges 1
  - snapshot acknowledges 3 while 4 and 5 remain pending
  - socket/room receipt arrives for command 4 but sim-consumption acknowledgement has not, so
    command 4 stays pending
  - duplicate snapshots
  - skipped authoritative ticks
  - out-of-date snapshots after a newer snapshot has already been applied
  - command rejection notices mixed with acknowledged commands

## Verification

- At least one tri-state scenario runs in two-lane mode and records prediction-buffer diagnostics
  in the browser client lane.
- Node unit tests for all reconciliation state transitions.
- Node or browser unit test that exercises representative command sources through the single issue
  path: viewport right-click, minimap right-click, HUD stop/train/research/cancel, build
  placement, and rally commands.
- Node tests for latest-only snapshot behavior matching `ConnectionSink` semantics.
- Client architecture check updated if a new module area is introduced.
- Client smoke test still passes with prediction tracking enabled but no visible prediction.
- Test that replay viewer and spectator modes never allocate gameplay command sequence ids.

## Manual Testing Focus

Play a short local match with prediction disabled and confirm visible behavior matches the
pre-prediction authoritative flow. Use debug output or scenario artifacts to inspect pending-command
counts, acknowledgement drops, and correction diagnostics while issuing several rapid commands.

## Handoff Expectations

At handoff, describe the prediction-controller state machine, the diagnostics exposed to the
tri-state harness, and any disabled code paths that Phase 3 or Phase 4 must activate. Include the
manual debug view or artifact fields a future agent should inspect when reconciliation behaves
unexpectedly.

## Player-Facing Outcome

No intended gameplay change. This phase adds the local bookkeeping needed to safely run prediction
later.
