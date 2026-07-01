# Phase 1 - Checkpoint File Contract

Status: Not started.

## Scope

Define the versioned checkpoint file contract without moving runtime behavior onto it yet. The
result should be an executor-ready design and DTO skeleton for `GameCheckpointV1`, including an
envelope, schema/version fields, compatibility metadata, validation rules, bounds, and a field map
from the current `GameState`/`DerivedState` registry.

This phase should start from the completed `plans/game-state/` readiness report and
`docs/design/server-sim.md` §3.1.1-§3.1.2. It should explicitly decide which parts of the map are
embedded versus referenced by stable identity/hash, how RNG state is represented, how command-log
compatibility metadata is carried, and which validation errors should be user-facing versus
developer-only.

Explicit non-goals:

- No conversion of live games through checkpoint files yet.
- No replay artifact schema change.
- No lab scenario file migration.
- No public route, UI command, or client protocol change.
- No direct serde derive on private `GameState` as the stable persisted contract.

## Expected Touch Points

- `docs/design/server-sim.md`: add a checkpoint file contract subsection near the ownership
  registry or readiness audit.
- `docs/context/server-sim.md`: point future checkpoint work to the new design subsection if section
  references shift.
- `plans/checkpoint/plan.md` and this phase file if implementation discovers a necessary phase
  adjustment.
- Optional new Rust module skeleton under `server/crates/sim/src/game/` only if useful for naming
  DTOs and validation errors without implementing full conversion.

Avoid touching server room code, client code, protocol crates, replay artifact schemas, lab scenario
schemas, or gameplay systems in this phase.

## Verification

- Confirm every current `GameState` field has one checkpoint DTO strategy: serialized directly,
  represented by a stable DTO, derived during import, or intentionally compatibility-only.
- Confirm `DerivedState.final_spatial` and `DerivedState.pathing` remain rebuild-only and are absent
  from the persisted contract.
- Confirm validation rules cover ids, owners, map shape/hash, coordinates, timers, counts, command
  queues, resource values, and file-size limits.
- Run docs and architecture checks if files touched require them:

```bash
node scripts/check-docs-health.mjs
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
git diff --check -- plans/checkpoint docs/design/server-sim.md docs/context/server-sim.md server/crates/sim/src/game
```

If the phase is docs-only, the Rust architecture check is optional unless the design changes
section anchors consumed by archcheck.

## Manual Testing Focus

No manual gameplay testing is expected. Manual review should focus on whether the contract is
implementable, versioned, bounded, and explicit enough for Phase 2 to build without guessing.

## Handoff

The handoff must name:

- the final checkpoint envelope and versioning policy;
- the field map for every current `GameState` and `DerivedState` field;
- validation and file-size bounds chosen;
- open compatibility decisions, if any;
- exact verification commands that passed;
- the core manual review focus for Phase 2.
