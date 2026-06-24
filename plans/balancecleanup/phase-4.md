# Phase 4 - Rust Balance Split

Status: planned.

## Goal

Split Rust balance internals into focused modules while preserving the existing
`rts_rules::balance::*` public surface and compatibility shims.

## Scope

- Move timing, map/resource, movement, support-weapon, ability, upgrade, economy, supply, body, and
  stats helper definitions into focused internal modules under the balance namespace.
- Keep `server/crates/rules/src/balance.rs` as the stable public re-export surface for downstream
  callers and compatibility shims.
- Keep `server/src/config.rs` and `server/crates/sim/src/config.rs` behavior-compatible and thin.
- Update `docs/design/balance.md` only to describe the new internal module paths and confirm that
  source-of-truth ownership did not change.
- Do not move `defs.rs` and `faction.rs` data unless the move is a mechanical import-path update
  required by the balance module split.

## Touch Points

- `server/crates/rules/src/balance.rs`
- possible `server/crates/rules/src/balance/*.rs` files
- `server/crates/rules/src/defs.rs` and `server/crates/rules/src/faction.rs`, only for import-path
  fallout
- `server/src/config.rs`
- `server/crates/sim/src/config.rs`
- `docs/design/balance.md`
- `scripts/hotspot-analysis.mjs` and `docs/hotspot-analysis.md`, only if Phase 1 did not already
  cover the chosen split paths

## Constraints

- Preserve all exported Rust names and values.
- Do not change `UnitStats`, `BuildingStats`, `unit_stats`, `building_stats`, or
  `unit_radius_tiles` behavior.
- Do not move sim-only behavior constants into `rts_rules::balance` unless Phase 2 explicitly
  established them as Rust-owned client-visible mirror values.
- Do not use broad workspace formatting; format only touched Rust files if needed.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-rules`
- `node scripts/check-faction-catalog-parity.mjs`
- `node scripts/check-wiki.mjs`
- `node tests/client_contracts.mjs`
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected from source movement alone. Manually review the before/after
Rust exports and generated wiki/faction outputs, then later sanity-check one match start, one build
menu, and one ability command.

## Handoff

Mark this phase done only after committing the Rust split. Summarize the new module boundaries,
unchanged exported names, parity/wiki/Rust verification, any import fallout, and whether Phase 5 can
run the final no-drift closeout.
