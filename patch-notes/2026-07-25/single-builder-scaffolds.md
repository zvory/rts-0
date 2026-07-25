<!-- rts-patch-note:v1 -->
<!-- branch: zvorygin/single-builder-scaffolds -->
# Single-worker construction

_2026-07-25_

## Changes

- Construction scaffolds now accept only one active worker; additional build orders are rejected, while unattended scaffolds can still be resumed.
- Queued workers skip occupied scaffolds and continue to their next order.

## Playtest watch

- Concurrent build commands and queued-order continuation around occupied scaffolds.
