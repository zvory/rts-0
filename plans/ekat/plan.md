# Ekat Hero Economy Plan

Status: serial planning gate active. This plan turns the current Ekat requirements draft into
user-reviewed briefs and rules specs before any implementation work starts.

## Purpose

Ekat is already present in the repo as a small playable hero/Zamok slice, while
[requirements.md](requirements.md) proposes a different next direction: direct hero mining, Golems,
Golem-converted tech buildings, and ability unlocks through those buildings. This plan does not
approve implementation of that direction by itself. Its job is to create a deliberate user-facing
serial process for approving the hero, Golem, Zamok/home structure, and each tech building before
code, balance, protocol, art, or tests change.

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

Phase 0 locks the serial process and resolves global Ekat identity questions before any individual
entity is briefed. It decides whether the new draft replaces, hides, debug-gates, or evolves the
current playable Ekat slice. It ends by naming the first active entity and confirming no
implementation files were edited.

Phase 1 handles only the existing Ekat hero/body stat rework and ability availability gate. The
Ekat body, Dash, Line Shot, Magic Anchor, return marker, projectile, and Magic Anchor runtime
already exist; this phase must not redesign or reimplement them. It only approves exact hero stat
changes and the rule that those existing abilities are unavailable at match start and become
available through explicit unlock sources.

Phase 2 handles only Zamok as the home structure. It completes the new-building checklist Phase 0
brief and Phase 1 rules/balance spec for mining proximity, starting-state, supply, revival, and
destruction consequences. It stops for user review before Golem or tech-building work starts.

Phase 3 handles only Golem. It completes the new-unit checklist Phase 0 brief and Phase 1
rules/balance spec for production, mining, transformation, supply, vulnerability, and consumption
healing. It stops for user review before any Golem-converted tech building is briefed.

Phase 4 handles only Death Box. It completes the new-building checklist Phase 0 brief and Phase 1
rules/balance spec for the Line Shot unlock family, transformation tradeoff, destruction
consequence, and upgrade direction. It stops for user review before Vortex work starts.

Phase 5 handles only Vortex. It completes the new-building checklist Phase 0 brief and Phase 1
rules/balance spec for the Magic Anchor unlock family, transformation tradeoff, destruction
consequence, and upgrade direction. It stops for user review before the Dash building work starts.

Phase 6 handles only the Dash building currently named `XYZ`. It completes the new-building
checklist Phase 0 brief and Phase 1 rules/balance spec for the Dash unlock family, final name,
transformation tradeoff, destruction consequence, and upgrade direction. It stops for user review
before any implementation phase is written.

## Phase Index

0. [Phase 0 - Serial Gate and Global Identity](phase-0.md)
1. [Phase 1 - Ekat Stats and Ability Unlock Gate](phase-1.md)
2. [Phase 2 - Zamok Brief and Spec](phase-2.md)
3. [Phase 3 - Golem Brief and Spec](phase-3.md)
4. [Phase 4 - Death Box Brief and Spec](phase-4.md)
5. [Phase 5 - Vortex Brief and Spec](phase-5.md)
6. [Phase 6 - Dash Building Brief and Spec](phase-6.md)

Future implementation phase files are intentionally not authored yet. Per the new unit and new
building workflow gates, each entity phase stops after that entity's Phase 0/1 checklist until the
user explicitly approves moving to the next entity. This effort stops after Phase 6 until the user
explicitly approves implementation scope.

## Overall Constraints

- Do not edit Rust, JavaScript, protocol, generated config, tests, art, sound, or other
  implementation files during this planning gate.
- Work serially. Only one entity or existing entity slice may be active at a time, and the next
  brief/spec must not start until the current slice has a user-reviewed Phase 0 brief and Phase 1
  rules/balance spec.
- Treat every unchecked item in [checklists.md](checklists.md) as unresolved. If the user chooses a
  direction but not a number, record the direction and leave the number deferred.
- Do not recreate purged RTS-style Ekat content such as workers, conscripts, signal teams, command
  posts, supply caches, workshops, a standard RTS loadout, or Mark Target unless the user approves
  that content by name.
- Do not assume the current playable Ekat slice is the final target. Phase 0 must explicitly decide
  whether the new design replaces, hides, debug-gates, or incrementally evolves the current slice.
- Treat Ekat's current body and ability mechanics as implemented. Future work may tune stats and
  gate ability availability, but should not rebuild Dash, Line Shot, Magic Anchor, return markers,
  line projectiles, or anchor projection unless a later bug or design change explicitly requires it.
- Keep the faction hero-centric unless the user changes that requirement. Golems and buildings are
  supporting strategic choices, not a broad RTS roster by default.
- Apply the new unit checklist to Ekat and Golem, and the new building checklist to each building.
- Collect patch-note bullets as factual draft notes only. No player-facing change exists until a
  later implementation PR lands.
- If an entity phase discovers a wire, balance mirror, fog, `Game` API, AI, prediction, or replay
  contract change, name it as a future implementation constraint instead of implementing it.

## User Engagement Process

The phase author should work as a facilitator, not a silent spec writer. For each active entity,
present the current draft as a hypothesis, ask the user to make the core tradeoff decisions, and
record the answers in plain language. Use narrow questions that force a real choice: what the
entity is for, why the player chooses it over another option, what beats it, what failure should
feel like, and what should be delayed until playtesting.

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

When Phase 6 is complete, the handoff must state exactly which implementation files are still
untouched and whether the user has approved proceeding to code.
