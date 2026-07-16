# Capsule: wire protocol

Use when adding, removing, or changing any field on a client↔server message, snapshot, or event.

## Read first
- [docs/design/protocol.md](../design/protocol.md) — full wire protocol
  - §2.0 Boundary authority and guardrails
  - §2.1 `ClientMessage`
  - §2.2 `ServerMessage`
  - §2.3 `start` payload
  - §2.4 `snapshot` payload (per-player, fog-filtered)
  - §2.4.1 Boundary inventory
  - §2.5 `Event` (transient, one snapshot only)
  - §2.5.1 Projection contract summary
  - §2.6 Replay playback state and vision
  - §2.7 Observer analysis state
- [docs/projection-audit-checklist.md](../projection-audit-checklist.md) — checklist for new
  projection-affecting snapshot fields, events, or observer modes

## Code map
- `server/crates/protocol/src/lib.rs` — authoritative Rust wire DTOs and compact transport
- `server/crates/contract/src/lib.rs` — shared semantic DTOs re-exported by protocol, including
  start/snapshot contract records and `DEFAULT_FACTION_ID`
- `server/src/protocol.rs` — server-shell adapter for typed kind conversion and legacy imports
- `server/crates/sim/src/protocol.rs` — sim-facing adapter for typed kind conversion
- `client/src/protocol.js` — mirror; must agree on every tag, field name, and shape
- `server/src/map_handoffs.rs` + `client/src/map_editor_handoff.js` — one-use HTTP Map Editor ↔
  Lab map-transfer DTO; map bodies never enter URLs
- `server/src/lobby/mod.rs` + `client/src/config.js` — `PLAYER_PALETTE` cross-surface mirror,
  guarded by `node tests/protocol_parity.mjs`

## Current lobby fields to remember
- `selectMap { map }` is the host-only map selector command.
- `lobby` carries `map` (selected stable map name) and `maps[]`
  (`{name, description, minPlayers, maxPlayers}` catalog rows). Replay start metadata separately
  uses `mapName`.
- Lab start payloads carry `lab` metadata with the public lab id, original operator id, recipient
  role, that recipient's current lab vision mode, optional setup-authored initial camera center,
  dirty flag, and operation count.
- Start payloads carry recipient-scoped `capabilities` metadata for shared room-time,
  vision-selection, and gameplay-command affordances. The client parser must not infer these from
  replay/dev/lab mode names. Lab timeline controls use neutral room-time speed, pause, step,
  relative seek,
  absolute seek, and `roomTimeState` keyframe metadata; they are not `LabClientOp` messages.
- Start payloads carry recipient-scoped `diagnostics` metadata when projection policy enables
  movement-path overlays or observer analysis. Do not infer those affordances from room mode names.
- Resolved AI matches send `observationReady` (replay/log lookup).
- The active protocol has no quickstart/debug lobby command or start-payload flag. Normal live
  countdown skipping for rooms with one or zero active humans is not a debug preset.
- `LobbyPlayer` carries `teamId`, `factionId`, `aiProfileId?` (canonical AI profile id), and
  `isSpectator`; spectators are lobby members but not active match players.
- Lab setup import/export accepts only checkpoint-backed `LabCheckpointScenarioV1`; legacy setup
  JSON is rejected. `validateScenario` previews catalog/path/payload/map bounds without mutating
  the room or accepting client-controlled server paths.
  `metadata.lab.initialCamera` may set the first Lab world-pixel center.
- `/api/map-handoffs` validates authored and materialized flat map data, caps records at 64, expires them after
  two minutes, and consumes each id once. Lab `exportMap` returns only `LabMapDraft` in reverse.

## Invariants
- **Mirror.** Every protocol change touches both files **and**
  [docs/design/protocol.md](../design/protocol.md) in the same commit.
- **Parity.** Run `node tests/protocol_parity.mjs` after protocol vocabulary, compact code/slot,
  prediction metadata, start/snapshot/replay DTO, default faction id, or lobby palette changes.
- **Fog is authoritative.** Anything sent per-player (entity views, `target_id` tracers, death/
  positional events) must be gated on visibility/ownership. Never send a player an entity or
  position they can't see. See [docs/design/protocol.md](../design/protocol.md) §2.4 and
  [docs/design/hardening.md](../design/hardening.md).
- **Clients are untrusted.** Validate and bound everything inbound: dedupe + cap ordinary unit lists
  (`MAX_UNITS_PER_COMMAND`) and lab command-limit-bypass unit lists (`LAB_MAX_UNITS_PER_COMMAND`),
  size-limit frames, range/overflow-check placement coords. See
  [deployment.md](deployment.md) and [docs/design/hardening.md](../design/hardening.md).
- `rts-protocol` depends only on `rts-contract` among workspace crates. Kind conversion that needs
  rules/sim vocabulary belongs in adapter modules.

## Cross-capsule triggers
- Adding a snapshot field consumed by rendering → [client-ui.md](client-ui.md).
- Changing event emission from the sim → [server-sim.md](server-sim.md).
