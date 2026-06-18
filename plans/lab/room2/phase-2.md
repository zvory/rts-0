# Phase 2 - Policy Bundle And Mutation Axis

## Phase Status

- [ ] Pending.

## Objective

Extend `SessionPolicy` into the complete capability bundle needed by the rest of this plan. Mutation
must be a first-class policy axis rather than a side effect of authority, vision, or product mode.

## Work

- Replace the current broad policy fields with a clearer bundle that names state source, lifecycle,
  clock capability, authority capability, mutation capability, visibility capability, diagnostic
  capability, persistence/export capability, and start/UI affordances.
- Model mutation explicitly enough to distinguish:
  - no authoritative mutation;
  - lobby-state mutation;
  - normal live gameplay commands;
  - replay playback cursor/keyframe mutation;
  - replay branch staging claims;
  - branch live gameplay through original-seat aliases;
  - dev scenario driver mutation and watch controls;
  - lab privileged setup operations;
  - lab issue-as gameplay commands;
  - lab scenario import/export and operation logging;
  - match-history and replay artifact persistence effects.
- Keep product-specific state sources and setup paths where product identity is real.
- Convert low-risk direct mode checks to policy reads only when the new policy expresses today's
  behavior exactly.
- Update policy tests so every product path in `capability-matrix.md` has a policy assertion.
- Update `docs/design/server-sim.md` and `docs/context/server-sim.md` if the named policy types or
  module responsibilities change.
- Do not rename protocol messages or client controls in this phase.

## Expected Touch Points

- `server/src/lobby/session_policy.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/participants.rs`
- `server/src/lobby/tick_control.rs`
- `server/src/lobby/projection.rs`
- `server/src/lobby/launch.rs`
- `server/src/lobby/tests.rs`
- `docs/design/server-sim.md`
- `docs/context/server-sim.md`
- `plans/lab/room2/capability-matrix.md`
- `plans/lab/room2/phase-2.md`

## Implementation Checklist

- [ ] Add or rename policy types for the full capability bundle.
- [ ] Add a first-class mutation capability with tests for every current room path.
- [ ] Move low-risk room-task decisions to policy reads.
- [ ] Document which product-mode checks intentionally remain at setup/routing edges.
- [ ] Update source-of-truth docs if policy names or responsibilities change.
- [ ] Mark this phase as done in this file.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server session_policy`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `git diff --check`

## Manual Test Focus

If runtime behavior is touched, smoke a normal live match, replay viewer, replay branch live launch,
dev scenario pause/step, and lab operator setup action. Confirm no user-facing controls changed.

## Handoff Expectations

Name the final policy types, show how mutation is represented, list every mode check converted or
left in place, and state which policy fields Phase 3 should use for room-controlled time.
