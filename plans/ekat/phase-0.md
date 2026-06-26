# Phase 0 - Serial Gate and Global Identity

Status: complete; user decisions recorded in [checklists.md](checklists.md) and
[requirements.md](requirements.md).

## Goal

Lock the serial user-engagement process and resolve global Ekat identity questions before any
individual unit or building is briefed. This phase exists because Ekat includes several related
entities, and designing them in parallel would blur tradeoffs that should be player-facing choices.

## Scope

- Review the existing Ekat requirements draft with the user as hypotheses, not final decisions.
- Reconcile the draft with the current playable Ekat hero/Zamok slice documented in the design
  docs.
- Confirm the serial entity order in [plan.md](plan.md) or record a user-approved change to that
  order.
- Complete only the "Global Gate" section in [checklists.md](checklists.md), or mark items as
  deferred with named unknowns.
- Update [requirements.md](requirements.md) only when the user confirms a global product decision
  or when an ambiguity must be recorded.
- Start draft patch-note bullets for any later player-facing direction that would matter.

## Out of Scope

- Briefing or speccing Ekat, Zamok, Golem, Death Box, Vortex, or the Dash building in detail.
- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Exact stat, cost, cooldown, timing, radius, footprint, or mining-rate implementation.
- Future implementation phase files.
- Recreating purged RTS-style Ekat content without explicit user approval.

## Required User Decisions

- Is the controlled hero/body called Ekat, Zamok, or something else?
- Is Zamok a home structure, a hero title, a starting base, or another concept?
- Does the new requirements draft replace the current playable Ekat slice, layer on top of it,
  debug-gate it, or stay hidden until it is complete?
- Should Ekat start with no combat abilities until tech buildings exist, even though the current
  implementation exposes Dash, Line Shot, and Magic Anchor?
- Should Ekat have no natural health regeneration, replacing the current implemented regeneration?
- Are Steel, Oil, and Supply still the only resources for this slice?
- Should AI and prediction remain blocked for Ekat in the first implementation pass?
- Is the serial entity order approved: Ekat, Zamok, Golem, Death Box, Vortex, then Dash building?

## Serial Handoff Rule

The handoff must name exactly one next active entity. By default, that next entity is the Ekat
hero/body in [phase-1.md](phase-1.md). No other entity brief or spec should start until that phase
is user-reviewed.

## Expected Deliverables

- [checklists.md](checklists.md) updated only for the Global Gate section.
- [requirements.md](requirements.md) updated only for confirmed global product decisions or
  recorded ambiguity.
- Draft patch-note bullets for any later player-facing behavior implied by the global decisions.
- No implementation files edited.

## Verification

- Documentation review only.
- Run `git diff --check` before committing.
- No automated gameplay suite is required for this docs-only phase.

## Manual Testing Focus

None. This phase has no gameplay change.

## Handoff Expectations

The handoff must list approved global decisions, unresolved global questions, rejected ideas, the
single next active entity, and any current-implementation behavior that the new global direction
intends to replace. It must also say that no implementation files were edited.

## Handoff

Approved global decisions:

- Ekat is the faction name and hero/body name. It is one word, short for Ekaterina.
- Zamok is the home base structure and core building.
- The new requirements replace the current playable Ekat/Zamok slice when ready.
- The existing Ekat body, visuals, abilities, and ability runtime are reused.
- Ekat starts with no unlocked combat abilities; locked abilities remain visible but disabled.
- Ekat has no natural health regeneration.
- Steel, Oil, and Supply remain the only resources for this slice.
- AI support and local prediction may remain disabled for Ekat indefinitely.

Unresolved global questions:

- None blocking Phase 1. HP scaling, cloning, revival/comeback mechanics, and AI/prediction support
  are deferred future design topics.

Rejected or replaced current behavior:

- The current prototype behavior where Dash, Line Shot, and Magic Anchor are available immediately
  is replaced by building-based unlocks.
- The current 1 HP/s Ekat regeneration is replaced by no natural regeneration.

Next active entity:

- Ekat hero/body stat and ability gate in [phase-1.md](phase-1.md).

No implementation files were edited.
