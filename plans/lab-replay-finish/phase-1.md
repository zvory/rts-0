# Phase 1 - Lab Replay Artifact Contract

Status: Done.

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
- Before adding the DTO, choose and document the owning crate and adapter boundary. If the artifact
  lives in protocol/contract space, add mirrored client constants and parity coverage; if it lives in
  sim/server space, keep the public wire import/export shape separate from sim-private runtime types.
- Define the operation DTO policy for current lab ops:
  - spawn/delete/move entity
  - set owner/resources/research/god mode
  - import checkpoint-backed setup
  - issue command as player
  - set lab vision, either as a serializable entry or as explicitly excluded session metadata with
    deterministic reopen behavior
  - seek/tick advancement policy
- State that the durable artifact stream is not `LabSession.operation_log` and is not the retained
  `LabTimeline` keyframe list. Room-local keyframes may be rebuilt from the artifact, but the artifact
  must remain replayable after process restart.
- Define reset/import semantics. A checkpoint setup import must either become a durable operation
  with clear id-remap scope for following entries, or it must rebase the artifact by replacing the
  initial setup and clearing prior entries.
- Define the import/export transport policy and whole-artifact cap. The contract must say whether a
  lab replay artifact can travel through the current WebSocket lab envelope, an HTTP/dev endpoint, a
  local file-only client path, or another bounded route.
- Decide how entity id remaps from setup imports are represented so later entries cannot refer to
  stale ids ambiguously.
- Add serde/validation tests for valid artifacts, malformed kinds, excessive sizes, too many entries,
  oversized nested payloads, stale ids, bad player ids, map/checkpoint mismatch, unsupported vision
  metadata if excluded, and setup-import id-remap ambiguity.

## Out Of Scope

- No UI changes.
- No deletion of `LabScenarioV1`.
- No deletion of replay schema 2 loading.
- No checkpoint keyframe replacement.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-protocol lab`
- Contract/sim crate test selected by the final DTO owner, if different from `rts-protocol`
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
