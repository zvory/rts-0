# Phase 4 - Lab Save Replay So Far

Status: Not started.

## Scope

Add the server-side lab save operation. It should serialize a replay artifact from the active
baseline checkpoint plus the retained current-branch replay actions, write it to the local dev
artifact area, and return an artifact name and `/?replayArtifact=<name>` URL. It must handle blank
labs, catalog labs, imports, rewinds, edits, and timeline cap reset policy explicitly.

## Expected Touch Points

- `server/src/lobby/room_task/lab.rs`
- `server/src/lobby/dev_replay.rs`
- `server/crates/protocol/src/lib.rs`
- `client/src/protocol.js` only for the request/response DTO
- Server-side lab save tests

## Verification

- Add tests for saving blank, catalog, imported, rewound, and edited labs.
- Add tests for clear failure when retained actions are insufficient and no replacement baseline
  checkpoint exists.

## Manual Testing Focus

Save from a lab, open the returned replay URL, seek, and verify the replay shows the expected branch.

## Handoff

The handoff must provide the exact URL or artifact path pattern used by the save flow.
