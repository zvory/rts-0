<!-- rts-patch-note:v1 -->
<!-- branch: zvorygin/unified-artillery-fire-prototype -->
# Unified Artillery Fire targeting

_2026-07-20_

## Changes

- Replaced the separate Point Fire and Blanket Fire command-card controls with a single Fire command (hotkey X): click once to choose the target center, then click again to set the fire radius.
- Artillery fire radius is player-selected from 4 to 15 tiles; values are clamped to that range. The targeting preview now displays the chosen area and a guide from its center to the radius cursor, including when targeting through the minimap.
- Artillery shells now scatter within the selected fire area. The former range-based Point Fire error and repeated-shot accuracy tightening have been removed, so the tightest available fire area is a 4-tile radius.
- Removed the Artillery Fire Control/Ballistic Tables research from the Research Complex. It previously cost 300 steel and 200 oil, required Heavy Guns, and tightened artillery fire over repeated shots.
- Queued artillery fire now retains its selected target center and radius in the order queue preview.

## Playtest watch

- Check whether the 4-tile minimum and 15-tile maximum provide useful choices between concentrated and wide-area fire.
- Watch the discoverability and reliability of the two-click gesture, especially cancellation, Shift-queued orders, and minimap targeting.
- Assess the artillery economy and combat impact of removing the accuracy-tightening research and its 300-steel/200-oil resource sink.
