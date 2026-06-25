# Phase 3 - Golem Brief and Spec

Status: planned.

## Goal

Complete the new-unit checklist Phase 0 brief and Phase 1 rules/balance spec for Golem only. This
phase should define Golem as the economic and tech-conversion piece before any Golem-converted tech
building is designed.

## Scope

- Read [docs/context/balance.md](../../docs/context/balance.md) before editing rules/spec details.
- Use [docs/new-unit-checklist.md](../../docs/new-unit-checklist.md) for this unit.
- Complete only the Golem sections in [checklists.md](checklists.md), or mark items as deferred with
  named unknowns.
- Specify production, mining, transformation, supply, vulnerability, and consumption-healing policy
  only for Golem.
- Update [requirements.md](requirements.md) only when a Golem decision becomes approved product
  direction.

## Out of Scope

- Death Box, Vortex, or Dash building briefs/specs except for dependency questions needed to keep
  Golem transformation coherent.
- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Implementing the specified rules.
- Future implementation phase files.

## User Interview Focus

- Is a Golem a unit the player controls, a worker-like economic body, a tech currency, a temporary
  summon, or a hybrid?
- What should a Golem feel like compared with four Kriegsia engineers?
- What does the player give up when transforming a Golem into a building?
- Should Golems be vulnerable while mining, transforming, or being consumed for healing?
- Can multiple Golems exist at once, and is there a desired cap?
- Is Golem production playable in the initial implementation, debug-only, or hidden?

## Expected Deliverables

- [checklists.md](checklists.md) updated only for Golem Phase 0 and Phase 1 items.
- [requirements.md](requirements.md) updated only for confirmed Golem product rules.
- No implementation files edited.

## Verification

- Documentation review only.
- Run `git diff --check` before committing.
- No automated gameplay suite is required for this docs-only phase.

## Manual Testing Focus

None. This phase has no gameplay change.

## Handoff Expectations

The handoff must name the approved Golem brief and rules, unresolved tuning questions, and exactly
one next active entity. By default, the next active entity is Death Box in
[phase-4.md](phase-4.md). If Golem is not approved, the handoff must say that Golem-converted
building work remains blocked.
