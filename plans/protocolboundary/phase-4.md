# Phase 4 - Balance Mirror Parity Expansion

## Phase Status

- [ ] Not implemented.

## Objective

Expand parity checks so client-visible balance values mirrored in `config.js` cannot drift silently.

## Work

- Add or extend a Rust rules dump for client-visible costs, supply, sight, body sizes, durations,
  ability timings, ability ranges, cooldowns, and related command-card descriptors.
- Compare the Rust dump against `client/src/config.js`.
- Explicitly exclude client-owned presentation data such as labels, icons, colors, and layout-only
  render constants.
- Do not tune balance in this phase.

## Expected Touch Points

- `server/crates/rules/src/balance.rs`
- Existing or new rules dump binary
- `scripts/check-faction-catalog-parity.mjs`
- `client/src/config.js`
- `docs/design/balance.md`

## Implementation Checklist

- [ ] Define which config values are Rust-authoritative and client-visible.
- [ ] Add structured dump coverage for those values.
- [ ] Extend parity assertions.
- [ ] Document intentional client-only exclusions.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `node scripts/check-faction-catalog-parity.mjs`
- `node tests/protocol_parity.mjs` if shared constants are touched
- Focused `cargo test --manifest-path server/Cargo.toml -p rts-rules` tests if dump code changes

## Manual Test Focus

Quickly inspect worker build, train, research, and ability command-card costs/ranges in a local
match if config descriptors change.

## Handoff Expectations

List every value intentionally excluded as client-only presentation or render data.
