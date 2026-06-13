# Capsule: wire protocol

Use when adding, removing, or changing any field on a client↔server message, snapshot, or event.

## Read first
- [docs/design/protocol.md](../design/protocol.md) — full wire protocol
  - §2.1 `ClientMessage`
  - §2.2 `ServerMessage`
  - §2.3 `start` payload
  - §2.4 `snapshot` payload (per-player, fog-filtered)
  - §2.5 `Event` (transient, one snapshot only)
  - §2.6 Replay playback state and vision
  - §2.7 Observer analysis state

## Code map
- `server/crates/protocol/src/lib.rs` — authoritative Rust wire DTOs and compact transport
- `server/src/protocol.rs` — server-shell adapter for typed kind conversion and legacy imports
- `client/src/protocol.js` — mirror; must agree on every tag, field name, and shape

## Invariants
- **Mirror.** Every protocol change touches both files **and**
  [docs/design/protocol.md](../design/protocol.md) in the same commit.
- **Fog is authoritative.** Anything sent per-player (entity views, `target_id` tracers, death/
  positional events) must be gated on visibility/ownership. Never send a player an entity or
  position they can't see. See [docs/design/protocol.md](../design/protocol.md) §2.4 and
  [docs/design/hardening.md](../design/hardening.md).
- **Clients are untrusted.** Validate and bound everything inbound: dedupe + cap unit lists
  (`MAX_UNITS_PER_COMMAND`), size-limit frames, range/overflow-check placement coords. See
  [deployment.md](deployment.md) and [docs/design/hardening.md](../design/hardening.md).
- `rts-protocol` depends only on `rts-contract` among workspace crates. Kind conversion that needs
  rules/sim vocabulary belongs in adapter modules.

## Cross-capsule triggers
- Adding a snapshot field consumed by rendering → [client-ui.md](client-ui.md).
- Changing event emission from the sim → [server-sim.md](server-sim.md).
