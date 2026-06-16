# Phase 3 - Structured Protocol Parity Export

## Phase Status

- [ ] Not implemented.

## Objective

Make protocol parity compare structured Rust-owned metadata to the JS mirror.

## Work

- Add a Rust-owned structured dump for message tags, compact codes, kind codes, ability codes,
  order-stage codes, resource codes, and compact snapshot versions.
- Migrate `tests/protocol_parity.mjs` away from source-text scraping where practical.
- Keep `client/src/protocol.js` behavior unchanged unless the structured check exposes a real bug.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- Possible `server/crates/protocol/src/bin/dump-protocol-contract.rs`
- `tests/protocol_parity.mjs`
- `client/src/protocol.js`

## Implementation Checklist

- [ ] Add structured Rust protocol metadata export.
- [ ] Update JS parity test to consume the export.
- [ ] Keep or document any remaining source-text assertions.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `node tests/protocol_parity.mjs`
- Focused `cargo test --manifest-path server/Cargo.toml -p rts-protocol` tests if added
- `node scripts/check-crate-boundaries.mjs`

## Manual Test Focus

No manual test expected unless compact snapshot decoding or start payload behavior changes
accidentally.

## Handoff Expectations

Identify remaining regex/source-text parity checks and explain why they remain.
