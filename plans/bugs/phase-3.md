# Phase 3 - Report API and Server Context

Status: planned.

## Goal

Expose the server API needed to create and review bug reports. The create path should validate a
bounded client diagnostic envelope, stamp authoritative server context, persist replay evidence, and
return the report id only after durable writes succeed.

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

If the live room task must provide authoritative context that HTTP cannot safely derive, add a small
bounded WebSocket request that returns a report context token, then have HTTP submit the final
report with that token.

## Touch Points

- `server/src/main.rs`
- `server/src/db.rs`
- `server/src/lobby/mod.rs` and `server/src/lobby/room_task.rs` if room-local context is required
- `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `client/src/protocol.js`, and
  `docs/design/protocol.md` only if WebSocket messages are added
- `docs/design/match-history.md` or report-specific docs
- `docs/context/match-history.md`
- `docs/context/protocol.md` if the wire surface changes

## Payload Bounds

- Bound description length.
- Bound JSON object sizes for client, network, and server-ish context supplied by the browser.
- Treat client-supplied tick/player/match/replay fields as hints unless they can be validated.
- Never let a malformed report payload panic the network path.

## Constraints

- No auth, categories, annotations, screenshots, spam controls, or privacy filtering.
- Submission is blocking. Do not return success until the report row and required replay linkage are
  persisted. For in-progress matches, required replay linkage means the report row stores
  `replay_key` and the server has registered that the report expects final replay evidence for that
  key.
- If the database is unavailable, return a visible failure; do not pretend the report was accepted.
- Do not expose arbitrary replay artifacts outside existing compatibility checks.

## Verification

- Add focused HTTP/API tests for create, list, status update, missing DB, malformed payload, and
  replay persistence failure behavior where practical.
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
