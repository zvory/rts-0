# Phase 5 - Multi-Stage Rallies

Goal: let production buildings send newly produced units through up to two rally stages.

## Scope

- Building-side rally queues:
  - Cap at 2 stages per building.
  - Store rally stages as point move intents.
  - First stage continues to influence spawn exit selection.
  - Produced units receive a copy of the building's current rally stages at spawn time.
- Input:
  - Normal rally command replaces the rally queue with one stage.
  - Shift rally appends a second stage if space is available.
  - Multi-building rally editing is deferred; first implementation can operate cleanly on one
    selected producer or apply the same simple replacement to all selected producers.
- Snapshot/projection:
  - Owner-only rally marker data includes both rally stages.
  - Existing single rally marker remains compatible when only one stage exists.
- Production:
  - Spawned unit receives a plain move to stage 1 and queued move to stage 2.
  - Later changes to the building rally do not mutate already-spawned units.

## Design Notes

Do not add rally attack-move or target attack in this phase. Point movement rallies are enough to
support staged exits and base routing. Combat rallies can be considered after mixed queued attacks
are stable.

The building queue cap is intentionally smaller than the unit queue cap. Rally paths are strategic
defaults, not a replacement for direct unit control.

## Tests

- Normal rally stores one stage and replaces any previous stages.
- Shift rally appends a second stage and ignores a third.
- Spawn exit prefers the first rally stage.
- Produced unit receives stage 1 as active move and stage 2 as queued move.
- Editing a building rally after a unit spawns does not alter that unit's copied queue.
- Rally markers remain owner-only.

## Done

- A producer can stage new units through a two-point route.
- Existing one-point rally behavior remains compatible.

