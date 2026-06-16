# Phase 3 - Rust-Authoritative Stats Tables

## Phase Status

- [ ] Not implemented.

## Objective

Add generated gameplay-reference tables whose rows come from authoritative Rust rules data, not
Markdown copies or client mirrors.

## Work

- Generate unit, building, and resource-node tables from `rts_rules::defs` and
  `rts_rules::balance`.
- Generate faction, trainable, buildable, upgrade, and ability tables from
  `rts_rules::faction::CATALOGS`.
- Include plain labels and stable ids so non-computer users can connect wiki rows to in-game names.
- Prefer shared data-builder functions that tests can inspect before HTML rendering.
- Avoid manually maintained stats tables except for prose in existing docs.

## Expected Touch Points

- Server wiki module
- `server/Cargo.toml` only if server-side access to extra rules helpers is needed
- `server/crates/rules/src/defs.rs`, `balance.rs`, or `faction.rs` only for read-only helper
  exposure if existing public data is insufficient
- Focused Rust tests or snapshots for generated table data

## Implementation Checklist

- [ ] Add `/wiki/stats` or equivalent generated stats route.
- [ ] Generate unit and building stat rows from Rust definitions.
- [ ] Generate resource, faction catalog, upgrade, and ability rows from Rust definitions.
- [ ] Add table-data tests that compare rendered rows to authoritative Rust records.
- [ ] Add escaping tests for generated labels/titles before HTML output.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server wiki`
- `cargo test --manifest-path server/Cargo.toml -p rts-rules`
- `node scripts/check-faction-catalog-parity.mjs`
- `git diff --check`

## Manual Test Focus

Manual testing should be unnecessary for correctness if generated-row tests pass. If needed, open
`/wiki/stats` and confirm the tables are readable enough to scan.

## Handoff Expectations

List every generated table, the Rust source used for each one, and any authoritative constants that
remain absent from the wiki.
