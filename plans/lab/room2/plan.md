# Room Capability Completion Plan

## Purpose

Complete the room-policy architecture after `plans/lab/room` and the lab MVP. The first room plan
created useful shared helpers, and the lab MVP now exists, but lower-level room, protocol, and client
code can still infer shared behavior from product-mode names such as replay, dev watch, Debug mode,
or lab. This plan finishes the policy model so a room mode selects an explicit bundle of capabilities:
clock ownership, authority, mutation, visibility, diagnostics, persistence/export, and UI
affordances.

This is not a lab MVP plan. Lab MVP work has already landed, so this effort should not prioritize new
lab product behavior, timeline editing, lab flags, or visual iteration tools. Treat the lab as one
important consumer that proves the room capability model, not as the feature being built here.

## Compatibility Stance

Backwards compatibility is not a requirement for this cleanup. If a replay/dev/debug wire name is the
wrong contract, replace it in the same phase across Rust protocol DTOs, server adapters, JavaScript
protocol mirrors, clients, tests, and `docs/design/protocol.md` instead of preserving fallback fields
or compatibility shims. Do not keep old names merely because existing clients once spoke them; the
repo ships server and client together.

Behavioral stability still matters. Normal matches, spectators, replays, replay branches, dev
scenarios, lab rooms, fog, client-trust boundaries, and room-local lab mutation safety should keep
their current product behavior unless a phase explicitly documents an intended behavior change.

## Overall Constraints

- Keep rooms as the server-owned session boundary for participants, lifecycle, transport, ticking,
  snapshot delivery, authority, and room-local privileged state.
- Keep named product modes. A normal match, replay viewer, replay branch, dev scenario, and lab remain
  understandable product workflows; downstream helpers should consume neutral capability choices when
  behavior is shared.
- Make mutation a first-class policy axis. Distinguish read-only playback, normal gameplay commands,
  branch seat-alias gameplay, dev scenario driver mutation, lab privileged setup operations, lab
  issue-as gameplay commands, and persistence/export effects.
- Do not build a plugin framework, dynamic capability registry, trait-object runtime, or arbitrary
  capability negotiation surface. Prefer small enums, structs, and helper methods that enumerate
  supported combinations.
- Keep `Game` as the simulation seam. Lobby code must not mutate sim internals directly; lab setup
  still goes through public `Game` lab APIs, and normal gameplay commands still go through normal
  command validation.
- Keep fog and diagnostics server-authoritative. Full-world vision, selected-team vision, movement
  paths, and observer analysis are explicit privileged projections or diagnostic data, never accidental
  leaks from mode identity.
- Keep clients untrusted. Replacing protocol names does not remove validation, bounds, ownership
  checks, stale-id handling, request-id validation, or per-recipient snapshot filtering.
- Add guardrails only after a boundary is stable and mechanically checkable. Do not add broad
  allowlists that simply bless today's leakage.
- Use focused tests that prove the changed capability boundary. The PR `./tests/run-all.sh` gate
  remains the full-suite authority.
- A filtered test command only counts as verification when it actually runs matching tests.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with auto-merge
  armed, then waited on until GitHub reports the PR merged and the phase head is reachable from
  `origin/main`.
- After each phase, the implementing agent must provide a handoff message naming exact verification,
  behavior affected, remaining risks, next-phase guidance, and the core features that should be
  manually tested.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Target Concepts

- `SessionMode`: the named product workflow, such as normal match, replay viewer, dev scenario,
  replay branch, or lab.
- `SessionPhase`: where the room is in its lifecycle, such as lobby, live game, replay viewer, or
  branch staging.
- `SessionPolicy`: the complete policy bundle selected by mode plus phase.
- `ClockCapability`: fixed realtime ticking or room-controlled time, with operations such as pause,
  speed, step, and seek only where that state source supports them.
- `AuthorityCapability`: what each connection may control, such as lobby host controls, live owner
  commands, replay playback controls, branch seat alias commands, dev scenario controls, lab operator
  operations, or read-only viewing.
- `MutationCapability`: what can change authoritative state, including none, normal gameplay
  commands, replay playback cursor/keyframes, branch staging claims, dev scenario driver ticks, lab
  privileged setup operations, lab issue-as gameplay commands, scenario import/export, and
  match-history/replay persistence.
- `VisibilityCapability`: normal actor vision or room-controlled vision, with explicit projection
  choices such as player fog, spectator union, replay selected players, lab full world, or lab team
  union.
- `DiagnosticCapability`: whether non-visibility debug data such as movement paths or observer
  analysis may be produced for a recipient.
- `Affordance`: the UI or protocol surface that lets a user operate a capability.

## Phase Summaries

### [Phase 1 - Capability Baseline](phase-1.md)

Audit latest `main` and record the current product modes as capability bundles before changing
runtime behavior. Add or tighten focused characterization tests for the existing clock, authority,
mutation, visibility, diagnostics, persistence/export, and client-affordance decisions. This phase
should create a `plans/lab/room2/capability-matrix.md` baseline and name every known product-mode leak
that later phases will remove.

### [Phase 2 - Policy Bundle And Mutation Axis](phase-2.md)

Extend `SessionPolicy` into a complete capability bundle, with mutation as a first-class axis instead
of an implication of authority or mode. Replace only the lowest-risk direct mode checks with policy
reads where the new bundle can exactly express current behavior. This phase should leave product
behavior unchanged while making the policy vocabulary strong enough for later phases to consume.

### [Phase 3 - Room-Controlled Time Contract](phase-3.md)

Rename and reroute replay/dev-watch time control to a neutral room-controlled-time contract, without
preserving old wire names. Server tick control should ask whether the room owns time and which
operations are allowed, not whether the room is a replay or dev scenario. Client controls should send
and receive the new neutral protocol messages while preserving current replay and dev scenario product
behavior.

### [Phase 4 - Projection And Diagnostics Contract](phase-4.md)

Move movement path diagnostics and other non-visibility debug data behind projection/diagnostic
policy. `Game` may still own the facts required to build diagnostic fields, but room projection policy
should decide whether they are included for a recipient. Start payloads and client settings should
advertise diagnostic affordances through explicit capability metadata, not `debugMode` or
`devWatch.kind`.

### [Phase 5 - Client Capability Affordances](phase-5.md)

Make the browser consume explicit capability metadata for room time controls, diagnostic toggles,
vision controls, observer analysis, lab controls, and read-only/gameplay command surfaces. Remove
mode-name fallbacks that use replay/dev/lab identity when a neutral capability answers the question.
This phase should keep the normal match, replay, dev scenario, and lab screens recognizable while
making their control surfaces policy-driven.

### [Phase 6 - Persistence, Export, Docs, And Guardrails](phase-6.md)

Finish the persistence/export axis, update source-of-truth docs, remove obsolete compatibility names,
and add precise guardrails for the stable boundaries created by earlier phases. The checks should
catch future product-mode shortcuts in generic room helpers, snapshot diagnostics, client
affordances, and lab mutation routing. This phase should close the room capability model and document
which product-mode references remain intentionally at setup or routing edges.

## Phase Index

1. [Phase 1 - Capability Baseline](phase-1.md)
2. [Phase 2 - Policy Bundle And Mutation Axis](phase-2.md)
3. [Phase 3 - Room-Controlled Time Contract](phase-3.md)
4. [Phase 4 - Projection And Diagnostics Contract](phase-4.md)
5. [Phase 5 - Client Capability Affordances](phase-5.md)
6. [Phase 6 - Persistence, Export, Docs, And Guardrails](phase-6.md)

## Non-Goals

- Do not implement new lab product features just because lab is a consumer of the policy model.
- Do not prioritize lab timeline controls, rewind, branch-from-lab, keyframe editing, god mode, inert
  units, disabled damage, visual hot reload, or scenario persistence libraries in this plan.
- Do not migrate `/dev/scenario` out of existence unless a phase explicitly proves the shared
  capability boundary can represent the current dev workflow without broad behavior churn.
- Do not turn every possible capability combination into a supported user-facing mode.
- Do not move room lifecycle, transport, database writes, AI ownership, or lab room-local session
  state into `rts-sim`.
- Do not weaken fog or client-trust boundaries while replacing mode-shaped contracts.

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait gate
and confirm the phase head is reachable from `origin/main`. For unattended executor passes, use:

```bash
scripts/phase-runner.sh --plan lab/room2 phase-1 --pr --wait
scripts/phase-runner.sh --plan lab/room2 --from 1 --to 6 --pr --wait
```
