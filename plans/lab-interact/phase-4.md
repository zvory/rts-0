# Phase 4 - Portable Setup And Replay Artifacts

Status: planned.

## Prerequisite

Do not start until Phase 0 has merged and the migrated Phase 3 CLI workflow has passed its manual
review gate.

## Goal

Let agents save and reopen useful tiny scenes without hand-authoring checkpoint internals. Static
state should use the existing checkpoint-backed Lab setup contract, while timed Lab mutations and
orders should use the existing portable Lab replay contract.

## Scope

- Add bounded `lab-interact export` and `lab-interact import` commands requiring the current opaque
  `sessionId`, with an explicit artifact kind of `setup` or `replay`. Store artifacts and alias
  sidecars under the current worktree's ignored `target/lab-interact/` root.
- For static setups, reuse `LabClient.exportScenario`, checkpoint scenario import, and the server's
  existing `LabCheckpointScenarioV1` validation. CLI JSON should return metadata, counts, safe
  absolute paths, and validation results without printing the embedded `checkpointPayload`.
- Promote the server-side `export_lab_replay_artifact` and `load_lab_replay_artifact` capabilities
  through a bounded daemon-owned local path. Long replay artifacts must not be embedded in the
  current WebSocket Lab result because that contract already reserves them for local transfer.
- Choose a local-only, environment-gated artifact bridge owned by the daemon-started private server
  and driver. It must be unavailable in production startup, bind only to loopback if networking is
  unavoidable, require an unguessable session capability, use opaque artifact ids rather than
  browser-supplied paths, enforce existing byte/operation/schema bounds, expire temporary data, and
  clean it on session close or daemon teardown.
- If implementation evidence supports a safer equally bounded local transport, document the
  alternative before using it. Do not reconstruct the authoritative replay stream from a
  daemon-side best-effort command log when the room already owns accepted operation ticks and
  request order.
- Persist aliases in a small Lab Interact sidecar keyed to artifact identity, never inside protocol
  schemas. Reconcile setup aliases through `sourceEntityIdMap`; on replay import, validate aliases
  against restored initial/current state and report stale entries.
- Add `lab-interact artifact-inspect` to return authoring metadata, map, tick/duration,
  entity/operation counts, build compatibility, aliases, and validation status without printing the
  full artifact.
- Let `open` or `import` select an artifact only by opaque id or a path confined beneath the current
  worktree's `target/lab-interact/`. Reject URLs and arbitrary filesystem paths.
- Add an optional concise reproduction summary based on CLI commands and aliases while keeping the
  existing checkpoint/replay contracts authoritative. Do not introduce a second scene
  serialization format.
- Define session lifecycle explicitly: import requires an open session and destructively replaces
  only that ephemeral session; close drops any unpersisted state; shutdown/idle also expire
  temporary bridge transfers and runtime metadata.

## Expected Touch Points

- Lab Interact daemon/driver artifact store, CLI schemas, and path confinement
- `server/src/lobby/room_task/lab/replay.rs` and a narrow local artifact handoff seam
- `server/src/main.rs` or a dedicated local-development service only if required by the bounded
  transport
- `server/crates/protocol/src/lab_replay.rs` only for a narrow validation helper, not a schema
  redesign
- client `LabClient` only for static setup plumbing or opaque artifact ids, never large bytes
- protocol, server simulation, hardening, CLI, and testing design/context documentation
- protocol/server/CLI integration and round-trip tests

## Constraints

- Do not add a public checkpoint upload/download endpoint, arbitrary path access, production
  persistence, database storage, or automatic source scenario/manifest edits.
- Do not let browser/daemon aliases change `LabCheckpointScenarioV1`, `GameCheckpointV1`,
  `LabReplayArtifactV1`, or `LabReplayOperation`.
- Do not accept legacy Lab scenario JSON or silently migrate incompatible artifacts.
- Do not print multi-megabyte replay JSON or carry it in a normal WebSocket control frame.
- Preserve room ownership of operation ordering, ticks, replay truncation, and validation.
- Keep import explicit and destructive only to the open ephemeral session; return resulting
  baseline/current tick and alias reconciliation.
- Keep every file operation confined to `target/lab-interact/` and every response bounded.

## Verification

- Round-trip an aliased two-unit setup through export, close/reopen, import, id remap, inspect, and
  recapture.
- Round-trip a Lab replay containing spawn, update/move, issue-as command, room ticks, and
  future-history truncation where practical.
- Test byte/count bounds, unsafe paths, wrong capability/session, expired ids, incompatible schema,
  map mismatch, stale aliases, duplicate operations, and interrupted-transfer cleanup.
- Run `node tests/protocol_parity.mjs` if a mirrored protocol surface changes.
- Run focused Rust tests for checkpoint/Lab replay export, validation, rebuild, and room handoff.
- Run the CLI artifact integration smoke, screenshot regression smoke, docs health, and suite
  selection verification.

## Manual Testing Focus

- Export a stationary setup, close and reopen the session, import it, and recapture the same scene.
- Export a short movement/attack replay, reopen it, seek through it, and confirm aliases and camera
  state remain understandable.
- Try an external path and incompatible artifact and confirm neither reads nor mutates unintended
  files.

## Handoff

After implementation, mark this phase done and report CLI grammar/results, local artifact
transport, environment/capability gate, alias sidecars/remaps, byte/count limits, lifecycle cleanup,
validation evidence, and exact setup/replay round trips. Tell Phase 5 which reopened session/replay
state is safe to record and which artifact-lifetime limits its manifests must explain.
