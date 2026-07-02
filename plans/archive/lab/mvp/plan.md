# Lab MVP Implementation Plan

## Purpose

Build the first usable lab as a real room mode around the authoritative `Game`, not as a second
simulator or visual scratchpad. The MVP should let an operator create a lab on a real map, stage
real players and units, inspect and control any side, switch server-owned vision, and import/export
legible JSON scenarios. It should consume the room primitives from `plans/lab/room` while keeping
normal matches, replays, replay branches, and dev scenarios stable.

## MVP Contract

- A user can open a lab route, create or join a lab room, choose a real map, and start a real
  `Game` with a small player/team template.
- The lab uses the normal client `Match`, renderer, minimap, HUD, command card, teardown, and
  snapshot ingestion wherever those surfaces are authentic.
- The lab has one omnipotent operator for the MVP. Additional connections may be read-only viewers
  or rejected until multi-operator semantics are designed.
- The operator can switch vision between full-world, one team, and selected-team union. Vision is
  server-authoritative and per viewer.
- The operator can spawn, delete, move, and reassign existing real unit and building kinds through
  typed lab operations.
- The operator can set useful player state for staged scenarios: steel, oil, and completed
  research.
- The operator can inspect any non-neutral entity and issue real gameplay commands as the owning
  player for single-owner selections.
- The operator can export the current setup to versioned JSON and import that JSON to rebuild a
  lab scenario.
- Accepted privileged operations are logged in room-local tick order from the first implementation
  that accepts mutations, even before timeline replay exists.

## Non-Goals

- Do not build visual rig iteration, scratch art hot reload, particle iteration, animation
  iteration, or balance-number hot reload.
- Do not replace the old `/dev/unit-lab` canvas preview in this MVP.
- Do not implement rewind, seek, branch-from-lab, keyframe editing, or tick-perfect restart
  recovery.
- Do not add god mode, inert units, disabled damage, frozen cooldowns, unlimited supply, or other
  simulation flags unless a later plan scopes them as explicit typed lab flags.
- Do not add public persistent scenario libraries, moderation flows, auth, sharing, or database
  scenario storage. Browser import/export and bundled read-only examples are enough for the MVP.
- Do not migrate `/dev/scenario` in this plan. It can stay mode-local until the lab proves which
  pieces should be shared.
- Do not let lobby or HTTP code mutate sim internals directly. Privileged state changes still go
  through public `Game` lab APIs.

## Architectural Boundaries

- `server/crates/sim/src/game` owns lab state validity and mutation. It validates lab operations,
  applies accepted changes, recomputes derived state such as supply, fog, spatial indexes, and
  building memory, and exposes structured errors for invalid input.
- `server/src/lobby` owns lab room lifecycle. It decides the operator, viewer roles, lab room
  config, per-viewer vision, operation logs, request/result routing, empty-room reset, and snapshot
  fanout policy.
- `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `client/src/protocol.js`, and
  `docs/design/protocol.md` stay mirrored for every lab message or `StartPayload` field.
- The client app shell owns lab entry state, map/scenario selection, `LabClient`, and `LabPanel`.
  `Match` receives lab collaborators through options and must not import lab panels directly.
- Normal gameplay commands remain normal commands. Lab issue-as behavior selects the owning player
  explicitly and still relies on normal server command validation.
- Lab scenario JSON is authoritative setup data, not a saved `Snapshot`. Snapshot wire fields are
  projections with fog, events, compact transport details, and client conveniences.
- Lab rooms are production-safe by construction. A lab operation may affect only that lab room's
  `Game`, not other rooms, arbitrary files, global state, or normal matches.

## Phase Summaries

### [Phase 1 - Lab Room Skeleton](phase-1.md)

Add the room, policy, launch, and route skeleton for a lab that starts a real `Game` and sends a
normal `start` payload with lab metadata. This phase should prove that a lab is room-hosted, hidden
from normal lobby behavior, uses the shared launch/projection primitives, and can show full-world
snapshots in the normal `Match` view. It should not add privileged mutations yet; the only
player-facing value is entering a real lab room and seeing the world through the normal renderer.

### [Phase 2 - Authoritative Lab Game API](phase-2.md)

Add narrow typed `Game` lab APIs for scenario-safe mutations: spawn, delete, move, set owner, set
resources, and set completed research. This phase keeps all mutation validation and derived-state
repair inside the sim crate, with focused tests for accepted and rejected operations. It should not
wire browser controls yet; room and client code must still be unable to reach into sim internals.

### [Phase 3 - Lab Protocol and Room Operations](phase-3.md)

Add mirrored lab wire messages, request ids, explicit results, lab state broadcasts, operation
logging, operator authorization, issue-as command routing, and per-viewer vision operations. This
phase connects the room task to the `Game` lab API while keeping every operation bounded and
room-local. It should finish with server tests proving that normal rooms are unaffected, only the
operator can mutate, viewers remain read-only, and vision modes use server-owned projection.

### [Phase 4 - Client Lab Shell](phase-4.md)

Add the client lab route, `LabClient`, and lab app shell that mounts a normal `Match` with injected
lab collaborators. The first panel should expose map/lab entry state, lab status, result errors,
vision controls, and enough static structure for later setup tools without putting lab code inside
the renderer or HUD internals. This phase should make the lab visibly usable as a mode, but setup
mutations can remain basic or developer-facing until Phase 5.

### [Phase 5 - Setup Tools and Control Policy](phase-5.md)

Build the operator tools that make the MVP useful: spawn, delete, move, owner reassignment,
resource/research edits, omnipotent inspection, and single-owner issue-as commands. The client
should make ownership policy explicit through injected control collaborators instead of faking
`playerId` or special-casing every surface. The server remains authoritative for every operation,
so stale ids, mixed-owner gameplay orders, invalid kinds, invalid coordinates, and impossible
research ids produce explicit results rather than client-only assumptions.

### [Phase 6 - Scenario Import and Export](phase-6.md)

Add versioned legacy setup import/export for browser JSON files and optional bundled read-only
examples. Export should capture map identity, seed, players, teams, useful player state, entities,
and lab metadata without copying transient snapshot-only fields. Import should validate schema,
remap entity ids, rebuild a coherent `Game`, and return clear errors without reading arbitrary
server paths.

### [Phase 7 - MVP Hardening and Documentation](phase-7.md)

Harden the end-to-end lab MVP, update design/context docs, and add guardrails where the new
boundaries have become stable. This phase should broaden focused coverage across protocol parity,
sim lab APIs, room authorization, client architecture, scenario round trips, and a live browser
smoke path. It should finish by documenting remaining non-MVP gaps, especially timeline controls,
lab flags, scenario persistence, and `/dev/scenario` migration.

## Phase Index

1. [Phase 1 - Lab Room Skeleton](phase-1.md)
2. [Phase 2 - Authoritative Lab Game API](phase-2.md)
3. [Phase 3 - Lab Protocol and Room Operations](phase-3.md)
4. [Phase 4 - Client Lab Shell](phase-4.md)
5. [Phase 5 - Setup Tools and Control Policy](phase-5.md)
6. [Phase 6 - Scenario Import and Export](phase-6.md)
7. [Phase 7 - MVP Hardening and Documentation](phase-7.md)

## Overall Constraints

- Preserve normal match, replay, replay branch, dev scenario, lobby, spectator, empty-room reset,
  drain, and match-history behavior unless a phase explicitly documents a behavior change for lab
  rooms only.
- Use the existing room primitives from `server/src/lobby/session_policy.rs`,
  `participants.rs`, `tick_control.rs`, `projection.rs`, and `launch.rs` where they fit. Do not
  replace them with a generic plugin framework.
- Keep the room task as the only event and tick owner for a lab room. Do not add locks around
  `Game` or cross-room mutable state.
- Keep clients untrusted. Bound lab payload sizes, ids, names, coordinates, JSON files, request
  ids, and selected entity lists on the server.
- Keep protocol, contract, JavaScript mirror, and protocol docs in the same phase whenever a lab
  message or start payload shape changes.
- Keep lab panels and services injected from the app shell. `Match`, HUD, input, minimap, and
  renderer can depend on small generic collaborators, but they should not import lab panels or
  scenario storage.
- Keep lab operation logging append-only and room-local for the MVP. Do not promise rewind or
  durable replay until a later phase designs keyframes and persistence.
- Prefer focused tests that prove the boundary being changed. The full `./tests/run-all.sh` gate
  remains the PR authority.
- A filtered test command only counts as verification when it actually runs matching tests.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the head SHA is reachable
  from `origin/main`.
- After each phase, the implementing agent must provide a handoff message with exact verification,
  behavior affected, remaining risks, next-phase guidance, and core manual test focus.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait
gate and confirm the phase head is reachable from `origin/main`. For unattended executor passes,
use:

```bash
scripts/phase-runner.sh --plan lab/mvp --from 1 --to 7 --pr --wait
```

Manual review is still appropriate before starting the chain because this plan introduces new
product behavior and protocol surface.
