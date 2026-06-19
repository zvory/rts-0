# Phase 3 - Delta Snapshot Design Pass

## Phase Status

- [ ] Tentative. Needs review and rewriting before implementation.

## Objective

Turn the delta snapshot idea into a concrete design that another agent can implement safely. This
placeholder is not runner-ready and should not be executed by the phase runner until it has been
expanded and approved.

## Tentative Scope

- Define whether deltas are computed in the room task, writer task, or a dedicated codec layer.
- Define per-recipient baseline ownership and when a baseline is updated.
- Account for latest-only snapshot coalescing, where pending snapshots can be replaced before they are
  sent.
- Define keyframe cadence, forced keyframes, startup behavior, reconnect behavior, replay behavior,
  lab time controls, observer visibility, and spectator/full-vision rules.
- Define how the client reconstructs the full semantic snapshot shape expected by `GameState`.
- Define how to detect stale, duplicate, skipped, or unsupported delta frames and recover without
  corrupting client state.
- Preserve fog safety: no delta may reveal an entity, position, target id, event, remembered building,
  or tile state the recipient cannot see.
- Decide which correctness tests, fuzz/property tests, and replay compatibility tests are required.

## Required Follow-up Before Execution

A separate AI pass should replace this file with a fully fleshed out phase or split it into multiple
reviewable phases. That pass should read `docs/context/protocol.md`, `docs/design/protocol.md`,
`server/src/lobby/connection.rs`, `server/src/main.rs`, `server/crates/sim/src/game/snapshot.rs`,
`server/crates/protocol/src/lib.rs`, `client/src/protocol.js`, and `client/src/state.js` before
proposing implementation details.

## Handoff Expectations

If this placeholder is revised, the handoff should explain the selected baseline/keyframe model, why
it is safe with latest-only writers, and which later implementation phases are now runner-ready.
