# Network docs

These notes cover transport, packet flow, and stutter investigation work for the browser client and
Rust server.

Start here:

- [Network stutter plan](network-stutter-plan.md)

Phase docs:

- [Phase 00: measure the freeze](phase-00-measurement.md)
- [Phase 01: reliable message priority](phase-01-reliable-message-priority.md)
- [Phase 02: latest-only snapshots](phase-02-latest-only-snapshots.md)
- [Phase 03: interpolation buffer tuning](phase-03-interpolation-buffer.md)
- [Phase 04: entity interpolation cleanup](phase-04-entity-interpolation-cleanup.md)
- [Phase 05: compact/binary WebSocket snapshots](phase-05-websocket-compact-snapshots.md)
