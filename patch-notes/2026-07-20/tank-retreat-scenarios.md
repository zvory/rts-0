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
- Compact formations retain their original top/left ordering: infantry occupies adjacent slots,
  while selections containing vehicles leave one open tile between slots.
- The reverse-traffic inspection scenario now issues one grouped move order, letting the normal
  formation planner assign the Tanks' destinations as it would for a player-issued group command.
- Unit stats, economy, weapon damage, armor multipliers, and player controls are unchanged.

## Playtest watch

- Watch dense reversing groups for over-cautious yielding or deadlocks at merging paths.
- Watch the transition back to ordinary forward movement after incoming fire stops, especially on
  curved retreat routes.
