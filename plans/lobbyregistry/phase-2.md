# Phase 2 - Public Lobby Deletion Semantics

Status: not started

## Goal

Make public normal lobby names stop being occupied by empty hidden room shells. Abandoned
create-lobby reservations and normal empty public rooms should be removed from the registry, so a
later create request starts from a genuinely absent name.

## Scope

- Replace the current immortal empty-room behavior for normal public rooms with registry disposal.
- Give `POST /api/lobbies` a short creator-join deadline if needed, but expiration must delete the
  room from the registry rather than reclaiming it through duplicate-create handling.
- Clear the room from the registry when the last human leaves a normal public lobby, a normal
  one-player sandbox, or a completed normal match's post-match room after all viewers leave.
- Preserve immediate duplicate protection while a newly-created room is still within its join
  deadline or has occupants.
- Keep deploy drain behavior intact: existing occupied rooms stay joinable during drain, new names
  are rejected, and disposable empty rooms may still be cleaned up.
- Add focused server tests for:
  - `alex's lobby` style apostrophe names;
  - abandoned create-lobby reservation disappears and can be created again;
  - immediate duplicate create still rejects;
  - joined lobbies remain visible and duplicate-protected;
  - empty normal rooms no longer appear in summaries or occupy the name.

## Touch Points

- `server/src/lobby/mod.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/tests.rs`
- `tests/lobby_browser_integration.mjs` if the live HTTP/WebSocket flow needs an end-to-end
  regression.

## Out Of Scope

- Do not change replay, replay branch, lab, or dev room cleanup in this phase except where normal
  post-match replay cleanup is part of a normal room's lifecycle.
- Do not introduce a client-only workaround for duplicate errors.
- Do not restore the PR #264 reclaim-on-duplicate implementation.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server create_lobby`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lobby_summaries_collect_browser_safe_rows_from_room_tasks`
- With a local server: `RTS_WS=ws://127.0.0.1:<port>/ws node tests/lobby_browser_integration.mjs`
- Manual local HTTP/WebSocket repro for `alex's lobby`.
- `git diff --check`

## Manual Testing Focus

Open the lobby browser, create `alex's lobby`, close or interrupt before joining if practical, wait
past the join deadline, then create the same lobby name again. Also create a normal lobby, join it,
leave it empty, and confirm the same name can be created again while occupied lobbies still reject
duplicates.

## Handoff

After this phase, report the join-deadline behavior, the exact deletion trigger for empty normal
rooms, and the local/browser verification performed. Tell the next agent which non-normal room modes
still keep registry entries after becoming empty.
