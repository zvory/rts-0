# Phase 5 - Config Shim Cleanup

## Phase Status

- [ ] Not implemented.

## Objective

Make server and sim config shims reflect intentional compatibility rather than unclear ownership.

## Work

- Narrow `server/src/config.rs` and `server/crates/sim/src/config.rs` comments and exports to
  intentional compatibility surfaces.
- Move sim-only constants that are not mirrored balance into clearly named sim-local modules.
- Avoid broad import rewrites unless the touched module has focused tests.
- Update docs when a constant's ownership is reclassified.

## Expected Touch Points

- `server/src/config.rs`
- `server/crates/sim/src/config.rs`
- Selected sim modules using sim-only constants
- `server/crates/rules/src/balance.rs`
- `docs/design/balance.md`

## Implementation Checklist

- [ ] Identify compatibility exports versus sim-only constants.
- [ ] Move or rename only the constants selected for this phase.
- [ ] Update call sites conservatively.
- [ ] Update docs with before/after ownership.
- [ ] Run verification and record exact results in the handoff.

## Verification

- Focused `cargo test --manifest-path server/Cargo.toml -p rts-sim` tests for touched modules
- `cargo build --manifest-path server/Cargo.toml`
- `node scripts/check-faction-catalog-parity.mjs` if mirrored values are touched

## Manual Test Focus

Sandbox match start, basic build/train/research, and one ability using moved timing or range
constants.

## Handoff Expectations

Include a before/after ownership table for every moved or reclassified constant.
