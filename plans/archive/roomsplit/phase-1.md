# Phase 1 - Test Split And Baseline

Status: done.

## Goal

Split the large in-file `room_task.rs` test module into focused child modules while preserving every
assertion and fixture behavior.

## Scope

- Read `docs/context/server-sim.md`, `docs/context/testing.md`, and the room-runtime sections of
  `plans/archive/hotspots/responsibility-map.md`.
- Convert the `#[cfg(test)] mod tests` body in `server/src/lobby/room_task.rs` into a child test
  module under `server/src/lobby/room_task/tests/`.
- Split tests by behavior family, with likely files for `lobby`, `live`, `replay`, `lab`, `branch`,
  `dev`, and `lifecycle`.
- Move shared test fixtures into `server/src/lobby/room_task/tests/support.rs` or a similarly narrow
  helper file.
- Keep production room-task behavior untouched except for the module declaration needed to load the
  split tests.
- Record the post-split root-file line count and the remaining largest test files in this phase
  document or handoff.

## Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/room_task/tests/*.rs`
- `plans/archive/roomsplit/phase-1.md`

## Constraints

- Do not rewrite assertions, change setup APIs, or collapse coverage while moving tests.
- Do not move production handlers or room state in this phase.
- Keep tests close to the behavior families they protect so future phases know which suite to run.
- Avoid creating a new oversized `support.rs`; shared helpers should stay smaller than the old test
  module and only contain genuinely shared fixtures.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lobby`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected for a pure test split. Manually inspect that tests still cover
normal lobby start, live spectators, replay playback, lab operations, branch launch, dev scenario
pause/step, match-history gating, and empty-room reset.

## Handoff

After implementation, mark this phase done and summarize the new test module map, the focused test
commands run, root/test line counts after the split, any tests deliberately left together, and the
manual smoke paths the next phases still need to keep in mind.

## Completion Baseline

- `server/src/lobby/room_task.rs`: 4,839 lines after replacing the inline test module with
  `#[cfg(test)] mod tests;`.
- Split test modules:
  - `tests/branch.rs`: 751 lines
  - `tests/dev.rs`: 121 lines
  - `tests/lab.rs`: 697 lines
  - `tests/lab_timeline.rs`: 861 lines
  - `tests/lifecycle.rs`: 206 lines
  - `tests/live.rs`: 719 lines
  - `tests/lobby.rs`: 397 lines
  - `tests/replay.rs`: 446 lines
  - `tests/support.rs`: 339 lines
- Lab timeline, seek, scenario import/export, and lab vision tests are split into
  `lab_timeline.rs` so the core `lab.rs` file does not become a new oversized test file.
