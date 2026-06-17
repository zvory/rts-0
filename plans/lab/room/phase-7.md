# Phase 7 - Cleanup, Docs, And Guardrails

## Phase Status

- [ ] Done.

## Objective

Remove temporary compatibility code, update architecture documentation, and add lightweight
guardrails after the composable room primitives exist.

## Work

- Remove compatibility helpers or duplicate mode checks that earlier phases intentionally left for
  sequencing, as long as removal is clearly behavior-neutral.
- Update `docs/design/server-sim.md` and `docs/context/server-sim.md` to describe the new lobby
  room modules and their responsibilities.
- Update `plans/room/mode-matrix.md` if the final internal module names differ from the Phase 1
  predictions.
- Re-evaluate `/dev/scenario` after the shared participants, clock, projection, and launch helpers
  exist. If the scenario path can move onto those helpers without broadening behavior, migrate it;
  otherwise document the remaining reason it stays mode-local.
- Add or tighten a lightweight architecture check only for stable boundaries that are now worth
  enforcing, such as prohibiting new deep sim mutation from lobby or keeping projection decisions
  in the projection helper.
- Do not add new product capabilities, protocol, or lab behavior.

## Expected Touch Points

- `server/src/lobby/*.rs`
- `docs/design/server-sim.md`
- `docs/context/server-sim.md`
- `plans/room/mode-matrix.md`
- `scripts/check-*.mjs` or `server/crates/archcheck` only if a clear guardrail is justified

## Implementation Checklist

- [ ] Remove temporary shims that no longer have consumers.
- [ ] Update room/lobby architecture docs to match the final module boundaries.
- [ ] Add guardrails only where the boundary is stable and the check is precise.
- [ ] Run the focused room tests carried forward from prior phases.
- [ ] Run any architecture check touched by this phase.
- [ ] Mark the phase as done in this file.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `node tests/protocol_parity.mjs`
- `git diff --check`

## Manual Test Focus

Normal match lifecycle, spectator lifecycle, post-match replay, persisted replay join, replay
branch staging and launch, saved artifact replay inspection if it exists, and one dev scenario.

## Handoff Expectations

Summarize the final room primitive boundaries, list remaining known duplication or risks, and state
which follow-up lab or room plan should consume these primitives next.
