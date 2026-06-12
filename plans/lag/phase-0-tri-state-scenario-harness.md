# Phase 0 - Tri-State Scenario Harness

Status: Designed, not implemented. Later prediction phases landed before this harness existed; use
Phase 0.5 and the later `.5` backfill phases to build the harness around the current code.

## Objective

Create the testing and inspection harness before prediction gets complex. The harness should let
humans and agents author small scenarios, advance scripted clicks and server ticks, inspect browser
client state at each step, and later compare against a local native/WASM prediction lane.

Phase 0 can ship before the local predictor exists. It should deliver the remote authoritative lane
and browser client lane now, define the local-lane adapter shape, and record explicit
`localLane: "unavailable"` metadata in artifacts until Phase 3 registers a real local lane.

## Core Model

Each scenario run has one timeline and up to three state lanes:

- `remote`: a real server room using the existing WebSocket/protocol path. Dev scenarios may use
  paused simulation plus explicit `stepDevTick`; normal live-room scenarios may advance by bounded
  tick windows until explicit room tick control exists.
- `client`: a real browser client, preferably headless for CI and optionally visible for humans,
  exposing `window.__rts.match.state`, prediction-controller state when present, and
  `window.__rtsDebug` marks/counts.
- `local`: an adapter contract for a native or WASM predictor/reference. In Phase 0 this lane may
  be absent. Phase 3 must add at least one implementation without changing scenario definitions.

The harness should make partial lanes intentional. A two-lane scenario is valid when it asserts
remote/client behavior. A three-lane scenario is required once a feature claims local prediction
correctness.

## Scenario Format

Define a small, reviewable scenario format for regression cases:

- setup: map, players, dev/scenario id or lobby flow, fog/debug mode, selected player
- script: named steps such as click, command, wait for snapshot, deliver/delay snapshot, advance
  remote ticks, advance local ticks, inspect client state
- network profile: none, fixed latency, jitter, delayed command, delayed/coalesced snapshots, slow
  consumer/head-of-line
- assertions: domain-specific checks over positions, order plans, selected units, pending command
  counts, acknowledgements, correction distances, resources, fog-visible entities, and notices
- artifact policy: always on failure, optionally on success for new scenario development

Prefer command-level script operations first, then add click/pointer operations where the bug
depends on input routing, selection, placement, or HUD state.

## Harness Work

- Build a reusable Node runner around the existing live-server WebSocket and headless-browser test
  patterns.
- Add a remote controller that can:
  - join/create a scenario room
  - send gameplay commands with optional future `clientSeq`
  - wait for or capture snapshots
  - step paused dev scenarios with `stepDevTick`
  - record authoritative tick, consumed command metadata when available, events, notices, and
    compact/raw snapshot frames
- Add a browser controller that can:
  - open a scenario URL with debug enabled
  - inspect client `GameState` summaries after each step
  - collect `window.__rtsDebug` marks/counts and prediction-controller diagnostics when present
  - optionally save screenshots for human inspection
- Define the local-lane adapter interface now:
  - initialize from the same scenario start/baseline
  - enqueue the same command stream
  - advance N ticks
  - export a comparable owner-safe state summary
  - export diagnostics such as tick, pending commands, correction distance, and disabled reason
- Implement domain-aware diff summaries for remote/client now and remote/local/client later. Diffs
  should summarize entity position/order/resource/fog/prediction differences instead of dumping
  raw JSON by default.
- Write artifacts under a stable ignored path such as
  `server/target/tri-state-scenarios/<scenario>/<timestamp>/`.

## Initial Scenarios

Start with scenarios that are useful before prediction exists:

- remote/client basic move: click or command a selected worker, step until the authoritative
  snapshot updates, assert client state follows the remote lane.
- queued order visibility: issue queued move/attack-move stages, inspect owner-only order plan in
  remote snapshots and client state.
- delayed snapshot tolerance: buffer authoritative snapshots in the harness and deliver them late
  to the browser controller once the prediction buffer exists.
- dev scenario tick stepping: open one existing game-backed dev scenario, pause it, step one tick at
  a time, and assert artifacts contain remote and client state for each step.

Later phases should add scenarios for command acknowledgement, local movement prediction,
correction after divergence, fog-hidden blocker correction, command rejection, and spectator/replay
non-prediction.

## Human Inspection

Provide a way to open a recorded scenario artifact locally and inspect the timeline. The first
version can be a readable JSON/summary plus optional screenshots. A later UI can show a
side-by-side remote/local/client timeline.

The artifact must include enough context for an agent or human to reproduce the run:

- scenario file and resolved launch URL/room
- command/click timeline
- remote snapshots and net-status summaries
- client state summaries and debug marks
- local-lane frames when available
- assertion failures and first meaningful domain diff

## Verification

- Node test for the scenario parser and timeline executor.
- One CI-safe two-lane scenario that uses a running server and headless browser.
- Artifact creation test for a forced assertion failure.
- Local-lane adapter contract test using the Phase 0 unavailable/stub adapter.
- Documentation in this plan explaining how a new regression scenario should be added.

## Manual Testing Focus

Run one visible or artifact-backed scenario and confirm the recorded timeline is understandable:
setup, scripted actions, remote snapshots, browser state summaries, and assertion results should be
easy to follow. Manual review should focus on whether a future prediction bug could be reproduced
from the artifact, not on multiplayer feel.

## Handoff Expectations

At handoff, include the scenario runner command, the artifact path for at least one successful or
forced-failure run, and any local-lane contract gaps that Phase 3 must fill. Name the next scenario
types that should be added when command acknowledgement and local prediction exist.

Because Phase 3 and Phase 4 are now already implemented, the actual handoff should point directly
to Phase 2.5, Phase 3.5, and Phase 4.5 so the existing prediction code is pulled into the harness
without rewriting the shipped predictor.

## Player-Facing Outcome

No gameplay change. This phase makes future lag/prediction work inspectable before the local
prediction surface exists, so new bugs can become small regression scenarios instead of
hard-to-reproduce multiplayer anecdotes.
