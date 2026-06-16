# Phase 2 - Protocol Adapter Consolidation

## Phase Status

- [ ] Not implemented.

## Objective

Consolidate duplicated kind-to-wire conversion without changing protocol shapes.

## Work

- Introduce a single rules-aware adapter path that server shell and sim can share without making
  `rts-protocol` depend on rules or sim.
- Keep existing public imports stable through re-exports where that avoids unrelated churn.
- Add focused tests for kind conversion and adapter behavior.

## Expected Touch Points

- `server/src/protocol.rs`
- `server/crates/sim/src/protocol.rs`
- A small shared adapter module if the dependency graph allows it
- Focused Rust tests

## Implementation Checklist

- [ ] Identify duplicated conversion tables.
- [ ] Choose a dependency-safe shared adapter location.
- [ ] Preserve public import compatibility.
- [ ] Add focused conversion tests.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-sim protocol`
- Focused server protocol tests, if present
- `node tests/protocol_parity.mjs`
- `node scripts/check-crate-boundaries.mjs`

## Manual Test Focus

Start a local match only if start payload or snapshot projection call sites changed.

## Handoff Expectations

Note remaining `crate::protocol` call sites that still mix DTO use and domain conversion.
