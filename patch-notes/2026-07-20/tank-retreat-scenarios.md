<!-- rts-patch-note:v1 -->
<!-- branch: zvorygin/tank-retreat-scenarios -->
# Tanks reverse away from anti-armor fire

_2026-07-20_

## Changes

- Tanks under recent direct anti-armor fire now choose forward or reverse movement based on which keeps their front armor closer to the incoming-fire source; this also applies to long retreat routes behind the threat.
- Repeated qualifying hits now refresh the three-second hull-facing preference while retaining the first attacker as the facing source for the current under-fire window.
- Reversing Tank traffic now senses travel in the rearward direction, causing the trailing reversing vehicle to yield to the vehicle ahead.
- Group move and attack-move orders now use the same compact destination layout at every command distance instead of increasingly preserving the selection’s original world-space separation on longer moves.
- Wide or tall selections now fold into compact, ordered near-square formations. Infantry uses adjacent tiles, while formations containing vehicles leave one open tile between destination slots.
- Vehicle groups of six or more use a slightly wider compact layout, with shorter rows centered.
- Infantry trench preference no longer overrides the spacing reserved around vehicle destination slots.
- Unit stats, costs, economy, weapon damage, armor multipliers, and available player controls are unchanged.

## Playtest watch

- Watch dense reversing vehicle groups for over-cautious yielding or deadlocks where paths merge.
- Watch Tanks returning to ordinary forward movement after the three-second incoming-fire preference expires, especially along curved retreat routes.
- Watch compact mixed-unit formations around trenches, chokepoints, and blocked destination tiles.
