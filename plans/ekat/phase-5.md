# Phase 5 - Vortex Brief and Spec

Status: planned.

## Goal

Complete the new-building checklist Phase 0 brief and Phase 1 rules/balance spec for Vortex only.
This phase should define the Magic Anchor tech commitment before the Dash building is designed.

## Scope

- Read [docs/context/balance.md](../../docs/context/balance.md) before editing rules/spec details.
- Use [docs/new-building-checklist.md](../../docs/new-building-checklist.md) for this building.
- Complete only the Vortex sections in [checklists.md](checklists.md), or mark items as deferred
  with named unknowns.
- Specify Magic Anchor unlocks, upgrades, transformation tradeoff, destruction consequence, and
  first playable exposure only for Vortex.
- Update [requirements.md](requirements.md) only when a Vortex decision becomes approved product
  direction.

## Out of Scope

- Dash building brief/spec except for comparison questions needed to make Vortex's tradeoff clear.
- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Implementing the specified rules.
- Future implementation phase files.

## User Interview Focus

- Is Vortex the final name?
- Why does the player choose Vortex over Killing Tools or the Dash building?
- What should Magic Anchor and its upgrades do before numbers are chosen?
- Should Vortex change battlefield space, defense, pursuit, escape, or economy?
- What should happen to Magic Anchor access if Vortex is destroyed or transformed away?

## Expected Deliverables

- [checklists.md](checklists.md) updated only for Vortex Phase 0 and Phase 1 items.
- [requirements.md](requirements.md) updated only for confirmed Vortex product rules.
- No implementation files edited.

## Verification

- Documentation review only.
- Run `git diff --check` before committing.
- No automated gameplay suite is required for this docs-only phase.

## Manual Testing Focus

None. This phase has no gameplay change.

## Handoff Expectations

The handoff must name the approved Vortex brief and rules, unresolved tuning questions, and exactly
one next active entity. By default, the next active entity is the Dash building in
[phase-6.md](phase-6.md). If Vortex is not approved, the handoff must say that Dash-building work
remains blocked or reordered by user decision.
