# Phase 3 - Lab Launch Drain Policy

## Phase Status

- [x] Done.

## Objective

Make lab launch behavior explicit during deploy drain. Normal match starts and replay branch starts
already check `DrainHandle::is_draining()`, and the lobby lets existing rooms remain joinable while
draining, but a first join to an existing lab room currently auto-calls `start_lab_session()` from
`RoomTask::on_join_lab()` and only then marks the session for drain accounting.

Before running this phase, confirm PR #257 and PR #258 are merged into `main`, then confirm Phases 1
and 2 have merged. Preserve the collaborative lab operator behavior from
`plans/lab/debug-collab/phase-2.md`; this phase is about launch policy and drain accounting, not HUD,
input, minimap, or lab tool collaboration.

## Work

- Decide and encode one explicit lab drain policy:
  - preferred default: labs are authoritative live sessions, so a lab room that has not started yet
    cannot auto-start during deploy drain, and a started lab is counted until the room empties;
  - acceptable alternative only if intentionally chosen in code and tests: labs are
    non-drain-tracked tools, so they must not call live-match drain accounting and should not rely
    on hidden `RoomMode::Lab` exceptions inside generic match helpers.
- Express that decision through `SessionPolicy` or a small drain/session helper instead of scattered
  mode checks. The helper should make it obvious whether a mode can newly launch while draining and
  whether it contributes to `active_match_count()`.
- Gate the lab auto-start path before it mutates room membership into a running lab:
  - existing lab rooms may still be joinable during drain because `Lobby::get_or_create_join_target`
    permits existing rooms;
  - if the room is still in `Phase::Lobby` and lab policy forbids new starts during drain, reject or
    fail the first lab join with the same deploy-drain wording used by normal starts;
  - if a lab is already `Phase::InGame`, additional collaborators should still attach according to
    the collaborative lab rules and receive the current shutdown warning.
- Keep drain accounting paired and explicit:
  - a lab start that counts for drain must call the start-accounting path only after successful
    launch and must finish accounting on empty-room reset;
  - failed lab map/load starts must not increment `active_match_count()`;
  - empty lab room reset must clear `lab_session` and leave `RoomMode::Lab` intact.
- Add focused Rust coverage for the policy, including the current bug shape: existing room join is
  allowed during drain, but lab auto-start must not create a new authoritative session unless the
  explicit lab policy says it can.
- Do not change lab collaboration client controls, lab command authorization, lab scenario import/
  export semantics, replay branch admission, replay start payloads, or room-time UI behavior.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/session_policy.rs`
- `server/src/lobby/mod.rs` only if `DrainHandle` or `Lobby` needs a narrow helper/test hook for
  explicit lab accounting policy
- focused Rust tests in `server/src/lobby/room_task.rs`,
  `server/src/lobby/session_policy.rs`, and/or `server/src/lobby/mod.rs`
- `docs/design/server-sim.md` only if the phase changes documented room lifecycle or deploy drain
  semantics

## Implementation Checklist

- [x] Confirm PR #257 and PR #258 are merged before starting implementation.
- [x] Confirm Phases 1 and 2 have merged and start from fresh `origin/main`.
- [x] Re-read the merged `plans/lab/debug-collab/phase-2.md` result and preserve later-joiner lab
      operator behavior.
- [x] Add an explicit policy/helper for whether a room mode may launch a new authoritative session
      during drain and whether that session is drain-tracked.
- [x] Apply the launch-during-drain gate before `on_join_lab()` creates a running lab from
      `Phase::Lobby`.
- [x] Keep already-running lab joins available so collaborators can attach during drain and receive
      the shutdown warning.
- [x] Ensure lab start accounting happens only after a successful `start_lab_session()` launch.
- [x] Ensure failed lab starts and rejected lab joins during drain do not increment
      `active_match_count()`.
- [x] Ensure empty lab rooms still reset to lobby, clear `lab_session`, decrement drain accounting
      when tracked, and remain `RoomMode::Lab`.
- [x] Add regression tests for drain-blocked first lab join, already-running lab collaborator join
      during drain, lab empty-room reset/accounting, and policy classification.
- [x] Avoid client collaboration, replay, branch, room-time UI, and start-payload-builder changes.
- [x] Run focused verification and record exact commands.
- [x] Mark this phase file done in the implementation commit.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab_room_join_launches_real_game_with_lab_start_metadata -- --nocapture`
- `cargo test --manifest-path server/Cargo.toml -p rts-server empty_lab_room_resets_session_without_changing_lab_mode -- --nocapture`
- Run the exact added drain-policy test names for blocked first lab join, running-lab collaborator
  join during drain, drain accounting, and `SessionPolicy` classification. Do not count a zero-test
  filter as passed.
- `git diff --check`

Do not run broad bundles by default. Rely on the PR `./tests/run-all.sh` gate for full-suite
coverage unless the implementation touches a wider contract.

## Manual Test Focus

Start deploy drain, then try to join an existing but not-yet-started lab room. Confirm the join does
not launch a new lab session during drain, the user sees the deploy-drain rejection/error path, and
`active_match_count()` does not increase.

Start a lab before deploy drain, begin drain, then join from a second browser session. Confirm the
second session attaches as an operator with the collaborative lab controls from
`plans/lab/debug-collab`, receives the shutdown warning, and can still use shared lab operations
according to the merged collaboration policy. Leave every lab viewer and confirm the room resets for
future use, `lab_session` clears, the room remains a lab room, and drain accounting reaches zero.

## Handoff Expectations

Summarize the lab drain policy chosen, the helper or `SessionPolicy` field that encodes it, and
where lab auto-start is gated. Include exact focused verification commands, manual drain/lab join
results, and any remaining ambiguity around whether labs should be drain-tracked authoritative
sessions or intentionally non-drain-tracked tools.
