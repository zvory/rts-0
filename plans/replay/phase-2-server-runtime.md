# Phase 2 - Server Replay Runtime

## Objective

Create the reusable server runtime that can play any valid `ReplayArtifactV1` as a multiplayer
viewer session.

## Server Work

- Add a `ReplaySession` abstraction that owns:
  - the immutable artifact
  - rebuilt `Game`
  - command replay driver
  - current tick
  - duration ticks
  - shared speed, defaulting to `2.0`
  - viewer ids and control policy
  - per-viewer fog source selection, defaulting to all players
- Add a first-class room phase, for example `Phase::ReplayViewer(Box<ReplaySession>)`.
- Extract shared snapshot fanout helpers so live spectators and replay viewers both use union-fog
  snapshots instead of no-fog full-world snapshots.
- Extend replay controls to carry sender identity into the room task.
- Decide initial control policy explicitly. Recommended first pass: any connected viewer can set
  speed or seek, and every accepted control emits `replayState`.
- Add per-viewer replay vision controls. These should be live, cheap, and local to the requesting
  viewer, so Player A can watch only Player B's fog while Player C watches the all-player union.
- Clamp speed and seek inputs server-side.
- Ensure production replay rooms reject gameplay commands and give-up.

## Fog Contract

Replay viewer snapshots must use authoritative fog from selected real players. The default selection
is the union of all active match players. Viewers can switch live to one player's fog or a subset of
players' fog. Snapshots should include `playerResources`, but must not use `snapshot_full_for`
except in dev-only watch flows.

## Verification

- Unit test replay session reaches the same final snapshots as the original live match.
- Unit test replay viewer snapshot hides an entity outside all players' union fog.
- Unit test single-player replay fog matches that player's live `snapshot_for` visibility policy.
- Unit test changing a viewer's fog source affects only that viewer's subsequent snapshots.
- Unit test speed and seek controls are clamped and broadcast through `replayState`.
- Regression test that gameplay commands are ignored during replay viewing.

## Player-Facing Outcome

Still no automatic player-facing transition required. This phase makes the server capable of
hosting replay playback safely and consistently.
