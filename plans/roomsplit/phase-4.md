# Phase 4 - Lab And Dev Mode Split

Status: not started.

## Goal

Move lab-room and dev-watch mode bodies into dedicated child modules without changing lab mutation
authorization, dev scenario behavior, or projection policy.

## Scope

- Move `LabSession`, lab operation log state, lab start metadata helpers, lab joins, lab session
  launch, lab start payload sends, lab snapshot projection inputs, lab request handling, lab vision,
  lab issue-as command routing, lab mutation routing, lab scenario export/import, lab results, and
  lab state broadcast into `room_task/lab.rs`.
- Move dev-watch joins, dev session launch, dev game construction, dev scripted driver glue, dev
  start payload sends, dev ticks, dev errors, and dev watch state broadcast into
  `room_task/dev.rs`.
- Continue using existing helpers such as `dev_replay.rs`, `projection.rs`, `snapshot_fanout.rs`,
  `launch.rs`, `tick_control.rs`, and public `Game` lab APIs.
- Keep accepted lab mutation and issue-as calls inside room-task ownership; this phase moves their
  source file, not their authority boundary.
- Update `scripts/check-lobby-architecture.mjs` only if the lab mutation centralization check needs
  to recognize `room_task/lab.rs` as the new approved location.

## Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/room_task/lab.rs`
- `server/src/lobby/room_task/dev.rs`
- `scripts/check-lobby-architecture.mjs` if the lab mutation allowlist needs a precise update
- Lab/dev-focused room-task tests
- `plans/roomsplit/phase-4.md`

## Constraints

- Do not change lab operator/read-only role rules, lab operation log behavior, lab result routing,
  lab projection, lab scenario export/import shape, or lab issue-as authorization.
- Do not change dev scenario setup, pause/step behavior, room-time controls, saved self-play replay
  loading, or full-world dev-watch projection.
- Do not move lab mutation APIs into lower crates or generic helpers outside room-task ownership.
- If protocol-visible lab or dev messages move in a non-mechanical way, stop and read
  `docs/context/protocol.md` before continuing.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `node scripts/check-lobby-architecture.mjs`
- `node tests/protocol_parity.mjs` if lab/dev message construction changes beyond pure movement
- `git diff --check`

## Manual Testing Focus

Manually check a lab room for first-join operator role, collaborator/read-only permissions, mutation
result delivery, issue-as commands, lab state broadcast, and scenario export/import. Manually check a
dev scenario room for start payload, pause/step, speed controls, and full-world snapshot visibility.

## Handoff

After implementation, mark this phase done and summarize the lab/dev module boundaries, commands
run, any checker allowlist changes, manual checks performed or still needed, and any lab or dev
helpers deliberately left in the root file.
