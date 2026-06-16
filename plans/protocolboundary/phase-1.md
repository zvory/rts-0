# Phase 1 - Boundary Inventory

## Phase Status

- [ ] Not implemented.

## Objective

Document the current source-of-truth map for protocol, adapter, config, balance, and client mirror
values.

## Work

- Classify mirrored values as wire DTO, compact transport code, domain adapter mapping, balance
  scalar, faction catalog fact, UI-only presentation data, or server-only constant.
- Record ambiguous values that need a product or architecture decision before enforcement.
- Update design docs only where they are stale about current authority.

## Expected Touch Points

- `docs/design/protocol.md`
- `docs/design/balance.md`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `server/crates/sim/src/protocol.rs`
- `server/crates/rules/src/balance.rs`
- `client/src/protocol.js`
- `client/src/config.js`

## Implementation Checklist

- [ ] Inventory protocol constants and compact codes.
- [ ] Inventory mirrored balance/config values.
- [ ] Mark UI-only client data explicitly.
- [ ] Record ambiguous ownership decisions.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `git diff --check`
- `node tests/protocol_parity.mjs` if protocol docs or assumptions are touched
- `node scripts/check-faction-catalog-parity.mjs` if catalog mirror assumptions are touched

## Manual Test Focus

No runtime manual testing expected. Human review should confirm value ownership classifications.

## Handoff Expectations

List disputed constants or fields whose ownership remains unclear before implementation phases.
