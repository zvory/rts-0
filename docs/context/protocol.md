# Capsule: wire protocol

Use when adding, removing, or changing any field on a client↔server message, snapshot, or event.

## Read first in `DESIGN.md`
- §2 Wire protocol (JSON over WebSocket) — full section
  - §2.1 `ClientMessage`
  - §2.2 `ServerMessage`
  - §2.3 `start` payload
  - §2.4 `snapshot` payload (per-player, fog-filtered)
  - §2.5 `Event` (transient, one snapshot only)

## Code map
- `server/src/protocol.rs` — authoritative
- `client/src/protocol.js` — mirror; must agree on every tag, field name, and shape

## Invariants
- **Mirror.** Every protocol change touches both files **and** `DESIGN.md §2` in the same commit.
- **Fog is authoritative.** Anything sent per-player (entity views, `target_id` tracers, death/
  positional events) must be gated on visibility/ownership. Never send a player an entity or
  position they can't see. See `DESIGN.md §2.4` and §7.
- **Clients are untrusted.** Validate and bound everything inbound: dedupe + cap unit lists
  (`MAX_UNITS_PER_COMMAND`), size-limit frames, range/overflow-check placement coords. See
  [deployment.md](deployment.md) and `DESIGN.md §7`.

## Cross-capsule triggers
- Adding a snapshot field consumed by rendering → [client-ui.md](client-ui.md).
- Changing event emission from the sim → [server-sim.md](server-sim.md).
