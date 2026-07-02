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
- Add a playback path that starts from the initial checkpoint-backed lab setup and applies each
  lab operation through the same `Game` lab API used by live operations.
- Preserve the timeline truncation behavior after seeking into the past and applying a new lab op.
- Prove `issueCommandAs` entries replay with the same issuer, command, and command-limit policy.
- Export and import/open lab replay artifacts through a test or dev-only surface first.
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
- Focused live Node/browser test for save/open lab replay
- `git diff --check`

## Manual Testing Focus

Create a lab session, spawn or mutate several entities, issue a player command, export/save the lab
replay, open it again, and confirm the lab world and timeline behavior match expectations.

## Handoff Notes

Name which lab operations are serializable and replayable, which operations still require follow-up,
and whether any id-remap behavior changed.
