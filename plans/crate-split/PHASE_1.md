# Phase 1 - Contract and Protocol Extraction

Status: Done.

Goal: make protocol and semantic message contracts importable without access to simulation internals.

## Scope

- Create a small contract crate for semantic DTOs used across boundaries.
- Move or copy-then-migrate stable DTOs out of `server/src/protocol.rs`:
  - `StartPayload`
  - `MapInfo`
  - `ResourceNode`
  - `PlayerStart`
  - `PlayerScore`
  - `Snapshot`
  - `SnapshotNetStatus`
  - `PlayerResourceSnapshot`
  - `ResourceDelta`
  - `SmokeCloudView`
  - `EntityView`
  - `AbilityCooldownView`
  - `OrderPlanMarker`
  - `DebugPathPoint`
  - `DebugPathView`
  - `AttackReveal`
  - `Event`
  - `NoticeSeverity`
- Keep WebSocket envelopes and JSON transport machinery in a protocol crate:
  - `ClientMessage`
  - `ServerMessage`
  - wire `Command`
  - compact snapshot serialization
  - compact vocabulary codes
- Keep protocol crate dependencies limited to `serde`, `serde_json`, and the contract crate.
- Update all server imports to use the new crates or temporary re-exports.
- Preserve `client/src/protocol.js` behavior exactly.

## Implementation Notes

- `server/crates/contract` now owns the semantic DTOs used by snapshots, starts, score screens,
  resource deltas, smoke visibility, ability cooldown projection, and events.
- `server/crates/protocol` now owns WebSocket envelopes, gameplay wire commands, string/code
  vocabularies, and compact snapshot serialization.
- `server/src/protocol.rs` remains as a compatibility re-export so existing server, sim, lobby, and
  tool call sites keep compiling while later phases move imports to narrower crates.
- The Rust wire shape is unchanged; `client/src/protocol.js` did not need edits.

## Design Notes

This phase should not force sim to stop using contract DTOs yet. It is acceptable for `rts-sim` to
depend on `rts-contract` while it still has adapters to/from wire commands. The hard line is that
`rts-protocol` must not depend on sim.

Avoid moving `EntityKind` into protocol. Protocol can own string constants and transport compact
codes, but the domain vocabulary belongs in rules/domain in Phase 2.

## Tests

- Contract/protocol unit tests, including compact snapshot tests.
- `cd server && cargo test`
- Existing Node protocol/integration tests when a running server is available.

## Done

- Protocol/contract crates compile without importing `game`, `rules`, `lobby`, `perf`, `tokio`, or
  `axum`.
- All compact snapshot serialization tests still pass.
- The Rust protocol shape and JS mirror remain unchanged at the wire level.
- Sim call sites that build snapshots/events compile through contract types or deliberate
  re-exports.
