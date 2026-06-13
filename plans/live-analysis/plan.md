# Live Analysis Plan

## Purpose

Unify the existing replay analysis overlay and payload into one observer analysis system for replay
viewers and live spectators. The goal is one server-authored analysis stream, one client overlay,
and one protocol contract that no longer describes the feature as replay-only. Active players must
not receive global analysis data during live matches.

## Phase Summaries

Phase 1 renames and documents the shared analysis contract without changing who receives data. It
keeps the existing `replayAnalysis` wire tag unless the implementation proves a rename is worth the
compatibility churn, but updates Rust/JS names, comments, and docs so the feature is "observer
analysis" rather than "replay analysis." The outcome is one clearly documented contract that can be
used by replay viewers and live spectators.

Phase 2 wires live spectator delivery into the server tick path. It computes the analysis payload
once per live tick only when live spectators are present, sends it only to spectator connections,
and preserves the existing replay seek/playback behavior. The outcome is server-authored live
analysis data available to spectators without exposing it to active players.

Phase 3 generalizes the client overlay and hardens the full observer experience. It mounts the same
analysis overlay for replay viewers and live spectators, removes replay-only UI copy where visible,
and verifies teardown, responsive layout, and basic live spectator behavior. The outcome is one
observer analysis overlay for both live games and replays.

## Phase Index

1. [Phase 1 - Shared Observer Analysis Contract](phase-1.md)
2. [Phase 2 - Live Spectator Analysis Delivery](phase-2.md)
3. [Phase 3 - Unified Client Overlay](phase-3.md)

## Overall Constraints

- Keep fog and privacy boundaries explicit. Live active players must never receive all-player
  analysis data, production queues, global unit inventory, or global loss data through this system.
- Treat this as a protocol contract change. Update `server/crates/protocol/src/lib.rs`,
  `client/src/protocol.js`, and `docs/design/protocol.md` together whenever message shape, naming,
  or semantics change.
- Prefer one observer analysis payload and one overlay implementation. Avoid adding a separate
  live-spectator overlay or a parallel live-only DTO unless a concrete privacy or performance issue
  requires it.
- Keep replay seek correctness intact. Replay analysis must still be recomputed from the current
  authoritative replay `Game` state after seeks, vision changes, and playback ticks.
- Keep live delivery spectator-only and room-local. Do not persist analysis payloads to match
  history and do not make them available through active-player snapshots.
- Avoid unnecessary bandwidth churn. If the implementation sends at 30 Hz, compute once per tick
  and share the payload across spectators; if this proves expensive, throttle observer analysis
  rather than changing snapshot cadence.
- Preserve client teardown discipline. The overlay must destroy DOM/listeners in every path where
  `Match.destroy()` or branch staging freeze tears down the game UI.
- Use focused verification during implementation. Relevant checks include protocol parity, client
  contracts, client architecture, focused Rust room-task tests, and one live server spectator flow.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do and what should be manually tested. Manual testing notes should cover the core
features for that phase, not an exhaustive test matrix.

