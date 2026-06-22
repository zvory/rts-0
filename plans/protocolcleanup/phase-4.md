# Phase 4 - Compact Snapshot Codec Split

Status: planned.

## Goal

Move compact snapshot semantic serialization and decoding helpers into focused modules while
preserving the semantic snapshot shape consumed by the server, client state, renderer, HUD, minimap,
prediction, replay, and tests.

## Scope

- Extract Rust compact snapshot serializer structs and helper functions from
  `server/crates/protocol/src/lib.rs` into a focused internal module such as
  `server/crates/protocol/src/compact_snapshot.rs`.
- Keep Rust public serializers stable: `serialize_compact_snapshot`,
  `serialize_messagepack_compact_snapshot`, and `encode_snapshot_frame`.
- Extract JS `decodeCompactSnapshot` and its private entity, event, ability-object, visible-tile,
  remembered-building, resource-delta, net-status, and read/validation helpers into a focused module
  such as `client/src/protocol_snapshot.js`.
- Keep `decodeServerMessage` exported from `client/src/protocol.js` and preserve the same semantic
  decoded object shape, malformed-frame rejection behavior, caps, optional omission handling, and
  unknown-code errors.
- Update docs only for internal boundary paths, not for compact schema changes.

## Touch Points

- `server/crates/protocol/src/lib.rs`
- possible `server/crates/protocol/src/compact_snapshot.rs`
- `client/src/protocol.js`
- possible `client/src/protocol_snapshot.js`
- `tests/protocol_parity.mjs`
- `tests/client_contracts/protocol_contracts.mjs`
- `tests/client_contracts/snapshot_frame_helpers.mjs`, only if fixture plumbing needs the new helper
- `docs/design/protocol.md`

## Constraints

- Do not change compact snapshot version `23`, field order, top-level compact keys, compact code
  values, optional trailing-slot omission, interior `null` handling, event record shapes, owner-only
  fields, fog-gated fields, or caps.
- Do not change the semantic decoded snapshot field names consumed by `GameState.applySnapshot`.
- Do not change event visibility, fog projection, death sight, smoke visibility, ability-object
  owner-state rules, remembered-building semantics, or command acknowledgement metadata.
- If any fixture must be updated because the wire shape changed, stop and report blocked; fixture
  changes should only be import-path or helper-path updates in this cleanup plan.

## Verification

- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- focused Rust protocol tests covering compact JSON and MessagePack snapshot serialization
- `node tests/regression.mjs` only if decode imports used by live test helpers are touched in a way
  that targeted protocol/client contracts do not cover
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Testing Focus

Run one local browser match or a live Node server smoke only if the code movement changes the
runtime import path used by `client/src/net.js`. Confirm snapshots still parse and render, resources
and units appear, and malformed compact frames are still rejected by the client contract tests.

## Handoff

Mark this phase done only after committing the compact snapshot codec split. Summarize exactly what
moved, unchanged compact version/schema facts, focused verification, and whether Phase 5 only has
comments/import cleanup left.
