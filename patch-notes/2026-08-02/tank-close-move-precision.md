<!-- rts-patch-note:v1 -->
<!-- branch: zvorygin/tank-close-move-investigation -->
# Vehicle close-move precision

_2026-08-02_

## Changes

- Single selected vehicles now use the exact clicked point when that vehicle pose and direct sweep are legal.
- Close same-tile vehicle nudges no longer auto-arrive while the vehicle center is visibly short of the click.
- Tanks reduce close-range sideways sliding by waiting for hull-axis alignment, then moving forward or reverse along the body direction.

## Review notes

- Vehicle movement feel needs human playtest review before merge.
- Infantry movement logic is unchanged.
