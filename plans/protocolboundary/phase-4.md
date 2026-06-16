# Phase 4 - Balance Mirror Parity Expansion

## Phase Status

- [x] Done.

## Objective

Expand parity checks so client-visible balance values mirrored in `config.js` cannot drift silently.

## Work

- Add or extend a Rust rules dump for client-visible costs, supply, sight, body sizes, durations,
  ability timings, ability ranges, cooldowns, and related command-card descriptors.
- Compare the Rust dump against `client/src/config.js`.
- Explicitly exclude only client-owned presentation data not present in Rust catalogs, such as
  global colors, fog alpha, camera defaults, layout hints, and purely local render affordances.
  Command-card descriptors exported by Rust faction/catalog registries remain Rust-owned even when
  they include labels, icons, hotkeys, target modes, ranges, cooldowns, costs, queue flags, or
  autocast metadata.
- Do not tune balance in this phase.

## Expected Touch Points

- `server/crates/rules/src/balance.rs`
- Existing or new rules dump binary
- `scripts/check-faction-catalog-parity.mjs`
- `client/src/config.js`
- `docs/design/balance.md`

## Implementation Checklist

- [x] Define which config values are Rust-authoritative and client-visible.
- [x] Add structured dump coverage for those values.
- [x] Extend parity assertions.
- [x] Document intentional client-only exclusions.
- [x] Run verification and record exact results in the handoff.

## Verification

- `node scripts/check-faction-catalog-parity.mjs`
- `node scripts/check-wiki.mjs` if visible stats/catalog/ability metadata changes
- `node tests/protocol_parity.mjs` if shared constants are touched
- Focused `cargo test --manifest-path server/Cargo.toml -p rts-rules` tests if dump code changes

## Manual Test Focus

Quickly inspect worker build, train, research, and ability command-card costs/ranges in a local
match if config descriptors change.

## Handoff Expectations

List every value intentionally excluded as client-only presentation or render data.
