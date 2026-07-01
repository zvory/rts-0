# Phase 5 - Lab Scenario Checkpoint Adapter

Status: Not started.

## Scope

Add side-by-side adapters between current lab scenarios and checkpoint starts. Current
`LabScenarioV1` assets should be convertible into `GameCheckpointV1` starts, and lab export should
be able to emit checkpoint-backed scenario data behind a test/debug or explicit internal option.

This phase is compatibility-first. Preserve current lab import/export UI behavior, id remap
responses, validation messages, authoring metadata, submission guardrails, and catalog behavior
while proving the checkpoint path restores the same lab world.

Explicit non-goals:

- No bundled lab catalog asset rewrite.
- No removal of `LabScenarioV1` support.
- No lab timeline action-stream migration.
- No public file picker/upload feature beyond existing lab scenario import/export behavior.

## Expected Touch Points

- `server/crates/sim/src/game/lab.rs`: compile `LabScenarioV1` into checkpoint starts and optionally
  export checkpoint-backed lab setup data.
- `server/src/lab_scenarios.rs`: validate checkpoint-backed previews beside existing scenario
  previews if needed.
- `server/src/lobby/room_task/lab.rs` and lab submission helpers: read-only unless an internal
  option is needed to exercise the adapter.
- `server/crates/protocol` and `client/src/lab_*`: avoid changes unless the phase explicitly adds a
  compatibility metadata field; do not alter UI copy casually.
- Tests covering scenario import/export parity, validation failures, id remap behavior, and
  authoring/submission guardrails.

## Verification

- For representative `LabScenarioV1` fixtures, direct scenario restore and checkpoint-adapter
  restore produce equivalent semantic state and snapshots.
- Id remap responses remain correct for existing lab import callers.
- Exported scenario metadata, authoring fields, selected lab vision, god mode, resources, research,
  and entity setup targets survive adapter round trips.
- Invalid scenario files still fail closed with clear messages before constructing a live game.
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
- preserved id-remap and metadata behavior;
- validation coverage;
- any protocol/client changes, or confirmation there were none;
- focused tests that passed;
- manual lab scenario smoke focus for Phase 6.
