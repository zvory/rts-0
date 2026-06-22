# Phase 2 - Module Skeleton And Shared Types

Status: done.

## Goal

Create the production child-module structure for `room_task` and move only low-risk shared data
types, constants, and pure helpers out of the root file.

## Scope

- Read `docs/context/server-sim.md` and the final module map in `docs/design/server-sim.md` before
  moving production code.
- Create `server/src/lobby/room_task/` production modules with a minimal first set, likely
  `types.rs` and `helpers.rs` only if both are justified.
- Move room-owned data types and constants that are not behavior handlers, such as `RoomPlayer`,
  `PendingClientCommandAck`, `AiSlot`, room-mode config types, and small pure helper functions.
- Re-export any moved types from `room_task.rs` so existing sibling modules such as `live_tick.rs`,
  `participants.rs`, and `snapshot_fanout.rs` do not churn more than necessary.
- Keep `RoomTask`, `RoomTask::run`, `handle_event`, phase transitions, joins, ticks, lab, replay,
  branch, live, lifecycle, and match-history behavior in the root file for this phase.

## Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/room_task/types.rs`
- `server/src/lobby/room_task/helpers.rs` only if pure helpers need a separate home
- Existing lobby helper imports only where moved type paths require re-exports or import updates
- `plans/roomsplit/phase-2.md`

## Constraints

- Do not move mode-specific event handlers in this phase.
- Keep moved exports `pub(super)` or narrower unless an existing sibling module already needs them.
- Do not introduce `room_task/mod.rs`; keep `room_task.rs` as the actor shell and use child modules
  below it.
- Do not change protocol shape, launch payload semantics, lobby behavior, replay behavior, lab
  behavior, or match-history policy.
- Avoid generic "utils" naming. If a helper is not clearly shared, leave it beside its handler for a
  later mode split.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lobby`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo fmt --manifest-path server/Cargo.toml --check`
- `node scripts/check-lobby-architecture.mjs`
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected for pure type/helper movement. Manually review imports and
visibility to confirm the new module skeleton does not widen the room-task API.

## Handoff

After implementation, mark this phase done and summarize the production child modules created, the
types/helpers moved, any re-exports kept for compatibility, verification commands run, and the exact
module pattern the next phase should follow for `impl RoomTask` handler moves.
