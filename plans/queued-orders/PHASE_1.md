# Phase 1 - Queued Move and Attack-Move

Goal: make Shift-clicked movement paths work for selected units.

## Scope

- Client input:
  - Terrain right-click with Shift appends a move intent instead of replacing.
  - Shift right-click on enemies or resources remains replacement behavior until later phases add
    queued attack and gather.
  - Command-card move/attack-move targeting honors Shift.
  - Normal right-click and command-card targeting keep replacing active and queued orders.
- Server command application:
  - `Move { queued: true }` appends a move intent to each valid selected unit.
  - `AttackMove { queued: true }` appends an attack-move intent to each valid selected unit.
  - Non-queued move/attack-move keeps using the existing coordinator replacement path.
- Promotion:
  - Plain `Move` promotes the next queued intent when movement arrival or tolerant arrival marks it
    complete.
  - `AttackMove` promotes only after reaching its final movement destination.
  - Enemy engagements during attack-move do not consume the queued intent.
- Formation:
  - Recalculate group destination spreading at promotion time.
  - Dead or missing units are ignored when the next group move is promoted.

## Design Notes

The first version can store a logical group id or command sequence id if needed, but it should not
require synchronized arrival for the whole group. Each unit can promote independently when its
current order completes. That keeps stuck or dead units from blocking the rest of the selection.

Attack-move completion needs special care because current combat can temporarily chase targets and
then resume the movement goal. Promotion should key off the original attack-move destination being
reached, not target acquisition or target death.

## Tests

- A unit follows two queued move points in order.
- Normal move issued after queued moves clears the queue and goes directly to the new destination.
- `Stop` during a queued path prevents later queued destinations from running.
- Attack-move resumes its destination after engaging an enemy and only promotes after arrival.
- Tolerant arrival on a queued move still promotes the next queued move.
- Queue cap of 8 is enforced per unit.

## Done

- Shift-clicking multiple movement points produces deterministic server-side behavior.
- Non-Shift movement behavior is unchanged.
- Basic replay of queued move/attack-move is deterministic.
