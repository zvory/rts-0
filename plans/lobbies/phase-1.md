# Phase 1 - Server Lobby Summary Contract

Status: planned.

## Goal

Add a server-owned lobby browser contract that can list normal rooms before a client joins one and
can atomically create a named lobby without accidentally joining an existing room.

This phase should treat the room-policy refactor as the current baseline. Build the summary and
create paths around `Lobby`, `RoomEvent`, `RoomMode`, and `SessionPolicy` instead of adding another
mode/status classifier next to the policy layer.

## Scope

- Define a compact lobby summary DTO for browser rows.
- Add a bounded way for the lobby registry to collect current state from room tasks.
- Expose summaries to the client through a low-latency route or message path suitable for 1-2
  second refreshes.
- Add an atomic create-lobby path that rejects existing names, reserved/internal prefixes, invalid
  lengths, empty names, and deploy-drain creation attempts.
- Track room creation time so summaries can show relative age.
- Include in-progress rooms in the summary output, but mark them non-joinable for the browser.
- Keep full waiting rooms visible and mark them spectator-joinable.
- Hide internal rooms from the public summary: dev self-play, dev scenarios, match replay rooms,
  saved replay artifact rooms, replay branch rooms, and lab rooms.

## Expected Summary Shape

The exact Rust names can change, but every browser row should have enough information to render
without inspecting joined-lobby payloads:

```text
LobbySummary {
  room: String,
  host_name: Option<String>,
  map: String,
  created_at_unix_ms: u64,
  occupied_slots: usize,
  max_slots: usize,
  spectator_count: usize,
  phase: Lobby | Countdown | InGame,
  join_state: Open | FullSpectatorOnly | Starting | InGame | Stale,
}
```

Notes:

- `occupied_slots` is active humans plus AI. It excludes spectators.
- `host_name` is the current host's display name. If there is no current host, the room should not
  normally appear unless implementation finds a valid transitional reason.
- `map` is `selected_map` in waiting/countdown rooms and `match_map_name` in in-game rooms.
- `created_at_unix_ms` is room creation time, not host join time.
- `join_state` is server-authored display/action state. The client may gray or sort rows, but the
  server remains authoritative at join time.
- Replay, replay-artifact, replay-branch, dev, and lab rooms should not produce public summary
  rows. If a summary request reaches one of those room tasks, it should reply with `None` or the
  registry should skip it based on a policy-derived public-browser flag.

## Suggested Architecture

- Add a `created_at` field to `RoomTask`.
- Add a `RoomEvent::Summary { reply }` request/reply event, or an equivalent watch-backed summary
  channel, so `Lobby` can ask room tasks for browser-safe state without reading their internals.
  The room task should build the summary from its current `SessionPolicy`, phase, host/player
  state, `selected_map`, `match_map_name`, and `match_countdown_deadline`.
- Add `Lobby::summaries()` that gathers summaries with a short timeout and skips dead/internal
  rooms. If a room task is gone, prune it only through a deliberate registry cleanup path.
- Add `Lobby::create_lobby(room)` or a clearly named equivalent that creates only when absent and
  creates explicitly as `RoomMode::Normal`. It should return distinct errors for duplicate,
  invalid/reserved name, and drain rejection.
- Prefer reusing existing join handling after successful creation so room membership semantics stay
  in one place.
- Keep the existing `get_or_create_join_target` behavior for lower-level joins/tests unless a later
  phase deliberately changes that contract. Browser create must not call it because duplicates are
  an error.
- If using HTTP for the list, add `GET /api/lobbies`. If using WebSocket for the list, add mirrored
  protocol tags and document them in `docs/design/protocol.md`.
- If adding a create WebSocket message, use a name such as `createLobby` with the same fields as
  `join` plus create-only semantics. The server should send a normal `lobby` message after the
  room accepts the creator.
- Validate reserved names against the current internal room prefixes and mode parser, including
  `__dev_scenario__:`, `__replay_artifact__:`, `__match_replay__`, `__replay_branch__`, and
  `__lab__:`. Future non-normal room modes should be hidden and uncreatable through this browser by
  default.

## Touch Points

- `server/src/lobby/mod.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/session_policy.rs` if a small public-browser classification helper is added
- `server/src/main.rs`
- `server/crates/protocol/src/lib.rs` if a WebSocket create/list message is added
- `server/src/protocol.rs` if protocol adapters are affected
- `client/src/protocol.js` only if this phase adds WebSocket tags/builders
- `docs/design/protocol.md` for any WebSocket contract change
- A server design doc or route section for `GET /api/lobbies` if the list is HTTP
- `docs/context/protocol.md` or relevant capsule if section lists change

## Constraints

- Do not change the joined-lobby `lobby` payload shape unless necessary.
- Do not make the list endpoint block indefinitely on a stuck room task. Use a short timeout and a
  clear fallback.
- Do not expose internal room names or replay/dev rooms in the public browser.
- Do not let create-lobby names use internal prefixes such as `__dev_scenario__:`,
  `__replay_artifact__:`, `__match_replay__`, `__replay_branch__`, or `__lab__:`.
- Do not silently turn duplicate create into join. Duplicate create must fail so the modal can show
  a clear error.
- Existing tests that join by room name may keep using the protocol. This phase changes the product
  UI path, not every internal test helper.

## Verification

- Add focused Rust tests for:
  - summary state for open waiting rooms
  - full waiting rooms marked spectator-joinable
  - countdown/starting rooms included but not active-joinable
  - in-game rooms included but non-joinable
  - hidden internal room modes/prefixes, including lab and saved replay artifact rooms
  - duplicate create rejected
  - invalid/reserved names rejected
  - drain rejects new create while existing room joins still work
- Run the focused Rust tests that cover `server/src/lobby` and any main route tests touched.
- Run `node tests/protocol_parity.mjs` if any WebSocket protocol tag or DTO changes.

## Manual Testing Focus

- Start the server, create or join a normal room through an existing client/test helper, and confirm
  the browser endpoint/message shows host, map, age source, slots, spectators, and join state.
- Fill a waiting room and confirm it remains visible as spectator-joinable.
- Start a match and confirm the row remains visible but disabled.
- Try to create an existing, empty, too-long, and reserved-prefix lobby name and confirm each is
  rejected clearly.

## Handoff

Mark this phase done in the implementation commit. Summarize the final DTO shape, whether the list
uses HTTP or WebSocket, the exact create-lobby API, and any summary collection timeout or pruning
behavior the client must account for.
