# Replay Lobby Plan

## Purpose

Make match-history replay launch create a shared spectator lobby before playback starts. Viewers
should be able to gather from the lobby browser, see only spectator slots, and let the host start
the replay without Ready/team setup. The implementation should preserve current replay playback and
branch-from-replay behavior once playback has started.

## Phase Summaries

### [Phase 1 - Server Replay Lobby Contract](phase-1.md)

Add a server-side replay staging lobby for persisted match-history replay rooms instead of starting
playback on the first confirmed join. The room should accept only spectator occupants, report itself
through `/api/lobbies`, and allow the host to start playback immediately without ready checks,
teams, AI, map selection, or active seats. This phase should update the wire/design contract and
server tests while keeping current replay playback behavior after start unchanged.

### [Phase 2 - Client Replay Lobby UI](phase-2.md)

Teach the lobby browser and joined lobby screen to recognize replay lobby rows. The browser should
show replay rooms as joinable spectator rooms, and the joined room view should hide Ready/team/map
controls while showing only spectators and an always-available host Start Match button. The Watch
Replay action should route the launching player into this staging lobby rather than immediately
auto-starting playback.

### [Phase 3 - Integration Coverage and Resume-Ready Polish](phase-3.md)

Add end-to-end coverage for creating a replay lobby, joining it from another client, starting
shared playback, and leaving without tearing down other viewers. Tighten copy, lifecycle cleanup,
and docs so replay lobbies are understandable in the lobby browser and do not regress normal lobby,
live spectator, post-match replay, or replay-branch flows. Leave a clear follow-up seam for future
"resume from replay" work without implementing playable resume in this plan.

## Overall Constraints

- Keep replay rooms spectator-only until a later replay-resume feature explicitly claims playable
  seats.
- Do not expose stored replay artifact JSON or privileged match-history data through `/api/lobbies`;
  browser rows should contain only safe room metadata.
- Keep normal lobbies, live spectator joins, post-match replay playback, dev replay artifacts, lab
  rooms, and replay branch staging behavior intact unless a phase explicitly changes them.
- Treat lobby/replay message shape changes as protocol changes: update Rust protocol DTOs, JS
  protocol mirrors if needed, `docs/design/protocol.md`, and targeted parity/contract tests.
- Avoid inferring controls from reserved room-name prefixes on the client. Prefer explicit lobby
  metadata or capabilities so the UI can distinguish normal, replay, and future room kinds.
- Keep host authority simple for this feature: first spectator in the replay lobby is host, host can
  start playback, later spectators can join playback as today.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase starts.
- When a phase is complete, mark that phase document done in the implementation commit and provide a
  handoff message describing what changed, what the next agent should do, and the core manual
  testing focus.

## Handoff Requirements

After every phase, the implementing agent must provide a handoff message for the next agent. The
handoff must summarize the shipped behavior, focused verification, known blockers, and any contract
or docs changes that later phases must honor. Manual testing notes should cover the core group
replay flow rather than an exhaustive test matrix.
