# Phase 2 - Client Prediction Buffer and Reconciliation Skeleton

## Objective

Build the browser-side prediction manager without running WASM yet. It should buffer local commands,
consume server acknowledgements, model reconciliation state transitions, and expose testable hooks
for delayed authoritative snapshots.

## Client Architecture Work

- Add a `PredictionController` or equivalent collaborator wired from `Match`.
- Keep it separate from `Net`, `Input`, and `Renderer`.
- Responsibilities:
  - allocate or receive command sequence ids
  - record pending local commands with sequence, local issue time, and latest known server tick
  - receive authoritative snapshots and acknowledgement metadata
  - drop acknowledged commands
  - expose pending command count and correction metrics for tests/debug UI
  - provide a single future seam where a WASM predictor can be plugged in
- Keep `GameState.applySnapshot` authoritative by default. The prediction controller may later feed
  predicted render snapshots into `GameState`, but this phase should not alter visible behavior.

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

- Add a pure Node test harness for prediction buffer behavior.
- Feed scripted sequences:
  - command 1, 2, 3 issued; snapshot acknowledges 1
  - snapshot acknowledges 3 while 4 and 5 remain pending
  - duplicate snapshots
  - skipped authoritative ticks
  - out-of-date snapshots after a newer snapshot has already been applied
  - command rejection notices mixed with acknowledged commands

## Verification

- Node unit tests for all reconciliation state transitions.
- Node tests for latest-only snapshot behavior matching `ConnectionSink` semantics.
- Client architecture check updated if a new module area is introduced.
- Client smoke test still passes with prediction tracking enabled but no visible prediction.
- Test that replay viewer and spectator modes never allocate gameplay command sequence ids.

## Player-Facing Outcome

No intended gameplay change. This phase adds the local bookkeeping needed to safely run prediction
later.
