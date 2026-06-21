# Phase 3 - Internal Room Cleanup And Browser Coverage

Status: not started

## Goal

Finish the cleanup policy for non-normal room modes and lock the lobby browser behavior with
end-to-end coverage. Internal rooms should either be explicitly disposable when empty or documented
as intentionally retained with a concrete reason.

## Scope

- Audit replay, persisted replay, replay branch, lab, and dev scenario rooms against the new
  registry-disposal primitive.
- Delete empty internal rooms when their URL or room id can safely recreate the state later.
- Keep any intentionally retained internal room mode private and document why it cannot be removed
  yet.
- Ensure empty branch/replay/lab/dev rooms do not turn into public normal lobby rows.
- Strengthen `tests/lobby_browser_integration.mjs` or nearby live suites so browser rows remain
  correct across abandoned reservations, empty normal cleanup, occupied public rooms, in-game rows,
  spectator joins, and internal-room isolation.
- Update `docs/design/server-sim.md`, `docs/design/protocol.md`, or context capsules only if the
  implemented lifecycle policy changes documented room behavior.

## Touch Points

- `server/src/lobby/mod.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/session_policy.rs`
- `server/src/lobby/tests.rs`
- `tests/lobby_browser_integration.mjs`
- Relevant docs only if lifecycle contract text changes.

## Out Of Scope

- Do not redesign the lobby browser UI.
- Do not change the wire protocol unless the implementation truly needs a new server/client field.
- Do not move `Game` ownership out of the room task.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lobby`
- With a local server: `RTS_WS=ws://127.0.0.1:<port>/ws node tests/lobby_browser_integration.mjs`
- `node tests/server_integration.mjs` if replay or post-match replay lifecycle behavior changes.
- `node tests/regression.mjs` if rejected joins, stale rooms, or robustness paths are touched.
- `git diff --check`

## Manual Testing Focus

Create and empty a normal lobby, inspect the public lobby browser, open a replay or lab URL if the
phase touched that mode, and confirm internal rooms never show as public lobbies. Also check that a
full or in-progress public room remains visible and correctly disabled while occupied.

## Handoff

After this phase, report which room modes are disposable, which are intentionally retained, the
browser/integration tests that cover the policy, and any docs updated. Call out any remaining
internal room mode that still needs a follow-up cleanup plan.
