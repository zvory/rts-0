<!-- rts-patch-note:v1 -->
<!-- branch: zvorygin/tank-retreat-scenarios -->
# Tanks reverse away from anti-armor fire

_2026-07-20_

## Changes

- Tanks under recent direct anti-armor fire now choose forward or reverse travel based on which direction keeps their front armor closer to the first qualifying incoming-fire source; this behavior also applies to retreat destinations beyond the normal short-range reversing distance.
- Each additional qualifying hit within the reaction window refreshes the three-second hull-facing preference without changing its original facing source.
- Reversing Tank traffic now evaluates vehicles in the rearward travel direction, so a trailing reversing Tank yields to the vehicle ahead instead of the leader yielding.
- Group move and attack-move orders now use the same compact destination layout regardless of command distance, rather than preserving more of the selection’s original world-space separation on longer moves.
- Wide and tall selections now fold into compact, ordered near-square formations. Infantry destination slots are one tile apart; formations containing vehicles use a two-tile pitch, leaving one open tile between slots.
- Formations containing at least six vehicles use a slightly wider compact layout, with shorter rows centered.
- Infantry trench preference no longer overrides the spacing reserved around vehicle destination slots.
- Unit stats, costs, economy, weapon damage, armor multipliers, and available player controls are unchanged.

## Playtest watch

- Watch dense reversing Tank groups for over-cautious yielding or deadlocks where paths merge.
- Watch Tanks returning to ordinary forward movement after the three-second incoming-fire preference expires, especially along curved retreat routes.
- Watch compact mixed-unit formations around trenches, chokepoints, map edges, and blocked destination tiles.
