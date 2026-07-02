# Phase 1 - Lab Replay Artifact Contract

Status: Not started.

## Scope

Define the stable lab replay artifact before adding live product behavior. The artifact should start
from a checkpoint-backed lab setup and append a versioned operation stream that can eventually be
replayed into the same lab state.

## Expected Touch Points

- `docs/design/protocol.md` and `docs/design/server-sim.md`
- `server/crates/protocol/src/`
- `server/src/lobby/lab_timeline.rs`
- `server/src/lobby/room_task/lab.rs`
- `tests/protocol_parity.mjs`
- `tests/client_contracts/lab_contracts.mjs`

## Requirements

- Add or specify a `LabReplayArtifactV1` container with:
  - schema/kind/version fields
  - build and authoring metadata
  - initial `LabCheckpointScenarioV1` or equivalent checkpoint-backed lab setup
  - ordered lab operation entries
  - tick/timeline metadata sufficient to replay deterministic lab state
  - explicit byte, entry, and nested payload caps
- Define the operation DTO policy for current lab ops:
  - spawn/delete/move entity
  - set owner/resources/research/god mode
  - import checkpoint-backed setup
  - issue command as player
  - seek/tick advancement policy
- Decide how entity id remaps from setup imports are represented so later entries cannot refer to
  stale ids ambiguously.
- Add serde/validation tests for valid artifacts, malformed kinds, excessive sizes, stale ids, bad
  player ids, and map/checkpoint mismatch.

## Out Of Scope

- No UI changes.
- No deletion of `LabScenarioV1`.
- No deletion of replay schema 2 loading.
- No checkpoint keyframe replacement.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-protocol lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `node tests/protocol_parity.mjs`
- `node tests/client_contracts/lab_contracts.mjs`
- `git diff --check`

## Manual Testing Focus

No required manual gameplay test for this contract-only phase. Review the artifact JSON shape for
readability and make sure it is not presented as a generic live-match checkpoint upload surface.

## Handoff Notes

Name the exact artifact kind/schema, operation DTOs that are supported, operation DTOs deliberately
left out, and the validation caps chosen.
