# Lobby Registry Cleanup Plan

## Purpose

Make lobby names mean one thing: a name is in use only while the lobby registry has a live room that
should still exist. Empty public lobby shells should not be hidden-but-reserved, and abandoned
create-lobby reservations should be removed from the registry instead of reclaimed through a special
duplicate-create path. The end state should make normal lobbies, replay rooms, branch rooms, lab
rooms, and dev rooms explicit about whether they are disposable when empty.

## Current Situation

- `POST /api/lobbies` reserves a normal room name by creating a `RoomTask` before the creating
  browser joins over WebSocket.
- A room with no host has no lobby-browser summary, so an abandoned reservation can be invisible to
  players while still occupying its name in `Lobby.rooms`.
- Empty rooms reset to a clean lobby today, but the room task and registry entry stay alive until
  process shutdown.
- PR #264 briefly fixed the symptom by allowing expired empty reservations to be reclaimed. That
  patch is intentionally reverted before this plan because the simpler long-term system is registry
  deletion, not hidden shell reuse.

## Desired Invariant

The lobby registry owns room identity. A room task may keep running only while it has occupants,
visible public state, an active authoritative session, replay viewers, or another explicit reason
to remain. When a room becomes empty and disposable, the registry removes the matching handle and
the room task exits because its event channel is dropped.

## Overall Constraints

- Keep the `RoomTask` as the sole owner of `Game`; registry cleanup must not inspect or mutate sim
  state directly.
- Guard removal with a room identity token or matching channel check so a stale cleanup signal cannot
  delete a newer room created under the same name.
- Preserve deploy drain semantics: existing occupied rooms remain joinable during drain, and new
  public rooms are still rejected while drain is active.
- Keep internal room modes private. Replay, replay branch, lab, and dev rooms must not decay into
  public normal lobbies just because their last viewer leaves.
- Avoid an immortal hidden reservation model. If create-lobby needs a short join deadline, expiration
  should remove the room from the registry rather than adding a duplicate-create reclaim path.
- Update design docs only if a phase changes a documented cross-file contract or room lifecycle
  policy; otherwise keep the plan and tests as the active handoff.
- Use focused verification for each phase and rely on the PR `./tests/run-all.sh` gate for broad
  coverage.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head is
  reachable from `origin/main`.
- After each phase, the implementing agent must provide a handoff message with exact verification,
  behavior affected, remaining risks, next-phase guidance, and the core features that should be
  manually tested.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phase Summaries

### [Phase 1 - Registry Disposal Primitive](phase-1.md)

Introduce a registry-owned room disposal path without changing public lobby behavior yet. Room tasks
should be able to report that they are empty and disposable, and the lobby registry should remove
only the exact matching room handle. This phase proves stale disposal signals cannot remove a newer
room under the same name.

### [Phase 2 - Public Lobby Deletion Semantics](phase-2.md)

Use the disposal primitive for normal public lobbies and create-lobby reservations. A fresh
create-lobby request may reserve the name briefly for the browser's follow-up WebSocket join, but
if the join never arrives the room is removed from the registry. When the last human leaves a normal
public lobby or match room, the name should become available because the old room is gone.

### [Phase 3 - Internal Room Cleanup And Browser Coverage](phase-3.md)

Extend or explicitly decline empty-room deletion for replay, replay branch, lab, and dev rooms
based on each mode's lifecycle needs. The lobby browser and live integration coverage should prove
that abandoned public reservations disappear, ordinary empty lobbies stop occupying names, in-game
and spectator-visible rows remain stable while occupied, and internal rooms do not leak into the
public browser. This phase also updates any lifecycle docs that became stale.

## Phase Index

1. [Phase 1 - Registry Disposal Primitive](phase-1.md)
2. [Phase 2 - Public Lobby Deletion Semantics](phase-2.md)
3. [Phase 3 - Internal Room Cleanup And Browser Coverage](phase-3.md)

## Suggested Execution

Run one phase at a time from fresh `origin/main`, and wait for each PR to merge before starting the
next phase.

```bash
scripts/phase-runner.sh --plan lobbyregistry phase-1 phase-2 phase-3 --pr --wait
```
