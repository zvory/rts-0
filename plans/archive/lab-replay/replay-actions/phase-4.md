# Phase 4 - Lab Save Replay So Far

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Add the server-side lab save operation. It should serialize a replay artifact from the active
baseline checkpoint plus the retained current-branch replay actions, write it to the local dev
artifact area, and return an artifact name and `/?replayArtifact=<name>` URL. It must handle blank
labs, catalog labs, imports, rewinds, edits, and timeline cap reset policy explicitly. The same
phase must harden the file-write surface: the server generates the artifact name, writes only under
a fixed `target/` artifact directory, rejects or avoids client-supplied paths or filenames, enforces
artifact/action size caps before writing, never writes match history, and returns clear validation
errors when saving is unavailable or history is insufficient.

## Expected Touch Points

- `server/src/lobby/room_task/lab.rs`
- `server/src/lobby/dev_replay.rs`
- `server/crates/protocol/src/lib.rs`
- `client/src/protocol.js` only for the request/response DTO
- Server-side lab save tests
- Hardening tests for generated names, fixed output directory, size/action caps, and capability or
  unavailable errors

## Verification

- Add tests for saving blank, catalog, imported, rewound, and edited labs.
- Add tests for clear failure when retained actions are insufficient and no replacement baseline
  checkpoint exists.
- Add tests proving the save path cannot write outside the fixed artifact directory and does not
  accept client-provided paths or filenames.

## Manual Testing Focus

Save from a lab, open the returned replay URL, seek, and verify the replay shows the expected branch.

## Handoff

The handoff must provide the exact URL or artifact path pattern used by the save flow and the
hardening limits enforced before any file write.
