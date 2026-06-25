# Ekat Hero Economy Plan

Status: planning gate active. This plan turns the current Ekat requirements draft into
user-reviewed briefs and rules specs before any implementation work starts.

## Purpose

Ekat is already present in the repo as a small playable hero/Zamok slice, while
[requirements.md](requirements.md) proposes a different next direction: direct hero mining, Golems,
Golem-converted tech buildings, and ability unlocks through those buildings. This plan does not
approve implementation of that direction by itself. Its job is to create a deliberate user-facing
process for approving the hero, Golem, Zamok/home structure, and each tech building before code,
balance, protocol, art, or tests change.

## Product Inputs

- [requirements.md](requirements.md) is the active draft input.
- [docs/new-unit-checklist.md](../../docs/new-unit-checklist.md) is mandatory for Ekat and Golem
  work.
- [docs/new-building-checklist.md](../../docs/new-building-checklist.md) is mandatory for Zamok,
  Death Box, Vortex, and the Dash building currently named `XYZ`.
- The archived clean-slate brief under `plans/archive/faction/ekat-brief.md` remains evidence that
  purged RTS-style Ekat content is not approved.
- Current design docs describe the implemented Ekat hero/Zamok slice. Phase 0 must reconcile that
  current slice with the new draft rather than assuming the draft silently replaces it.

## Phase Summaries

Phase 0 creates user-reviewed briefs for the Ekat-controlled hero/body, the Zamok/home structure,
Golem, Death Box, Vortex, and the Dash building. It asks one entity at a time what fantasy, role,
tradeoff, counterplay, UI description, and unusual interactions the player should experience. It
ends with brief checklists completed or explicitly deferred and no implementation files edited.

Phase 1 turns the approved Phase 0 briefs into rules and balance worksheets. It specifies or
defers the exact stats, costs, timings, unlocks, transformations, mining rules, consumption rules,
ability gates, and building consequences needed before implementation phases can be scoped. It
ends with a user-review handoff that names exactly what a later implementation phase may build and
what remains blocked.

## Phase Index

0. [Phase 0 - Entity Briefs and User Interviews](phase-0.md)
1. [Phase 1 - Rules and Balance Specification](phase-1.md)

Future implementation phase files are intentionally not authored yet. Per the new unit and new
building workflow gates, this effort stops after Phase 0 and Phase 1 until the user explicitly
approves implementation scope.

## Overall Constraints

- Do not edit Rust, JavaScript, protocol, generated config, tests, art, sound, or other
  implementation files during this planning gate.
- Treat every unchecked item in [checklists.md](checklists.md) as unresolved. If the user chooses a
  direction but not a number, record the direction and leave the number deferred.
- Do not recreate purged RTS-style Ekat content such as workers, conscripts, signal teams, command
  posts, supply caches, workshops, a standard RTS loadout, or Mark Target unless the user approves
  that content by name.
- Do not assume the current playable Ekat slice is the final target. Phase 0 must explicitly decide
  whether the new design replaces, hides, debug-gates, or incrementally evolves the current slice.
- Keep the faction hero-centric unless the user changes that requirement. Golems and buildings are
  supporting strategic choices, not a broad RTS roster by default.
- Apply the new unit checklist to Ekat and Golem, and the new building checklist to each building.
- Collect patch-note bullets as factual draft notes only. No player-facing change exists until a
  later implementation PR lands.
- If Phase 1 discovers a wire, balance mirror, fog, `Game` API, AI, prediction, or replay contract
  change, name it as a future implementation constraint instead of implementing it.

## User Engagement Process

The phase author should work as a facilitator, not a silent spec writer. For each entity, present
the current draft as a hypothesis, ask the user to make the core tradeoff decisions, and record the
answers in plain language. Use narrow questions that force a real choice: what the entity is for,
why the player chooses it over another option, what beats it, what failure should feel like, and
what should be delayed until playtesting.

Each user review pass should end with three artifacts:

- approved decisions, written as facts;
- deferred decisions, written as named unknowns;
- implementation permission, either explicitly withheld or explicitly granted for a later phase.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, pushed as an owned PR with
auto-merge armed, merged to `main`, and verified reachable from `origin/main` before the next phase
starts.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do, what user decisions were made, what remains unresolved, and what should be manually
reviewed. Manual testing notes are expected to be "none" for these docs-only gates unless a later
implementation phase is explicitly approved.

When Phase 1 is complete, the handoff must state exactly which implementation files are still
untouched and whether the user has approved proceeding to code.
