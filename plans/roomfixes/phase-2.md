# Phase 2 - Branch Room Admission And Identity

## Phase Status

- [x] Done.

## Objective

Fix replay branch room admission so branch staging and branch-live matches keep separate join
behavior, and preserve internal branch room identity after empty-room cleanup. This phase should
only address the branch-live late-join routing bug and the `__replay_branch__` room identity issue.

Before running this phase, confirm PR #257 and PR #258 are merged into `main`, then confirm Phase 1
has merged. Build on the merged spectator join shape from `plans/spectator`; do not duplicate
spectator admission, spectator payload, or lobby-browser work from that plan.

## Work

- Split branch-room join handling by policy instead of treating every branch join as staging:
  - `SessionMode::ReplayBranch + SessionPhase::LiveMatch` correctly maps to
    `JoinPolicy::BranchLiveAttach`;
  - `SessionPolicy::uses_branch_room_join()` currently groups `JoinPolicy::BranchStaging` and
    `JoinPolicy::BranchLiveAttach` together;
  - `RoomTask::on_join()` routes both cases to `on_join_branch_staging()`;
  - `on_join_branch_staging()` can replace any non-staging phase with `Phase::BranchStaging` by
    rebuilding from the branch seed, so a late join to an active branch match can demote that live
    branch back into staging.
- Add an explicit branch-live attach path for late joins to active replay branch matches. It should
  keep `Phase::InGame`, preserve `branch_live_seat_by_connection`, and use the merged late
  spectator shape for observer admission instead of creating a staging occupant.
- Keep branch staging joins unchanged for frozen branch rooms that have not launched yet. Staging
  occupants should still see `BranchStaging`, claim/release seats, and let the host launch after all
  original seats are claimed.
- Fix internal replay branch room identity after the last viewer leaves:
  - branch rooms are created with the private `__replay_branch__` prefix;
  - empty cleanup currently can mutate `RoomMode::ReplayBranch` to `RoomMode::Normal`;
  - that makes admission, cleanup behavior, and public lobby listing ambiguous for an internal room
    name that should never become a normal lobby.
- Choose one clear identity outcome for empty branch rooms and test it: either keep
  `RoomMode::ReplayBranch` private until the task exits, or expire the internal room/task instead of
  turning it into a public normal room. Do not make `__replay_branch__` rooms appear in public lobby
  summaries.
- Add focused Rust coverage proving branch-live late joins do not change phase or branch seat
  mappings, branch staging joins still initialize staging, and empty branch cleanup no longer
  decays into a public normal room.
- Keep the fix server-side unless a small client assertion is needed for manual testability.

## Expected Touch Points

- `server/src/lobby/session_policy.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/mod.rs` if internal-room summary or cleanup behavior needs a small helper
- `server/src/main.rs` only if branch-room routing from replay branch creation needs a narrow
  adjustment
- focused Rust tests in `server/src/lobby/room_task.rs` and/or `server/src/lobby/session_policy.rs`
- `docs/design/server-sim.md` only if the implementation changes the documented room lifecycle or
  `Game` API seam

## Implementation Checklist

- [x] Confirm PR #257 and PR #258 are merged before starting implementation.
- [x] Confirm Phase 1 has merged and start from fresh `origin/main`.
- [x] Inspect the merged spectator late-join path and reuse its live observer shape where branch
      live attach needs spectator admission.
- [x] Stop using one boolean helper that groups `BranchStaging` and `BranchLiveAttach` into the
      same branch-staging handler.
- [x] Add or route to a branch-live attach handler that never rewrites `Phase::InGame` to
      `Phase::BranchStaging`.
- [x] Preserve branch-live command identity mappings in `branch_live_seat_by_connection`.
- [x] Preserve branch-staging join behavior for unlaunched replay branch rooms.
- [x] Fix empty internal branch room cleanup so `__replay_branch__` rooms cannot become public
      normal lobbies.
- [x] Add regression tests for branch-live late join, branch-staging join, and empty branch-room
      identity cleanup.
- [x] Avoid replay start capability, lab collaboration, lab drain, room-time client, and
      start-payload builder refactors.
- [x] Run focused verification and record exact commands.
- [x] Mark this phase file done in the implementation commit.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server session_policy_classifies_replay_branch_staging_and_live -- --nocapture`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay_branch_room_join_initializes_staging_and_broadcasts_seats -- --nocapture`
- Run the exact added branch-live late-join and branch-room identity test names. Do not count a
  zero-test filter as passed.
- `git diff --check`

Do not run broad bundles by default. Rely on the PR `./tests/run-all.sh` gate for full-suite
coverage unless the implementation touches a wider contract.

## Manual Test Focus

From a replay, create a practice branch, join the branch staging room, claim all seats, and launch
the branch match. While the branch is live, join the same branch room from another browser/session
and confirm it attaches as a live observer without returning the room to staging or clearing active
seat mappings. Then leave all viewers and confirm the internal `__replay_branch__` room does not
show up as a normal public lobby and does not accept normal lobby behavior under that private name.

## Handoff Expectations

Summarize the new branch admission split, the empty-room identity outcome chosen for internal branch
rooms, and the exact regression tests that cover both. Include focused verification commands and
manual branch-live late-join results, and call out any remaining roomfixes phases without pulling
spectator or replay start capability work into this phase.
