# Phase 8 - Safe Production and Research Progress Extrapolation

Status: Implemented.

## Objective

Keep already-confirmed unit production and research progress bars moving locally during short
snapshot gaps, without making the client authoritative for queue acceptance, resources, completion,
spawns, upgrades, or notices.

This phase is deliberately narrower than general production prediction. It only extrapolates an
active timer after the server has already shown that timer in an authoritative snapshot.

## Prediction Scope

Enable local progress extrapolation only when all of these are true:

- the entity is an owned, visible, completed production or research building
- the latest authoritative snapshot shows an active item with `prodQueue > 0`
- the active item identity is known from `prodKind` or `prodUpgrade`
- `prodProgress` is a finite value in `[0, 1)`
- the client can resolve the authoritative duration from mirrored rules/config
- the extrapolated value remains below completion

The client may render:

- a locally advanced selected-building production/research bar
- a locally advanced world-space production bar on the building
- a subtle predicted/debug marker when developer prediction diagnostics are enabled

The client must not locally:

- create a new queue item
- deduct steel or oil
- increase or decrease supply
- spawn a unit
- apply a completed upgrade
- emit production completion notices
- advance progress past a conservative cap such as `0.98`
- continue extrapolating after the item identity, queue depth, owner, or building state changes

## Safety Model

Production/research progress is safe enough to extrapolate because the server has already accepted
the queue item and exposed the active timer. The only player-facing correction should be a small
progress-bar adjustment when the next authoritative snapshot arrives.

If an authoritative snapshot shows cancellation, pause, queue change, building death, ownership
change, or completion, the server state wins immediately. Completion remains authoritative-only, so
the local bar can approach completion but never finish the item before the server snapshot does.

Steel and oil income are explicitly out of scope for this phase. They depend on worker state,
dropoff timing, resource depletion, pathing, deaths, blocking, and server receipt of recent
commands, so they should remain snapshot-driven until a separate economy-specific design proves a
safe subset.

## Client Work

- Add a small progress extrapolator in the client model layer, preferably near `GameState` rather
  than in renderer/HUD code.
- Track the authoritative receive time, active item identity, queue depth, and baseline
  `prodProgress` for each owned active producer/research building.
- Derive predicted display progress from elapsed client time and mirrored item duration.
- Clamp predicted progress below completion and reset on every authoritative snapshot.
- Keep the existing optimistic `train` pending indicator separate from this feature. Pending queue
  optimism means "command likely accepted"; progress extrapolation means "server already started
  this item."
- Ensure `entityById()` and `entitiesInterpolated()` expose the same display progress so HUD and
  renderer agree.
- Add diagnostics counters for active extrapolated bars and correction magnitude if practical.

## Server Work

No gameplay server changes should be required. If the client cannot safely infer item duration from
existing mirrored config, add the smallest owner-only metadata needed and update the protocol
mirror and docs in the same phase.

Do not add server trust in client progress. Do not send full production internals beyond what the
owner already needs to render progress.

## Verification

- Unit tests for the client progress extrapolator:
  - advances an already-started unit production item
  - advances an already-started research item
  - clamps below completion
  - resets to lower authoritative progress on correction
  - stops on queue identity change
  - stops on cancellation / `prodQueue` becoming zero
  - does not affect steel, oil, supply, upgrades, or spawned units
- Tri-state scenarios under delayed and coalesced snapshots:
  - already-started unit production keeps moving locally during a 250-500 ms snapshot gap
  - already-started research keeps moving locally during a 250-500 ms snapshot gap
  - cancellation or completion from the server clears the extrapolated display
- Browser smoke coverage that selects a producing building under artificial latency and asserts the
  visible progress bar advances before the next authoritative echo.

## Manual Testing Focus

Under artificial latency, start unit production and research, wait until the server has shown
progress, then watch the selected-building panel and world-space bar during snapshot gaps. Confirm
the bar keeps moving smoothly, never completes early, and snaps cleanly to cancellation,
completion, or queue changes from the server.

Also confirm steel, oil, supply, produced units, and completed upgrades remain authoritative and do
not move or appear early.

## Handoff Expectations

At handoff, list the item families whose progress bars extrapolate, the exact completion clamp,
and the conditions that disable extrapolation. Include verification commands and results, plus any
manual latency checks performed.

The next agent should only proceed to Phase 8.5 if production/research extrapolation is stable and
the scenario artifacts make correction behavior easy to inspect.

## Player-Facing Outcome

Already-started production and research feel less frozen during brief server jitter, while actual
economy, queue acceptance, completion, and spawned results remain server-authoritative.
