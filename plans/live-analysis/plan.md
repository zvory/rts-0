# Observer Analysis and Input Plan

## Purpose

Unify the existing replay analysis overlay and payload into one observer analysis system for replay
viewers and live spectators, and fix the replay-only camera input drift introduced by the dedicated
replay viewer. The goal is one shared observer camera/navigation foundation, one server-authored
analysis stream, one client overlay, and one protocol contract that no longer describes the feature
as replay-only. Active players must not receive global analysis data during live matches, and replay
viewers must keep the same camera navigation affordances as normal match views unless a difference
is explicitly documented.

## Phase Summaries

Phase 1 extracts and documents shared observer camera/navigation input before more observer UI is
mounted. It fixes replay middle-mouse drag panning, keeps replay viewers command-free, preserves
live spectator inspection behavior, and adds client coverage so wheel zoom, keyboard/edge state,
and drag panning do not drift again. The outcome is a stable input foundation that later observer
overlay work can rely on without duplicating camera controls.

Phase 2 renames and documents the shared analysis contract without changing who receives data. It
keeps the existing `replayAnalysis` wire tag unless the implementation proves a rename is worth the
compatibility churn, but updates Rust/JS names, comments, and docs so the feature is "observer
analysis" rather than "replay analysis." The outcome is one clearly documented contract that can be
used by replay viewers and live spectators.

Phase 3 wires live spectator delivery into the server tick path. It computes the analysis payload
once per live tick only when live spectators are present, sends it only to spectator connections,
and preserves the existing replay seek/playback behavior. The outcome is server-authored live
analysis data available to spectators without exposing it to active players.

Phase 4 generalizes the client overlay and hardens the full observer experience. It mounts the same
analysis overlay for replay viewers and live spectators on top of the shared observer input
foundation, removes replay-only UI copy where visible, and verifies teardown, responsive layout,
camera/minimap/settings interaction, and basic live spectator behavior. The outcome is one observer
analysis overlay for both live games and replays without reintroducing replay-only camera
regressions.

## Phase Index

1. [Phase 1 - Shared Observer Camera Input](phase-1.md)
2. [Phase 2 - Shared Observer Analysis Contract](phase-2.md)
3. [Phase 3 - Live Spectator Analysis Delivery](phase-3.md)
4. [Phase 4 - Unified Client Overlay](phase-4.md)

## Overall Constraints

- Keep fog and privacy boundaries explicit. Live active players must never receive all-player
  analysis data, production queues, global unit inventory, or global loss data through this system.
- Treat observer input as a shared client surface. Replay viewers, live spectators, dev self-play
  watchers, and active players should share camera/navigation primitives where possible; differences
  such as gameplay commands, replay seek controls, pointer lock, and selection inspection must be
  explicit mode policy, not copy-pasted input logic.
- Fix the known replay middle-mouse drag regression in Phase 1 before mounting any broader observer
  overlay. Do not work around it in the overlay phase by adding another replay-only input path.
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
- Preserve client teardown discipline. Shared input helpers and the overlay must destroy
  DOM/listeners in every path where `Match.destroy()` or branch staging freeze tears down the game
  UI.
- Use focused verification during implementation. Relevant checks include protocol parity, client
  contracts, client architecture, focused Rust room-task tests, and one live server spectator flow.
- Coordinate write ownership. Phase 1 and Phase 4 both touch `client/src/match.js`, input modules,
  client contracts, and `docs/design/client-ui.md`; implement them sequentially from this plan
  rather than in parallel worktrees.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do and what should be manually tested. Manual testing notes should cover the core
features for that phase, not an exhaustive test matrix.
