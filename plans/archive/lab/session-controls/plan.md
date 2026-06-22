# Lab Session Controls Plan

## Purpose

Retire the old quickstart/debug mode path and move the next lab control work onto the shared room
primitives that now exist. Presets are deliberately out of scope because they will be hand-authored
later as explicit lab scenarios, and lab flags are not needed for this slice. The plan keeps the
collaborator model simple: every direct lab joiner is an omnipotent operator, while lab vision
becomes per-operator and lab time controls become shared room state.

## Scout Findings

- The visible lobby Debug mode has already been replaced by an Open Lab path, but
  `setQuickstart`, boosted resources, debug starting loadouts, and owner-only movement diagnostics
  still exist as internal/test compatibility.
- Current lab joiners already receive `LabStartRole::Operator`, and the original `operatorId` is
  compatibility metadata rather than the sole mutation authority.
- Lab vision is still stored as one room-global `LabVisionMode`, so one operator's vision switch
  changes projection for every collaborator.
- Replay and dev scenario rooms already use neutral room-time messages and `RoomTimeCapabilities`.
  Labs currently advertise a fixed live-match clock and omit pause, step, seek, keyframes, and
  timeline affordances.
- Replay rooms already have keyframes and seek rebuilds, but lab rewind cannot simply be replay
  playback because accepted lab operations and issue-as commands must be replayable alongside normal
  ticks.

## Product Contract

- Quickstart and Debug mode mean the same legacy thing for this plan: the hidden
  `setQuickstart` path that skips countdown, grants 99,999 resources, creates debug structures and
  units, adds an inert enemy fixture, and enables owner-only movement diagnostics.
- After this plan, that legacy path is gone from active product, protocol, client, tests, and
  source-of-truth docs. Normal solo starts may still skip countdown, but they use normal resources
  and normal starting loadouts.
- Every lab URL joiner remains an omnipotent operator. Do not add fine-grained permissions, locks,
  invite flows, auth, private-room policy, cursors, or read-only product UI in this plan.
- Lab vision is per-operator. One collaborator can inspect full world while another uses a team or
  team-union projection, and both choices remain server-authoritative.
- Lab time controls are shared room controls. Any operator can pause, resume, change speed, step
  while paused, and eventually seek the lab timeline for the whole room.
- Lab timeline history is room-local and in-memory. It is not a public scenario library, durable
  replay artifact, match-history row, or server-side scenario storage feature.
- Lab rewind uses recorded lab operations, issue-as commands, keyframes, and authoritative ticks.
  If an operator seeks into the past and then applies a new operation or command, future lab
  timeline entries after the current tick are truncated instead of creating branch UI.

## Non-Goals

- Do not create or generate lab presets. They should be hand-authored later as explicit scenarios.
- Do not add lab flags such as god mode, inert units, disabled damage, frozen cooldowns, unlimited
  supply, or unlimited resources.
- Do not add fine-grained permissions or make any direct lab joiner read-only.
- Do not add durable/public scenario libraries, moderation, auth, sharing flows, or DB-backed lab
  storage.
- Do not migrate `/dev/scenario`, `/dev/unit-lab`, AI self-play harnesses, or visual rig iteration.
- Do not make lab prediction client-authoritative. Lab operators can keep spectator-shaped
  projection and disabled prediction while using privileged lab commands.
- Do not overload `LabClientOp` with clock controls. Pause, speed, step, and seek belong to the
  neutral room-time message family and capability metadata.

## Architectural Constraints

- Keep the lab as one room task owning one authoritative `Game`; do not add locks around `Game` or
  cross-room mutable lab state.
- Keep room time policy in `SessionPolicy`, `TickControl`, and room-owned runtime state. Lab time
  should become another room-time source, not a replay-mode special case.
- Keep accepted lab mutations through public `Game` lab APIs. Room code may authorize, sequence,
  log, and replay lab ops, but it must not reach into sim internals to mutate entities directly.
- Keep protocol mirrors together when removing quickstart or adding lab timeline metadata:
  `server/crates/protocol/src/lib.rs`, `server/crates/contract/src/lib.rs`,
  `server/src/protocol.rs`, `client/src/protocol.js`, and `docs/design/protocol.md`.
- Keep client room controls metadata-driven. The client must read `StartPayload.capabilities` and
  `roomTimeState`, not infer timeline affordances from replay, lab, dev, URL, or legacy debug names.
- Keep lab UI app-owned. `App` may own lab clients/panels and pass small collaborators into `Match`;
  `Match`, HUD, input, minimap, renderer, and room-time controls must not import lab panels.
- Keep timeline memory bounded and room-local. The implementation must define keyframe interval,
  maximum retained keyframes or history, and behavior when the cap is reached.
- Keep snapshot and event visibility authoritative. Per-operator lab vision must be applied in
  server fanout before snapshots leave the room task.
- Preserve normal matches, spectators, replays, replay branches, dev scenarios, match history,
  deploy drain behavior, and empty-room reset unless a phase explicitly scopes a lab-only change.
- Focus local verification on touched boundaries. The PR `./tests/run-all.sh` check remains the
  full-suite authority, and a filtered command only counts when it actually runs matching tests.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the head SHA is reachable
  from `origin/main`.
- After each phase, the implementing agent must provide a handoff message with exact verification,
  behavior affected, remaining risks, next-phase guidance, and the core manual test focus.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phase Summaries

### [Phase 1 - Quickstart Caller Migration](phase-1.md)

Move active tests, internal client seams, and documented workflows off the legacy quickstart/debug
path while leaving the wire command in place temporarily. The phase should make every remaining
quickstart reference either dead compatibility slated for deletion in Phase 2 or archived history
outside active docs. This keeps the removal phase small and prevents tri-state/regression coverage
from depending on a product mode the user wants gone.

### [Phase 2 - Quickstart Protocol And Loadout Removal](phase-2.md)

Delete the compatibility command, lobby field, client sender, server room state, debug starting
loadout, boosted-resource constants, and movement-diagnostic coupling. Normal solo starts still use
the ordinary game setup and may still skip countdown because they are solo, not because Debug mode is
enabled. The phase updates source-of-truth docs and proves no active test or protocol mirror still
depends on quickstart.

### [Phase 3 - Per-Operator Lab Vision](phase-3.md)

Replace room-global lab vision with recipient-specific server-owned lab vision while keeping every
direct lab joiner an operator. `setVision` changes only the requesting operator's projection, and
lab start/state messages report the recipient's own vision rather than a shared room value. Export
and import keep a simple rule: scenario vision applies to the requesting operator and future join
defaults, not to already connected collaborators.

### [Phase 4 - Lab Pause, Speed, And Step](phase-4.md)

Add a lab room-time source that reuses neutral `setRoomTimeSpeed`, `stepRoomTime`, capability
metadata, and `roomTimeState`. Pausing, resuming, speed changes, and one-tick stepping are shared by
the whole lab room and may be controlled by any operator. The implementation should route lab ticks
through the same authoritative live-game tick/fanout path, not through replay playback or a second
lab simulator.

### [Phase 5 - Lab Timeline Recording](phase-5.md)

Introduce an in-memory room-local lab timeline model that records baseline state, keyframes,
accepted lab operations, and issue-as commands in authoritative tick order. Scenario import is a
timeline discontinuity and should reset the baseline/history instead of trying to stitch unrelated
worlds together. This phase exposes trustworthy room-time state and keyframe metadata for labs, but
does not enable seeking yet.

### [Phase 6 - Lab Timeline Seek And Rebuild](phase-6.md)

Enable relative and absolute lab seek by restoring the nearest lab keyframe and replaying recorded
lab timeline entries to the target tick. Seeking is shared room control and re-stamps connected lab
operators with fresh start/snapshot/time state so clients cannot keep stale world assumptions. If
the room accepts a new lab operation or issue-as command after a past seek, future entries and
keyframes after the current tick are truncated instead of creating branch or undo semantics.

### [Phase 7 - Timeline UI, Smoke, And Documentation](phase-7.md)

Make the browser expose lab pause, step, speed, and timeline seek through capability-driven room-time
controls without replay-specific assumptions leaking into lab UI. Harden client/server/protocol
coverage, update design/context docs, and run a two-browser smoke covering per-user vision and shared
timeline control. This phase closes the plan by recording remaining future work: hand-authored lab
presets, optional lab flags, durable scenario libraries, branch-from-lab, and `/dev/scenario`
migration.

## Phase Index

1. [Phase 1 - Quickstart Caller Migration](phase-1.md)
2. [Phase 2 - Quickstart Protocol And Loadout Removal](phase-2.md)
3. [Phase 3 - Per-Operator Lab Vision](phase-3.md)
4. [Phase 4 - Lab Pause, Speed, And Step](phase-4.md)
5. [Phase 5 - Lab Timeline Recording](phase-5.md)
6. [Phase 6 - Lab Timeline Seek And Rebuild](phase-6.md)
7. [Phase 7 - Timeline UI, Smoke, And Documentation](phase-7.md)

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait
gate and confirm the phase head is reachable from `origin/main`.

For unattended executor passes, prefer explicit phase ids for this nested plan:

```bash
scripts/phase-runner.sh --plan lab/session-controls phase-1 phase-2 phase-3 phase-4 phase-5 phase-6 phase-7 --pr --wait
```

Manual review is recommended before Phase 5 because the timeline recording model sets the durable
shape for later rewind and branch-from-lab work.
