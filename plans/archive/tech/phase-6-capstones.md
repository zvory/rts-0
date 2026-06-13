# Phase 6 - Artillery

## Objective

Add the Superior Firepower late-game capstone after the Scout Car, Mortar, Anti-Tank Gun, and Tank timing
foundation is playable.

## Artillery

Working checklist: [artillery-checklist.md](artillery-checklist.md).

- Built at Gun Works.
- Requires a late Gun Works upgrade.
- Costs 300 steel / 100 oil.
- Like Anti-Tank Guns, must set up before firing and tear down before moving.
- Movement and setup behavior matches Anti-Tank Guns, but Artillery is slower and uses a tank-sized
  footprint.
- When setting up, shows a cone indicating where it can shoot, like the Anti-Tank Gun setup cone.
- Field of fire is 20 degrees total after setup.
- Minimum range is 10 tiles.
- Maximum range is 50 tiles.
- Fires every 3 seconds.
- Each shot costs 10 steel, paid only when the shot actually fires.
- Shoots at a point, not a unit.
- First shot after setup has a CEP of 5 tiles.
- Accuracy improves while maintaining fire until the fifth shot after setup has a CEP of 2 tiles.
- On impact, deals 150 armor-piercing damage in a 1-tile radius.
- Outside the 1-tile radius, deals non-armor-piercing splash damage that falls from 150 damage down
  to 10 damage at 3 tiles.
- Deals no splash damage beyond 3 tiles.
- Intended to let Superior Firepower grind down positions and destroy resource bases if it survives
  the Mobile Warfare stage-two surge.

## Work

- Add the Artillery unit definition, Gun Works upgrade, training option, cost, footprint, movement,
  setup, and teardown behavior.
- Reuse the Anti-Tank Gun-style setup command flow and setup cone affordance where possible, adjusted for
  Artillery's 20-degree field of fire.
- Add point-fire command handling and UI affordances for Artillery.
- Track per-setup firing accuracy so the CEP starts at 5 tiles and reaches 2 tiles on the fifth
  shot.
- Spend 10 steel only when a shot fires; rejected orders, out-of-range targets, setup delays, and
  unaffordable shots must not spend steel.
- Apply the armor-piercing inner blast and non-armor-piercing outer falloff separately.
- Ensure Artillery point targeting is validated and cannot panic on invalid coordinates.

## Verification

- Artillery setup, teardown, point fire, accuracy ramp, and range limits are tested.
- Artillery cannot fire inside its 10-tile minimum range, outside its 50-tile maximum range, or
  outside its 20-degree field of fire.
- Artillery spends 10 steel only on shots that actually fire.
- Artillery impact tests cover the 1-tile armor-piercing radius, non-armor-piercing falloff to 3
  tiles, and no damage beyond 3 tiles.
- Fog tests cover Artillery fire and impact events without leaking hidden positions.
- Regression tests cover invalid point targets, unaffordable shots, stale ids, and setup/teardown
  transitions.

## Player-Facing Outcome

Superior Firepower gains a costly, slow, positional siege weapon that can delete economic positions
from extreme range if protected, but must commit to a narrow firing arc, minimum range, setup time,
and ongoing steel ammunition cost.
