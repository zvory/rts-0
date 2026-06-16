# Phase 4 - Protocol And Config Drift Guardrails

## Phase Status

- [ ] Not implemented.

## Objective

Align faction-facing protocol/config docs and parity checks with current code.

## Work

- Tighten protocol parity around `setFaction`, `DEFAULT_FACTION_ID`, faction-bearing payload fields,
  ability codes, order-stage codes, kind codes, and compact snapshot version.
- Verify `server/src/protocol.rs` and `client/src/protocol.js` stay aligned for faction-facing
  constants and mappings.
- Require protocol docs to list every faction-facing compact kind, ability, order-stage, and compact
  version touched by the current code. Add a doc-code checker if practical; otherwise make the
  phase acceptance criteria explicitly include the doc parity review.
- Update docs only where current code establishes the contract.
- Avoid gameplay changes unless a mismatch reveals a real bug.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `docs/design/protocol.md`
- `tests/protocol_parity.mjs`
- Possibly `client/src/config.js`

## Implementation Checklist

- [ ] Inventory faction-facing protocol constants and fields.
- [ ] Tighten parity assertions.
- [ ] Update stale docs to code-confirmed facts.
- [ ] Confirm protocol docs list the same faction-facing codes and compact version as Rust/JS.
- [ ] Confirm compact snapshot version handling.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `node tests/protocol_parity.mjs`
- `cargo test --manifest-path server/Cargo.toml -p rts-protocol`
- `node scripts/check-faction-catalog-parity.mjs`

## Manual Test Focus

No deep gameplay test expected. Inspect lobby and start payloads if protocol fields change.

## Handoff Expectations

List every protocol field or compact code touched and state whether compact snapshot version changed.
