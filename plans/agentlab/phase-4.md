# Phase 4 - Portable Setup And Replay Artifacts

Status: planned.

## Prerequisite

Do not start this phase until the Phase 3 MVP review gate explicitly approves continuing and any
tool-vocabulary revisions have landed.

## Goal

Let agents save and reopen useful tiny scenes without hand-authoring checkpoint internals. Static
state should use the existing checkpoint-backed lab setup contract, while timed lab mutations and
orders should use the existing portable lab replay contract.

## Scope

- Add `lab_export` and `lab_import` tools with an explicit artifact kind of `setup` or `replay`.
  Store artifacts and alias sidecars under the session's ignored target directory by default.
- For static setup export/import, reuse `LabClient.exportScenario` / checkpoint scenario import and
  the server's existing `LabCheckpointScenarioV1` validation. Do not expose the embedded
  `checkpointPayload` to model context; return metadata, counts, safe resource/path references, and
  validation results.
- Promote the existing server-side `export_lab_replay_artifact` and
  `load_lab_replay_artifact` capabilities through a bounded local agent path. Long replay
  artifacts must not be embedded in the current WebSocket lab result because the protocol contract
  already reserves them for a local-file or future artifact path.
- Choose a local-only, env-gated artifact bridge owned by the MCP-started server/driver. It should:
  - be unavailable in ordinary production startup;
  - bind only to loopback and require an unguessable session capability;
  - use opaque artifact ids rather than browser-supplied paths;
  - enforce existing 8 MiB replay, 4 MiB checkpoint, operation-count, payload, and schema bounds;
  - transfer validated bytes between the room-owned exporter/importer and the driver;
  - expire in-memory/temp artifacts and remove them during teardown.
- If implementation evidence supports a safer equally bounded local transport, document the
  alternative before using it. Do not reconstruct the authoritative replay stream from an
  MCP-side best-effort log when the room already owns accepted operation ticks and request order.
- Persist session aliases in a small agent-lab sidecar keyed to artifact identity, not inside the
  protocol schemas. On setup import, reconcile aliases through `sourceEntityIdMap`; on replay import,
  validate aliases against the restored initial setup/current state and report stale entries.
- Add artifact inspection that returns authoring metadata, map, tick/duration, entity/operation
  counts, build compatibility, aliases, and validation status without returning the full JSON to
  the model.
- Allow `lab_open` or `lab_import` to reopen an artifact from an artifact id/path rooted only under
  the selected worktree's agent-lab target area. Do not accept arbitrary filesystem paths or URLs.
- Add a small, optional human-readable reproduction summary generated from tool calls/aliases, but
  keep the authoritative artifact as the existing checkpoint/replay contract. Do not introduce a
  second scene serialization format in this phase.

## Expected Touch Points

- Agent Lab MCP/driver artifact store and tool schemas
- `server/src/lobby/room_task/lab/replay.rs` and a narrow local artifact handoff seam
- `server/src/main.rs` or a dedicated local-dev artifact service only if needed for the chosen
  bounded transport
- `server/crates/protocol/src/lab_replay.rs` only if existing artifact validation needs a narrow
  public helper, not a schema redesign
- client `LabClient` only for static setup plumbing or opaque artifact ids, never large replay bytes
- `docs/design/protocol.md`, `docs/design/server-sim.md`, `docs/design/hardening.md`, and relevant
  capsules for the local artifact boundary
- protocol/server/MCP integration tests

## Constraints

- Do not add a public generic checkpoint upload/download endpoint, arbitrary path reads/writes,
  production persistence, database storage, or automatic source scenario/manifest edits.
- Do not let browser/MCP aliases change `LabCheckpointScenarioV1`, `GameCheckpointV1`,
  `LabReplayArtifactV1`, or `LabReplayOperation` schemas.
- Do not accept legacy lab scenario JSON or silently migrate incompatible artifacts.
- Do not carry multi-megabyte replay JSON in an MCP text result or normal WebSocket control frame.
- Preserve room ownership of accepted operation ordering, ticks, replay truncation, and artifact
  validation.
- Keep artifact import explicit and destructive only to the named ephemeral session; return the
  resulting baseline/current tick and alias reconciliation.

## Verification

- Add round-trip coverage for a static aliased two-unit setup exported, reset/closed, imported, and
  inspected with remapped ids.
- Add round-trip coverage for a lab replay containing spawn, move/update, issue-as command, room
  ticks, and future-history truncation where practical.
- Test artifact bounds, unsafe paths, wrong capability/session, expired ids, incompatible schema,
  map mismatch, stale aliases, duplicate operations, and interrupted transfer cleanup.
- Run `node tests/protocol_parity.mjs` if any mirrored protocol surface changes.
- Run focused Rust tests for checkpoint/lab replay export, validation, rebuild, and room artifact
  handoff.
- Run the MCP artifact integration smoke and `node scripts/check-docs-health.mjs`.

## Manual Testing Focus

- Build and export a stationary setup, close everything, reopen it in a new session, and recapture
  the same scene.
- Build a short movement/attack session, export a lab replay, reopen it, seek through it, and confirm
  aliases and camera setup remain understandable.
- Try an unsafe external path and an incompatible artifact and confirm both fail without reading or
  mutating unintended files.

## Handoff

After implementation, mark this phase done and report the local artifact transport, env/capability
gate, MCP schemas, alias sidecar/remap behavior, byte/count limits, validation evidence, and the
exact static/replay round trips. Tell Phase 5 which reopened replay/session state is safe to record
and any artifact lifecycle limitations video manifests should reference.
