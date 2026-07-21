<!-- rts-patch-note:v1 -->
<!-- branch: zvorygin/tank-retreat-scenarios -->
# Tanks reverse away from anti-armor fire

_2026-07-20_

## Changes

- Tanks under recent direct anti-armor fire now choose forward or reverse movement according to
  which direction keeps their front armor closer to the incoming-fire source.
- Retreat orders behind the threat begin in reverse even across long, multi-waypoint routes.
- Repeated qualifying hits keep the three-second facing preference active without redirecting it
  to a different attacker during the same under-fire window.
- Reversing Tank traffic now senses and yields along its actual travel direction.
- Group move and attack-move destinations now form one compact layout at every command distance
  instead of preserving progressively more of the selection's original world-space separation.
- Compact formations now favor near-square blobs instead of inheriting extreme lines from the
  selection's current shape. Wide groups fold into ordered columns, tall groups fold into ordered
  rows, infantry occupies adjacent slots, and selections containing vehicles leave one open tile
  between slots.
- Larger vehicle groups use a slightly wider blob with the deeper cells centered, keeping central
  vehicles ahead of outside vehicles when traffic compresses through a choke.
- Small position differences no longer unpredictably switch between line and blob layouts, and
  infantry trench preference no longer violates an assigned vehicle's one-tile clearance.
- The reverse-traffic inspection scenario now issues one grouped move order, letting the normal
  formation planner assign the Tanks' destinations as it would for a player-issued group command.
- Unit stats, economy, weapon damage, armor multipliers, and player controls are unchanged.

## Playtest watch

- Watch dense reversing groups for over-cautious yielding or deadlocks at merging paths.
- Watch the transition back to ordinary forward movement after incoming fire stops, especially on
  curved retreat routes.
