<!-- rts-patch-note:v1 -->
<!-- branch: zvorygin/tank-close-move-investigation -->
# Tank close-move precision

_2026-08-02_

## Changes

- Single-unit move commands preserve the exact clicked point as the unit's body-center goal instead of snapping through formation tile centers.
- Same-tile vehicle moves keep moving toward visible close nudges instead of immediately treating the command as arrived.
- Close tank nudges now wait for hull-axis alignment and move forward or reverse along the body direction to reduce sideways sliding.

## Review notes

- Needs human playtest review before merge because tank retargeting and close movement feel changed noticeably.
- Full long-distance hull-axis locomotion refactor is deferred to the local TODO list.
