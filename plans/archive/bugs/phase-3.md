# Phase 3 - Report API and Server Context

Status: planned.

## Goal

Expose the server API needed to create and review bug reports. The create path should validate a
bounded client diagnostic envelope, stamp authoritative server context, persist replay evidence, and
return the report id only after durable writes succeed. This phase must add the explicit
`RoomReportContext` seam before any API path claims authoritative live/replay context.

## Scope

- Add a report creation endpoint or wire message.
- Add report list and minimal status-update endpoints for the review dashboard:
  - newest-first list
  - get one report by id
  - update `viewed`
  - update `resolved`
- Return a success payload with the created report id.
- Return clear failures when the database is unavailable, the report context is invalid, or replay
  expectation registration cannot be completed.
- Add `RoomReportContext`, a bounded authoritative room-task snapshot that can be requested for a
  connected player or replay viewer. It should include only report-safe fields: room, phase, map,
  `replay_key`, match id/replay id if already known, current live/replay tick, reporter seat id,
  reporter display name/faction when known, spectator/replay flags, and server receipt time.
- Route live/replay report creation through this context seam. HTTP may remain the final submission
  transport, but it must not trust client-supplied room, tick, player, replay, or match ids unless
  they were validated against `RoomReportContext`.
- Capture authoritative server context: room, current phase, server build SHA, map, `replay_key`,
  replay id when already available, match id when already available, current tick when available,
  reporter seat identity when known, and report receipt time.

## Recommended API Shape

Prefer HTTP endpoints because reports are database-backed review artifacts:

- `POST /api/bug-reports`
- `GET /api/bug-reports?limit=N`
- `GET /api/bug-reports/{id}`
- `PATCH /api/bug-reports/{id}` with only `viewed` and/or `resolved`
- `POST /api/bug-reports/{id}/replay` or a query-aware replay launch endpoint that can seek near
  the report tick

For live match and replay viewer reports, do not make this conditional: add an explicit bounded
request/reply context path. Acceptable shapes are either:

- a small WebSocket `RequestReportContext` message that returns a short-lived context token, followed
  by HTTP `POST /api/bug-reports` with that token; or
- an HTTP endpoint that calls `Lobby::capture_report_context(room, player_id)` through a room event
  and one-shot reply before creating the report.

Pick one shape during implementation and document why. The important boundary is that room state is
read by the room task and represented as a narrow value object, not reached through shared mutable
state.

Prefer a short-lived report context token captured when the report form opens. That token should
contain the server-stamped `RoomReportContext`, not mutable room references. This preserves the
product requirement that a report can still submit if the match ends after the tester opened the
form but before they pressed submit. Define token TTL, whether tokens are one-shot or reusable for
retry, payload size, and behavior when the room resets before submission.

## Touch Points

- `server/src/main.rs`
- `server/src/db.rs`
- `server/src/lobby/mod.rs` and `server/src/lobby/room_task.rs` for `RoomReportContext`
- `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `client/src/protocol.js`, and
  `docs/design/protocol.md` if a WebSocket context request/response is added
- `docs/design/match-history.md` or report-specific docs
- `docs/context/match-history.md`
- `docs/context/protocol.md` if the wire surface changes

## Payload Bounds

- Bound description length.
- Bound JSON object sizes for client and network context supplied by the browser.
- Define exact limits before exposing the endpoint: description length, total JSON byte cap,
  per-string cap, array length, object depth, key count, and whether oversize fields are rejected or
  truncated.
- Do not accept browser-supplied `server_context`; stamp it from `RoomReportContext`.
- Treat client-supplied tick/player/match/replay fields as hints unless they can be validated.
- Bound context-token lifetime and payload size if the WebSocket-token shape is chosen.
- Never let a malformed report payload panic the network path.

## Constraints

- No auth, categories, annotations, screenshots, spam controls, or privacy filtering.
- Submission is blocking. Do not return success until the report row and required replay linkage are
  persisted. For in-progress matches, required replay linkage means the report row stores
  `replay_key` and the server has registered that the report expects final replay evidence for that
  key.
- Do not let API handlers inspect or mutate room-task state directly. The room task owns room
  context and exposes it only through `RoomReportContext`.
- Do not let report creation depend on Recent Matches visibility. Hidden persisted replay rows must
  still be resolvable through report review when the evidence registry says they are available.
- If the database is unavailable, return a visible failure; do not pretend the report was accepted.
- Do not expose arbitrary replay artifacts outside existing compatibility checks.

## Verification

- Add focused HTTP/API tests for create, list, status update, missing DB, malformed payload, and
  replay persistence failure behavior where practical.
- Add room-context tests proving live match and replay viewer context is captured by the room task,
  not trusted from client payload fields.
- Add protocol mirror tests only if a WebSocket request/response is introduced.
- Run the relevant Rust test target and any touched Node API suite.

## Manual Testing Focus

- Submit a report against a live match using a direct API call or temporary harness.
- Confirm the response includes a report id only after the DB row exists.
- Confirm reports list newest first and status booleans can be toggled.
- Confirm invalid oversized payloads are rejected cleanly.

## Handoff

After implementation, mark this phase done and summarize the final API shape, request/response
payloads, bounds, and how the client should obtain the authoritative report context for Phase 4.
