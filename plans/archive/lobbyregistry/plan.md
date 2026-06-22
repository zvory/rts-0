# Lobby Registry Cleanup Plan

## Purpose

Make lobby names mean one thing: a name is in use only while the lobby registry has a live room that
should still exist. Empty public lobby shells should not be hidden-but-reserved, and abandoned
create-lobby reservations should be removed from the registry instead of reclaimed through a special
duplicate-create path. The end state should make public lobby deletion strict: after the short
pending-create window, an empty public lobby is gone.

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

The lobby registry owns room identity. Public normal lobby names are occupied only while the lobby
has at least one connected human, is running a live match/post-match viewer session for connected
humans, or is inside the short pending-create lease before the creator joins. When a public normal
lobby becomes empty outside that pending-create lease, the registry removes the matching handle
immediately and the room task exits because its event channel is dropped. There is no host reconnect
grace period and no empty-public-lobby retention path; a returning host can create the lobby again.

## Overall Constraints

- Keep the `RoomTask` as the sole owner of `Game`; registry cleanup must not inspect or mutate sim
  state directly.
- Guard removal with a room identity token or matching channel check so a stale cleanup signal cannot
  delete a newer room created under the same name.
- Preserve deploy drain semantics: existing occupied rooms remain joinable during drain, and new
  public rooms are still rejected while drain is active.
- Keep internal room modes private. Replay, replay branch, lab, and dev rooms must not decay into
  public normal lobbies just because their last viewer leaves.
- Avoid every immortal hidden reservation model. The only allowed empty public lobby state is a short
  pending-create lease while the accepted creator joins over WebSocket; lease expiration removes the
  room from the registry.
- Do not add host reconnect grace for public lobby names. If every human disconnects, the room name
  becomes available again.
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
should be able to report that they are empty and removal-eligible, and the lobby registry should remove
only the exact matching room handle. This phase proves stale disposal signals cannot remove a newer
room under the same name.

### [Phase 2 - Public Lobby Deletion Semantics](phase-2.md)

Use the disposal primitive for normal public lobbies and create-lobby reservations. A fresh
create-lobby request reserves the name only as a pending-create lease for the browser's follow-up
WebSocket join; if the join never arrives the room is removed from the registry. When the last human
leaves a normal public lobby or match room, the old room is removed immediately and the name becomes
available.

### [Phase 3 - Internal Room Cleanup And Browser Coverage](phase-3.md)

Audit replay, replay branch, lab, and dev rooms after public lobbies have strict deletion semantics.
Those internal modes are not public lobbies: delete them when empty unless a mode has an explicit
non-public state source that must remain alive. The lobby browser and live integration coverage
should prove that abandoned public reservations disappear, ordinary empty lobbies stop occupying
names, in-game and spectator-visible rows remain stable while occupied, and internal rooms do not
leak into the public browser.

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
