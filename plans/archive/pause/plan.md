# Live Match Pause Plan

## Purpose

Add a server-authoritative pause control for live matches. Each active player gets three successful
pause initiations per match, and any active player can unpause the match. The UI should expose Pause
beside Give up in the gear menu without confirmation, then show a centered Game Paused overlay with
an Unpause button while the server says the live match is paused.

## Overall Constraints

- Keep live-match pause separate from `roomTime` pause. `roomTime` is currently the replay/dev-watch
  clock-control surface; live pause is a match control for fixed-realtime games.
- Preserve the room policy model. Session policy should advertise whether live pause controls are
  available, while `RoomTask` owns the actual pause state, pause counts, connected-player authority,
  branch-live seat aliases, and tick skipping.
- Preserve server authority. The client may optimistically disable buttons for ergonomics, but the
  overlay, remaining pause count, and paused state must be driven by reliable server state.
- Do not advance simulation, AI thinking, command acknowledgement, production, combat, cooldowns, or
  match resolution while paused. Pings, net reports, reliable unpause messages, disconnect handling,
  and score/game-over messages must still work.
- Do not let spectators, replay viewers, dev-watch viewers, lab viewers, or lobby users spend live
  pauses. Branch-live players may use live pause only if the implementation deliberately maps their
  connection to an active original seat through the existing branch-live issuer path.
- Reset pause state and per-player counters on every new live match and when a room returns to a
  clean lobby. Do not leak pause counts across rematches.
- Keep protocol mirrors in lockstep: Rust wire DTOs, `client/src/protocol.js`, protocol docs, and
  protocol parity tests must agree on every new tag and field.
- Keep the settings UI metadata-driven. Do not infer pause affordances from raw room mode names;
  consume start-payload capability metadata parsed by `room_capabilities.js`.
- Keep the overlay as ordinary DOM owned by the match shell with teardown in `Match.destroy()`.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head is reachable
  from `origin/main`.
- After each phase, the implementing agent must provide a handoff message naming exact verification,
  behavior affected, remaining risks, next-phase guidance, and the core features that should be
  manually tested.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phase Summary

### [Phase 1 - Live Match Pause](phase-1.md)

Ship the full live pause loop in one PR because the protocol, room task behavior, and UI need to
change together for a usable feature. This phase should add live pause/unpause messages, policy
capability metadata, server-owned pause counters, reliable live pause state fanout, tick skipping,
the gear-menu Pause action, and the centered Game Paused overlay. It should include focused
server/protocol/client tests plus protocol, server-sim, and client-ui documentation updates.

## Phase Index

1. [Phase 1 - Live Match Pause](phase-1.md)

## Non-Goals

- Do not implement replay, dev scenario, lab, or lobby pause controls.
- Do not reuse `setRoomTimeSpeed` or `roomTime.pause` for live matches.
- Do not add host-only pause authority; any active live player can pause while they have remaining
  pauses, and any active live player can unpause.
- Do not add a pause confirmation dialog.
- Do not add a pause countdown, vote, timeout, automatic resume, or pause reason in this phase.
- Do not change simulation tick rate, snapshot cadence, command validation, or match-history
  persistence beyond skipping live simulation work while paused.
- Do not bump compact snapshot schema just to carry pause state; prefer a reliable control-plane
  server message unless implementation evidence shows snapshot state is necessary.

## Implementation Process

This is a one-phase plan. For unattended executor work, use an explicit phase id:

```bash
scripts/phase-runner.sh --plan pause phase-1 --pr --wait
```

Do not report the phase complete until the phase PR is merged and the phase head is reachable from
`origin/main`.
