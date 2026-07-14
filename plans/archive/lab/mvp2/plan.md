# Lab MVP2 Interaction Plan

## Purpose

Make the shipped lab MVP usable by treating it as a real match with a privileged operator tool
layer, not as a separate UI or simulator. MVP2 fixes the two immediate failures: lab operators must
be able to select real units and issue real command-card orders, and spawning must be a palette plus
world-click workflow instead of a form with arbitrary coordinates. The priority is a cleaner lab
interaction architecture that uses the existing room, `Game`, command, and catalog seams better
while keeping normal matches, replays, spectators, and scenario import/export stable.

## Scout Findings

- The server already has the main authoritative lab seams: `Game::new_lab`, `Game::apply_lab_op`,
  `Game::issue_lab_command_as`, scenario import/export, room-local lab request routing, and
  operator/viewer checks.
- Lab launch currently sends lab clients through spectator-shaped start payloads, with prediction
  disabled and normal gameplay command capability disabled. That is reasonable for authority and
  projection, but the client currently also uses spectator state as a broad UI-disable switch.
- The client app shell already owns `LabClient`, `LabPanel`, and `LabControlPolicy`, then injects
  lab collaborators into `Match`. That boundary is the right place to keep building; `Match`, HUD,
  input, minimap, and renderer should receive small collaborators or intent state, not import lab
  panels directly.
- Lab selection and command plumbing is partly ready. `LabControlPolicy` knows which owner a lab
  operator can control, input command helpers already consult control policy in several places, and
  command issuing can wrap normal commands as `issueCommandAs`.
- The command card is hidden because spectator gates still dominate parts of `Match`, HUD, and
  command-card descriptor construction. The fix should distinguish "projection is spectator-like"
  from "this viewer may use the command surface."
- Spawning is form-based today. `LabPanel` presents kind, owner, `X`, and `Y` fields, and the
  default position comes from camera or map center, which makes spawn placement feel arbitrary.
- Faction catalogs already exist in the client config mirror and lobby UI exposes playable
  factions. MVP2 should use those catalogs for a faction-filtered palette instead of deriving a
  flat spawn list from all stats.
- `ClientIntent` already owns placement, command-target, and preview state for normal match
  interactions. Lab setup tools should extend that intent boundary instead of attaching independent
  viewport click listeners to the panel.

## MVP2 Contract

- A lab operator can select any inspectable non-neutral entity and can control a single player-owned
  selection through the normal command card.
- Lab gameplay orders remain normal commands wrapped as lab issue-as requests. The server remains
  authoritative for stale ids, mixed-owner selections, invalid targets, and command validity.
- Read-only lab viewers, replay viewers, and normal spectators remain passive and do not gain
  command-card or setup-tool controls.
- The operator can choose a spawn owner, choose a faction, pick from a filtered unit palette, and
  click the world to spawn the selected kind at that point.
- Unit spawn is the primary workflow. Any existing building/setup spawn affordance that remains
  should either use the same click-to-world tool path or be explicitly treated as temporary advanced
  setup, not as the main unit spawn experience.
- Lab setup tools use explicit client intent and small injected callbacks. They do not store tool
  state in `GameState`, do not fake local ownership, and do not bypass `LabClient`.
- Vision switching, player resource/research setup, selected-entity mutations, scenario
  import/export, and lab teardown continue to work through existing lab services.

## Non-Goals

- Do not rewrite the lab room model, create a second renderer, or fork a separate lab match app.
- Do not redesign the wire protocol unless a phase proves a small explicit capability field is
  required for clean command-surface behavior.
- Do not add timeline rewind, pause/step simulation, branching, keyframes, durable scenario
  libraries, auth, sharing, or multi-operator collaboration.
- Do not add new unit kinds, balance changes, art changes, AI behavior, or production match rules.
- Do not migrate `/dev/scenario` or the old unit preview surfaces in this plan.
- Do not let client lab controls mutate sim internals directly. Privileged changes still go through
  typed lab requests and public `Game` APIs.

## Architectural Constraints

- Keep lab as a real room mode around the authoritative `Game`.
- Keep the app-shell ownership model: `App` owns lab route state, `LabClient`, `LabPanel`, and lab
  policies; `Match` receives collaborators through options.
- Use `ClientIntent` for active lab tools. HUD, input, and renderer feedback may observe or consume
  generic intent state, but they must not import `LabPanel` or `LabClient`.
- Preserve the normal command pipeline. The command card should build descriptors from real
  selected entities, and lab issue-as should happen at the command issuer/control-policy boundary.
- Separate projection state from command permission. A lab operator may still receive
  spectator-shaped snapshots and disabled prediction while being allowed to use the command surface.
- Keep read-only roles explicit. Avoid inferring lab privileges from room names, URL modes, or
  local UI state.
- Use the faction catalog mirror for palette contents. If current export direction makes that
  awkward, extract a small shared catalog helper rather than duplicating faction lists in
  `LabPanel`.
- Keep server validation authoritative. Client palette filtering is an affordance, not a trust
  boundary.
- Preserve focused tests for every changed boundary and rely on the PR `./tests/run-all.sh` gate
  for full-suite coverage.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the head SHA is reachable
  from `origin/main`.

## Phase Summaries

### [Phase 1 - Lab Operator Command Surface](phase-1.md)

Make the lab operator command surface behave like a controllable match without changing server
authority. This phase decouples command-card and input gates from raw spectator status, uses
`LabControlPolicy` to decide owner control, and keeps read-only viewers, replay viewers, and normal
spectators passive. It proves that selecting a player-owned unit in lab shows the real command card
and that normal orders still travel through lab issue-as.

### [Phase 2 - Lab Tool Intent Boundary](phase-2.md)

Add an explicit client intent boundary for active lab setup tools before rebuilding spawn UI. This
phase lets `LabPanel` arm or cancel a generic lab tool through `Match`, lets input consume world
clicks for that tool, and keeps setup tool state out of `GameState`. It should finish with a
minimal test-only or placeholder tool path proving click coordinates, cancellation, and interaction
priority are clean.

### [Phase 3 - Faction Spawn Palette](phase-3.md)

Replace coordinate-entry unit spawning with the intended palette workflow. The operator chooses
owner and faction, picks a unit kind from catalog-filtered options, then clicks the world to send a
typed spawn request at the clicked world position. Existing spawn capabilities should either move
onto the same tool path or remain clearly secondary until Phase 4, but the main unit spawn flow must
no longer depend on manual `X` and `Y` fields.

### [Phase 4 - Setup Tool Cleanup](phase-4.md)

Move the remaining selected-entity setup actions onto the same lab tool/control pattern where it
improves accuracy or consistency. This phase focuses on delete, move-to-click, owner reassignment,
and selected-entity result handling while preserving resource, research, vision, and scenario
workflows. It should reduce special-case form behavior and leave the panel as a controller for
explicit tools rather than a collection of unrelated mutations.

### [Phase 5 - Hardening, Smoke, and Docs](phase-5.md)

Harden the end-to-end MVP2 Lab interaction model and document the stable contracts. This phase adds
or refreshes focused client, protocol, and architecture coverage, tightens active-tool UI state,
and performs a manual browser smoke of command-card ordering plus click-to-spawn. It closes the
plan by updating the relevant design/context docs and recording remaining lab gaps for future
plans.

## Phase Index

1. [Phase 1 - Lab Operator Command Surface](phase-1.md)
2. [Phase 2 - Lab Tool Intent Boundary](phase-2.md)
3. [Phase 3 - Faction Spawn Palette](phase-3.md)
4. [Phase 4 - Setup Tool Cleanup](phase-4.md)
5. [Phase 5 - Hardening, Smoke, and Docs](phase-5.md)

## Overall Constraints

- Preserve normal match, lobby, replay, replay branch, dev scenario, spectator, empty-room reset,
  drain, and match-history behavior unless a phase explicitly documents a lab-only change.
- Keep protocol, client mirror, server protocol, and `docs/design/protocol.md` together if a phase
  changes any lab message, `StartPayload` field, or capability shape.
- Keep `docs/design/client-ui.md` and the client-ui context capsule aligned if the lab collaborator,
  command-surface, or `ClientIntent` contracts change.
- Keep `docs/design/server-sim.md` aligned if any phase changes public `Game` lab APIs or room
  ownership semantics.
- Prefer small generic client collaborators over lab-specific imports across HUD, renderer, input,
  minimap, and command-card modules.
- Do not let active lab tools collide with normal placement, command-target mode, drag selection,
  or camera controls. Define the priority and cancellation rules in the phase that introduces the
  tool boundary.
- Keep palette filtering deterministic and testable. Do not scrape labels or infer unit lists from
  DOM text.
- Bound server-side and client-side payloads the same way the MVP already does; MVP2 UI polish does
  not make clients trusted.
- Focus local verification on the touched boundary for each phase. The full `./tests/run-all.sh`
  check remains the GitHub PR authority.
- A filtered test command only counts as verification when it actually runs matching tests.
- After each phase, the implementing agent must provide a handoff message with exact verification,
  behavior affected, remaining risks, next-phase guidance, and the core manual test focus.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait
gate and confirm the phase head is reachable from `origin/main`. For unattended executor passes,
use:

```bash
scripts/phase-runner.sh --plan lab/mvp2 --from 1 --to 5 --pr --wait
```

Manual review is recommended before starting the chain because Phase 1 intentionally changes the
client meaning of "spectator-shaped projection" versus "command-surface permission."
