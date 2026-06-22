# Lab Debug Replacement And Collaboration Plan

## Purpose

Make the lab the normal place for multiplayer experimentation and remove the visible lobby Debug
mode workflow from product UI. This plan does not recreate the old Debug-mode preset with boosted
resources, prebuilt structures, staged armies, or inert enemy fixtures; those should come back later
as explicit lab scenario/preset work. The goal here is narrower: multiple users can join the same
lab, all allowed collaborators can spawn, edit, issue-as, and control armies through the existing
lab tools, and the lobby stops presenting Debug mode as the player-facing experimentation path.

## Scout Findings

- The lab already runs as a real `RoomMode::Lab` around an authoritative `Game`, with typed
  `LabClientOp` requests, `Game::apply_lab_op`, `Game::issue_lab_command_as`, JSON import/export,
  and the normal `Match` renderer/HUD/input shell.
- `/lab?room=...&map=...` already maps to a shared internal `__lab__:<room>:map=<map>` room, so
  multiple browser sessions can join the same lab room today.
- The server deliberately grants `LabStartRole::Operator` only to the first lab joiner. Later lab
  joiners receive `ReadOnly`, and `on_lab_request` rejects their lab operations.
- The client gates setup tools and command-card issue-as on the lab role. Once the server sends
  `role: "operator"` to a collaborator, most existing `LabPanel`, `LabClient`, and
  `LabControlPolicy` behavior should become available without a broad client rewrite.
- Lab vision is currently room-global. A collaborator changing vision affects the shared projection
  state for the lab room. Per-user lab vision is useful later, but it is not required for the first
  collaborative lab.
- Lobby Debug mode is still the `setQuickstart` path. It skips countdown, boosts starting
  resources, applies a special debug loadout, sets owner-only movement diagnostics for active
  players, and is still used by regression and tri-state tooling.

## Product Contract

- A user can share a lab URL with another user, and both users can operate in the same live lab
  room.
- Every lab collaborator in this plan is omnipotent inside that room: they can spawn units, move or
  delete selected entities, reassign owners, set player state, switch lab vision, import/export
  scenarios, and issue real gameplay commands as a selected entity's owner.
- Lab operations remain server-authoritative and room-local. A bad/stale entity id, invalid owner,
  invalid placement, invalid research id, or mixed-owner gameplay command still produces an explicit
  lab result instead of trusting the browser.
- Concurrent operations use simple room task ordering. If two collaborators act at the same time,
  the room applies accepted operations in receive order and broadcasts the resulting lab state and
  snapshots.
- The normal lobby no longer advertises Debug mode as the main experimentation affordance. The
  visible path should point users toward `/lab`; the old `setQuickstart` implementation can remain
  temporarily as an internal/test compatibility path.

## Non-Goals

- Do not recreate the old Debug-mode starting preset, boosted-resource setup, inert enemy fixture,
  or movement-debug scenario as part of this plan.
- Do not delete `setQuickstart` from the wire protocol until the test harnesses and internal
  compatibility users have a replacement.
- Do not add auth, invitations, private-room access control, moderation, persistent public scenario
  libraries, or server-side scenario storage.
- Do not add per-operator permissions, locks, cursors, presence lists, edit conflict UI, undo,
  timeline rewind, pause/step/seek, keyframes, or branch-from-lab behavior.
- Do not migrate `/dev/scenario`, `/dev/unit-lab`, AI self-play harnesses, or visual rig iteration.
- Do not make lab prediction client-authoritative. Lab operators may keep spectator-shaped
  projection and disabled prediction while using the privileged lab envelope.

## Architectural Constraints

- Keep the lab as one room task owning one authoritative `Game`. No locks around `Game`, no
  cross-room mutable lab state, and no client-side mutation bypasses.
- Keep lab mutation APIs narrow and typed. Room/lobby code calls public `Game` lab APIs; it must
  not reach into sim internals to implement collaboration.
- Keep protocol mirrors together if a phase changes `LabStartMetadata`, `LabState`, lab roles, room
  capabilities, `ClientMessage`, or `ServerMessage`.
- Prefer the smallest role model that supports the product: for this plan, "collaborator operator"
  and "read-only viewer" are enough. Do not design a full ACL matrix.
- Shared vision is acceptable for this plan. If a phase chooses to add per-viewer lab vision, it
  must explicitly update room projection, `LabState`, tests, and docs in that phase.
- Keep normal matches, spectators, replays, replay branches, dev scenarios, match history, drain
  behavior, and empty-room reset unchanged unless a phase explicitly scopes a lab-only change.
- Keep the old quickstart/debug protocol path hidden or compatibility-only, not removed, until
  tests and internal harnesses stop depending on it.
- Focus local verification on touched boundaries. The PR `./tests/run-all.sh` check remains the
  full-suite authority.
- A filtered test command only counts as verification when it actually runs matching tests.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the head SHA is reachable
  from `origin/main`.
- After each phase, the implementing agent must provide a handoff message with exact verification,
  behavior affected, remaining risks, next-phase guidance, and the core manual test focus.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phase Summaries

### [Phase 1 - Collaborative Lab Authority](phase-1.md)

Change the server lab authority model so additional lab joiners can be operators instead of
read-only viewers. The room task should authorize lab requests by the sender's lab role, record the
actual requester in the operation log, and keep all mutation validation inside the existing typed
lab APIs. The phase proves that two connections in one lab can both apply lab operations while
normal rooms still reject lab messages.

### [Phase 2 - Collaborative Lab Client Experience](phase-2.md)

Make the browser experience match the new server contract. A second user joining the same `/lab`
URL should see operator tools, command cards, lab results, and state updates without needing to
know who joined first. The phase keeps client tool state local to each browser tab while treating
world state, lab dirty state, operation count, and vision as shared room state.

### [Phase 3 - Lab Entry Replaces Visible Debug Mode](phase-3.md)

Move the player-facing experimentation entry away from the normal lobby Debug mode toggle and toward
the lab route. This phase should remove or hide the visible `Debug mode` lobby control from normal
product UI, add an intentional lab entry/share path where appropriate, and leave `setQuickstart`
available only as a temporary internal/test compatibility command. It should update docs so the lab
is the documented way to stage experiments, without recreating the old debug preset.

### [Phase 4 - Hardening, Smoke, And Documentation](phase-4.md)

Harden the complete collaborative lab and debug-entry migration. Add focused server/client/protocol
coverage, update source-of-truth docs, and run a manual two-browser smoke where both collaborators
spawn units, control armies, and observe shared state. This phase closes the plan by documenting the
remaining explicit follow-ups: lab presets, lab flags, per-user vision, timeline controls, and final
quickstart deletion.

## Phase Index

1. [Phase 1 - Collaborative Lab Authority](phase-1.md)
2. [Phase 2 - Collaborative Lab Client Experience](phase-2.md)
3. [Phase 3 - Lab Entry Replaces Visible Debug Mode](phase-3.md)
4. [Phase 4 - Hardening, Smoke, And Documentation](phase-4.md)

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait
gate and confirm the phase head is reachable from `origin/main`.

For unattended executor passes, prefer explicit phase ids for this nested plan:

```bash
scripts/phase-runner.sh --plan lab/debug-collab phase-1 phase-2 phase-3 phase-4 --pr --wait
```

Manual review is recommended before Phase 3 because it changes the player-facing lobby/debug
workflow while intentionally leaving the legacy quickstart command alive for tests and internal
compatibility.
