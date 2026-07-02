# Phase 2 - Lab Operation Recording And Playback

Status: Not started.

## Scope

Wire the lab timeline to the lab replay artifact. A saved lab replay should restore its initial
checkpoint-backed setup, replay the serialized lab operations in order, and reach the same
observable lab state.

## Expected Touch Points

- `server/src/lobby/lab_timeline.rs`
- `server/src/lobby/room_task/lab.rs`
- `server/src/lobby/room_task/tests/lab_timeline.rs`
- `server/src/lab_scenarios.rs` only if setup import helpers need to be shared
- `client/src/lab_panel.js`
- `client/src/lab_client.js`
- `tests/lab_mortar_regression.mjs`
- New focused live Node or client contract coverage for lab replay open/save

## Requirements

- Record stable lab operation entries from live lab mutations instead of relying only on in-memory
  `LabTimelineEntry` internals.
- Keep the durable replay entry stream separate from `LabSession.operation_log`, which only carries
  request/result metadata for the current room, and from retained `LabTimeline` keyframes, which are
  an in-process seek optimization.
- Add a playback path that starts from the initial checkpoint-backed lab setup and applies each
  lab operation through the same `Game` lab API used by live operations.
- Preserve the timeline truncation behavior after seeking into the past and applying a new lab op.
  Future durable replay entries must be truncated or rebased with the same semantics as the
  room-local timeline.
- Prove `issueCommandAs` entries replay with the same issuer, command, and command-limit policy.
- Implement the Phase 1 setup-import reset policy. If import rebases the artifact baseline, prove the
  exported artifact starts from the imported checkpoint and has no stale pre-import entries; if import
  is a replayable entry, prove later entity references use the documented remap scope.
- Apply the Phase 1 `setVision` decision. If vision is serializable, prove reopened lab state matches
  the saved operator/default vision; if it is excluded, prove reopen uses the documented fallback and
  user-facing metadata says so.
- Export and import/open lab replay artifacts through a bounded test or dev-only surface first. Do
  not add an unbounded WebSocket JSON upload path; rejected oversize artifacts should fail before the
  live lab game is mutated.
- Add a process-cold round-trip test: deserialize only the exported artifact JSON into a fresh lab
  room/session, rebuild the game and timeline from that artifact, and compare observable snapshots,
  lab state, room-time state, god-mode players, operation count, and replayable command effects.
- Keep existing in-memory keyframes for seek performance unless a specific bug requires changing
  them.

## Out Of Scope

- No compatibility deletion.
- No database persistence requirement.
- No generic checkpoint upload command.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab_timeline`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `node tests/protocol_parity.mjs`
- Focused live Node/browser test for save/open lab replay, plus the process-cold server round trip
- `git diff --check`

## Manual Testing Focus

Create a lab session, spawn or mutate several entities, change lab vision, issue a player command,
import a checkpoint setup if that path is supported, export/save the lab replay, open it again from
the saved artifact only, and confirm the lab world, lab state, and timeline behavior match
expectations.

## Handoff Notes

Name which lab operations are serializable and replayable, which operations still require follow-up,
the transport/cap used for save/open, whether setup imports rebase or replay as entries, and whether
any id-remap behavior changed.
