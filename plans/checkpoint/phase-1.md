# Phase 1 - Checkpoint Payload Contract

Status: Not started.

## Scope

Define the versioned checkpoint payload contract without moving runtime behavior onto it yet. The
result should be an executor-ready design and DTO skeleton for `GameCheckpointV1`, including an
embeddable text schema, container expectations, schema/version fields, compatibility metadata,
validation rules, bounds, and a field map from the current `GameState`/`DerivedState` registry.

This phase should start from the completed `plans/game-state/` readiness report and
`docs/design/server-sim.md` §3.1.1-§3.1.2. It should explicitly separate runtime `GameState`
ownership from external artifact composition: `Game` may own the live map internally, but
`GameCheckpointV1` should be embeddable inside replays, lab scenarios, match-start artifacts, and
debug documents rather than becoming its own product-specific file format. Because the current
registry classifies runtime `GameState.map` as a full internal checkpoint field, this phase must
reconcile that row with the public payload rule that outer containers supply exact map data while
the payload carries only a binding. The phase should decide the map binding contract for checkpoint
import, how an outer container supplies or embeds the exact map, how RNG state is represented, how
command-log compatibility metadata is carried, and which validation errors should be user-facing
versus developer-only.

Explicit non-goals:

- No conversion of live games through checkpoint payloads yet.
- No replay artifact schema change.
- No lab scenario file migration.
- No public route, UI command, or client protocol change.
- No direct serde derive on private `GameState` as the stable persisted contract.

## Expected Touch Points

- `docs/design/server-sim.md`: add a checkpoint payload contract subsection near the ownership
  registry or readiness audit, including the distinction between runtime map ownership and
  container-supplied map data/bindings. Update or annotate the `map` registry row so the internal
  `GameState.map` ownership policy and the public `GameCheckpointV1` map-binding policy cannot be
  read as conflicting instructions.
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
- Confirm the `map` field's public checkpoint policy distinguishes the live `Game` ownership tree
  from external artifact composition: a map remains normal map data, while the checkpoint payload
  carries enough map binding facts to validate that the containing artifact supplied the exact map.
- Confirm the registry/map-policy reconciliation is explicit enough that Phase 2 cannot accidentally
  serialize a full map body inside `GameCheckpointV1` while also expecting a container-supplied map.
- Confirm the field map covers entity-local active orders, queued order intents, selected
  paths/waypoints/path goals, smoke store active/pending entries, scheduled mortar/artillery
  impacts, and ability runtime projectiles/world objects.
- Confirm `DerivedState.final_spatial` and `DerivedState.pathing` remain rebuild-only and are absent
  from the persisted contract.
- Confirm validation rules cover ids, owners, map shape/hash, coordinates, timers, counts, command
  queues, resource values, payload-size limits, and container-size limits.
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

- the final checkpoint payload schema, container expectations, and versioning policy;
- the map binding policy and how replays, scenarios, starts, and debug documents supply map data;
- the exact `GameState.map` registry reconciliation made for the public payload contract;
- the field map for every current `GameState` and `DerivedState` field;
- validation and payload/container-size bounds chosen;
- open compatibility decisions, if any;
- exact verification commands that passed;
- the core manual review focus for Phase 2.
