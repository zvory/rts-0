# Phase 3 - Constants And Contract Metadata Split

Status: done.

## Goal

Move protocol constants, compact code tables, and contract metadata into focused modules while
keeping the public Rust and JS mirror exports unchanged.

## Scope

- Extract Rust protocol contract structs, compact code tables, slot schema metadata, vocabulary maps,
  and `protocol_contract()` implementation from `server/crates/protocol/src/lib.rs` into a focused
  internal module such as `server/crates/protocol/src/contract_metadata.rs`.
- Keep `protocol_contract()` exported from `rts_protocol` with the same JSON shape emitted by
  `dump-protocol-contract`.
- Extract JS message tags, command tags, vocabularies, compact code maps, reverse-code maps, version
  constants, and small classification helpers into a focused module such as
  `client/src/protocol_constants.js`.
- Keep all existing JS names re-exported from `client/src/protocol.js`, including `S`, `C`, `CMD`,
  `KIND`, `UNIT_KINDS`, `BUILDING_KINDS`, `RESOURCE_KINDS`, `STATE`, `SETUP`, `EVENT`, `ABILITY`,
  `UPGRADE`, `*_CODE`, `COMPACT_SNAPSHOT_VERSION`, `SNAPSHOT_CODEC`, and related helpers.
- Update docs only for internal boundary paths, not for wire shape.

## Touch Points

- `server/crates/protocol/src/lib.rs`
- possible `server/crates/protocol/src/contract_metadata.rs`
- `client/src/protocol.js`
- possible `client/src/protocol_constants.js`
- `server/crates/protocol/src/bin/dump-protocol-contract.rs`, only if module visibility requires it
- `tests/protocol_parity.mjs`
- `tests/client_contracts/protocol_contracts.mjs`
- `docs/design/protocol.md`

## Constraints

- Do not change any tag string, command string, kind string, code value, code map key, vocabulary key,
  protocol contract JSON field, or code table order.
- Do not change the unknown-code sentinel `255`.
- Do not move rules/sim kind conversion into the protocol crate.
- Do not remove compatibility constants or helpers just because a current module has no direct
  runtime import.

## Verification

- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- focused Rust protocol tests covering `protocol_contract` and compact codes
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected. Manually review the protocol contract dump diff, JS exported
constants, and docs boundary inventory to confirm the split did not change the mirrored vocabulary.

## Handoff

Mark this phase done only after committing the constants and contract metadata split. Summarize the
new module paths, unchanged exported names, verification, and any constants intentionally left in the
top-level files for the compact snapshot codec phase.
