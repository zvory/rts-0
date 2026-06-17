# Phase 2 - Session Policy Descriptor

## Phase Status

- [x] Done.

## Objective

Introduce a neutral internal descriptor for existing room/session policy choices without changing
any runtime behavior.

## Work

- Add a lobby-local module for session policy naming, for example `server/src/lobby/session_policy.rs`.
- Represent today's mode and phase choices in neutral terms such as state source, join policy,
  clock capability, authority policy, vision policy, mutation policy, persistence policy, and
  start-payload category.
- Replace small scattered checks only when the new descriptor can express the exact current branch:
  live dev watch detection, replay-room detection, branch-staging detection, match-history
  persistence eligibility, countdown eligibility, and tick interval speed source are candidate
  sites.
- Keep the existing `RoomMode` and `Phase` enums unless a narrow rename is purely internal and all
  call sites stay behavior-equivalent.
- Do not introduce lab variants or new protocol messages.

## Expected Touch Points

- `server/src/lobby/session_policy.rs` or similarly named lobby-local module
- `server/src/lobby/mod.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/tests.rs`
- `docs/design/server-sim.md` only if the room architecture description becomes stale

## Implementation Checklist

- [ ] Add explicit policy names for every current mode from the Phase 1 matrix.
- [ ] Convert a limited set of duplicated mode checks to policy reads.
- [ ] Preserve all existing room modes and client-visible behavior.
- [ ] Add focused tests that prove the descriptor classifies normal, replay, branch, and dev modes
      the same way the old checks did.
- [ ] Leave larger call-site migrations for later phases if they would make this phase broad.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server session_policy`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `git diff --check`

If no `session_policy` test filter exists yet, add narrowly named tests in this phase and use that
filter in the handoff.

## Manual Test Focus

Normal lobby start, persisted replay join, replay branch staging room join, and dev watch speed or
pause controls.

## Handoff Expectations

List every old mode check converted to the descriptor, every mode check intentionally left in place,
and whether the next phase can rely on the descriptor or needs one more policy case first.
