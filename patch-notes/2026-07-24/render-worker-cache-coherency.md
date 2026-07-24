<!-- rts-patch-note:v1 -->
<!-- branch: zvorygin/render-worker-cache-coherency -->
# Client cache reliability

_2026-07-24_

## Changes

- Fixed live matches failing to render after an update due to stale cached files.

## Playtest watch

- Confirm matches load and render normally in browsers that cached the previous build.
