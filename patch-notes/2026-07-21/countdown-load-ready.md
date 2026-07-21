<!-- rts-patch-note:v1 -->
<!-- branch: zvorygin/countdown-load-ready -->
# More reliable match starts

_2026-07-21_

## Changes

- Multiplayer matches now start only after every active player finishes loading; failed loads return everyone to the lobby.
- Match rendering warms up during the ready check for smoother starts.

## Playtest watch

- Slow or failed multiplayer clients return cleanly to the lobby and can retry.
