# Phase 5 - Lab Scenario Checkpoint Adapter

Status: Done.

## Scope

Add side-by-side adapters between current lab scenarios and checkpoint-backed scenario containers.
Current `LabScenarioV1` assets should be convertible into map data/binding plus `GameCheckpointV1`,
and lab export should be able to emit checkpoint-backed scenario data behind a test/debug or
explicit internal option.

This phase is compatibility-first. Preserve current lab import/export UI behavior, id remap
responses, validation messages, authoring metadata, submission guardrails, and catalog behavior
while proving the checkpoint path restores the same lab world.

Explicit non-goals:

- No bundled lab catalog asset rewrite.
- No removal of `LabScenarioV1` support.
- No lab timeline action-stream migration.
- No public file picker/upload feature beyond existing lab scenario import/export behavior.
- No casual wire/protocol shape change. If the optional checkpoint-backed scenario export changes
  client-visible JSON, it must be treated as a protocol change and mirrored in docs/client/server
  protocol code in this same phase.

## Expected Touch Points

- `server/crates/sim/src/game/lab.rs`: compile `LabScenarioV1` into checkpoint starts and optionally
  export checkpoint-backed lab setup data.
- `server/src/lab_scenarios.rs`: validate checkpoint-backed previews beside existing scenario
  previews if needed.
- `server/src/lobby/room_task/lab.rs` and lab submission helpers: read-only unless an internal
  option is needed to exercise the adapter.
- `server/crates/protocol` and `client/src/lab_*`: avoid changes unless the phase explicitly adds a
  compatibility metadata field or exposes checkpoint-backed scenario JSON; do not alter UI copy
  casually.
- `docs/design/protocol.md` and `docs/context/protocol.md`: update if any import/export scenario
  shape, metadata field, or validation response changes.
- Tests covering scenario import/export parity, validation failures, id remap behavior, and
  authoring/submission guardrails.

## Verification

- For representative `LabScenarioV1` fixtures, direct scenario restore and checkpoint-adapter
  restore produce equivalent semantic state and snapshots.
- Id remap responses remain correct for existing lab import callers.
- The checkpoint-backed scenario container preserves the scenario's map binding and rejects restore
  against the wrong map identity/hash.
- Exported scenario metadata, authoring fields, selected lab vision, god mode, resources, research,
  and entity setup targets survive adapter round trips.
- Invalid scenario files still fail closed with clear messages before constructing a live game.
- If protocol-visible JSON changes, run protocol parity:

```bash
node tests/protocol_parity.mjs
```

- Suggested focused commands:

```bash
cargo fmt --manifest-path server/Cargo.toml
cargo test --manifest-path server/Cargo.toml -p rts-sim lab
cargo test --manifest-path server/Cargo.toml -p rts-sim checkpoint_lab
cargo test --manifest-path server/Cargo.toml -p rts-server lab
node scripts/check-crate-boundaries.mjs
git diff --check -- server/crates/sim/src/game server/src/lab_scenarios.rs server/src/lobby/room_task/lab.rs server/crates/protocol client/src plans/checkpoint
```

Use narrower filters if final test names differ.

## Manual Testing Focus

Open one bundled lab scenario, export it, restore it, and verify visible entities, resources,
research, setup targets, and lab controls behave as before. If an internal checkpoint export option
exists, inspect one exported checkpoint-backed lab file for expected metadata and bounds.

## Handoff

The handoff must name:

- adapter direction(s) implemented;
- scenario container shape, including map data/binding plus embedded `GameCheckpointV1`;
- preserved id-remap and metadata behavior;
- validation coverage;
- any protocol/client changes, or confirmation there were none;
- focused tests that passed;
- manual lab scenario smoke focus for Phase 6.
