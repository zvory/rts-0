# Phase 2 - Frame Transport Split

Status: planned.

## Goal

Move low-level snapshot frame transport internals out of the large protocol files while preserving
the same public Rust and JS entry points.

## Scope

- Extract Rust MessagePack frame-writing helpers and frame envelope details from
  `server/crates/protocol/src/lib.rs` into a focused internal module such as
  `server/crates/protocol/src/messagepack_frame.rs`.
- Keep Rust public names and behavior stable: `encode_snapshot_frame`,
  `serialize_messagepack_compact_snapshot`, `SnapshotCodec`, `SnapshotFrame`,
  `SNAPSHOT_CODEC_VERSION`, and `MESSAGEPACK_SNAPSHOT_FRAME_MAGIC`.
- Extract JS binary frame parsing and the MessagePack reader from `client/src/protocol.js` into a
  focused module such as `client/src/protocol_frame.js`.
- Keep `parseServerFrame` exported from `client/src/protocol.js` with the same accepted inputs,
  malformed-frame errors, MessagePack version checks, and JSON fallback behavior.
- Update `docs/design/protocol.md` boundary inventory only to point at the new internal module paths.

## Touch Points

- `server/crates/protocol/src/lib.rs`
- possible `server/crates/protocol/src/messagepack_frame.rs`
- `client/src/protocol.js`
- possible `client/src/protocol_frame.js`
- `tests/protocol_parity.mjs`
- `tests/client_contracts/protocol_contracts.mjs`
- `docs/design/protocol.md`
- `scripts/hotspot-analysis.mjs` and `plans/hotspots/group-map.md` if Phase 1 did not already cover
  the new JS split path

## Constraints

- Do not change compact snapshot version `23`, codec version `1`, frame magic `RTSM`, frame kind
  labels, compact JSON baseline behavior, or the active MessagePack default.
- Do not change the raw compact snapshot object shape handed to the semantic decoder.
- Do not expose the new JS helper module to app callers; app and tests should continue importing
  from `client/src/protocol.js` unless a test is explicitly focused on the helper.
- Do not make `rts-protocol` depend on sim, rules, server, or AI crates.

## Verification

- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- focused Rust protocol tests covering snapshot codecs and MessagePack frames
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Testing Focus

No broad gameplay manual test is expected for this mechanical split. Manually inspect that a live
snapshot still travels through `parseServerFrame` to `decodeServerMessage` and that no client/server
caller imports the new internal modules directly for normal operation.

## Handoff

Mark this phase done only after committing the frame transport split. Summarize the new internal
module paths, unchanged public entry points, exact verification, and any transport code left in
`lib.rs` or `protocol.js` for Phase 4.
