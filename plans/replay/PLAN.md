# Replay System Plan

## Goals

Build a first-class replay system that can replay completed matches immediately, later load
persisted replays from match history, and evolve into a dedicated analysis viewer with overlays,
scrubbing, and shared multiplayer controls.

Core principles:

- Replays are command-log based, not recorded video or full snapshot streams.
- Replay artifacts are strongly versioned and self-describing.
- The viewer shows selectable real player vision, defaulting to the union of all players' vision,
  not a no-fog full-world view.
- Replay playback is server-authoritative so multiple viewers share speed, seek, and room state.
- Camera position and selected fog perspective are local per viewer unless explicitly promoted to a
  shared control later.
- Live-match, dev-replay, and persisted-replay paths should converge on shared abstractions instead
  of diverging into special cases.

## Phase Index

1. [Phase 1 - Replay Contract](phase-1-replay-contract.md)
2. [Phase 2 - Server Replay Runtime](phase-2-server-runtime.md)
3. [Phase 3 - Automatic Post-Match Viewer](phase-3-post-match-viewer.md)
4. [Phase 4 - Client Replay Viewer](phase-4-client-viewer.md)
5. [Phase 5 - Persistence and Match History Entry Points](phase-5-persistence.md)
6. [Phase 6 - Scrubbing, Overlays, and Hardening](phase-6-analysis-hardening.md)
7. [Phase 7 - Resume Play From Replay](phase-7-resume-play.md)

## Non-Goals For The First Pass

- Do not store full per-tick snapshots unless command-log replay proves insufficient.
- Do not expose no-fog world state in normal replay viewing.
- Do not make replay controls purely client-local; multiplayer viewers need shared playback state.
- Do not couple production replay loading to dev self-play artifact directories.
- Do not attempt branching/resume-play until ordinary replay playback, seeking, and compatibility
  checks are stable.
