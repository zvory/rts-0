# Phase 6 - Ratchets And Documentation

## Phase Status

- [ ] Not implemented.

## Objective

Lock in the new command/order boundaries with architecture checks and design docs.

## Work

- Tighten archcheck role classifications, import allowlists, broad mutable signature budgets, and
  line-count baselines to reflect the extracted boundary.
- Update `docs/design/server-sim.md` and `docs/context/server-sim.md` if responsibility boundaries
  or recommended service patterns changed.
- Avoid adding new gameplay behavior in this phase.
- Bless baselines only when the reason explains the narrower boundary or renamed files.

## Expected Touch Points

- `server/crates/archcheck/src/lib.rs`
- `server/crates/archcheck/baselines/sim-architecture.json`
- `docs/design/server-sim.md`
- `docs/context/server-sim.md`

## Implementation Checklist

- [ ] Capture the final service responsibility map.
- [ ] Tighten archcheck rules and baselines around command/order service edges.
- [ ] Update design/context docs.
- [ ] Run verification and record exact results in the handoff.
- [ ] Mark this phase complete in the implementation commit.

## Verification

- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- Focused command/order Rust tests selected by the final changed files
- `git diff --check`

## Manual Test Focus

Only a light smoke pass should be needed: movement, queued movement, rally, production, build, and
one ability path.

## Handoff Expectations

Include the final architecture report summary, remaining grandfathered edges, and any follow-up
areas that still deserve a separate plan.
