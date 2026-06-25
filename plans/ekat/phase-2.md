# Phase 2 - Zamok Brief and Spec

Status: planned.

## Goal

Complete the new-building checklist Phase 0 brief and Phase 1 rules/balance spec for Zamok only.
This phase should define the home structure's player-facing purpose before Golem or tech-building
details are designed.

## Scope

- Read [docs/context/balance.md](../../docs/context/balance.md) before editing rules/spec details.
- Use [docs/new-building-checklist.md](../../docs/new-building-checklist.md) for this building.
- Complete only the Zamok/Home Structure sections in [checklists.md](checklists.md), or mark items
  as deferred with named unknowns.
- Specify mining proximity, starting-state, supply, revival, victory relevance, and destruction
  policy only for Zamok.
- Update [requirements.md](requirements.md) only when a Zamok decision becomes approved product
  direction.

## Out of Scope

- Golem, Death Box, Vortex, or Dash building briefs/specs except for dependency questions needed to
  keep Zamok coherent.
- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Implementing the specified rules.
- Future implementation phase files.

## User Interview Focus

- Is Zamok required for mining deposits, Golem production, Ekat revival, supply, victory, or some
  combination?
- What should happen if Zamok is destroyed?
- Should Zamok be buildable, fixed at match start, transformable, repairable, movable, or unique?
- Should Zamok provide +10 supply as the current implementation does, or is that a compatibility
  detail to revisit?
- What should the opponent learn from scouting or damaging Zamok?

## Expected Deliverables

- [checklists.md](checklists.md) updated only for Zamok/Home Structure Phase 0 and Phase 1 items.
- [requirements.md](requirements.md) updated only for confirmed Zamok product rules.
- No implementation files edited.

## Verification

- Documentation review only.
- Run `git diff --check` before committing.
- No automated gameplay suite is required for this docs-only phase.

## Manual Testing Focus

None. This phase has no gameplay change.

## Handoff Expectations

The handoff must name the approved Zamok brief and rules, unresolved tuning questions, and exactly
one next active entity. By default, the next active entity is Golem in [phase-3.md](phase-3.md). If
Zamok is not approved, the handoff must say that Golem and later building work remains blocked.
