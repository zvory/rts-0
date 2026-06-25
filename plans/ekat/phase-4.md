# Phase 4 - Death Box Brief and Spec

Status: planned.

## Goal

Complete the new-building checklist Phase 0 brief and Phase 1 rules/balance spec for Death Box
only. This phase should define the Line Shot tech commitment before Vortex or the Dash building is
designed.

## Scope

- Read [docs/context/balance.md](../../docs/context/balance.md) before editing rules/spec details.
- Use [docs/new-building-checklist.md](../../docs/new-building-checklist.md) for this building.
- Complete only the Death Box sections in [checklists.md](checklists.md), or mark items as
  deferred with named unknowns.
- Specify Line Shot unlocks, upgrades, transformation tradeoff, destruction consequence, and first
  playable exposure only for Death Box.
- Update [requirements.md](requirements.md) only when a Death Box decision becomes approved product
  direction.

## Out of Scope

- Vortex or Dash building briefs/specs except for comparison questions needed to make Death Box's
  tradeoff clear.
- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Implementing the specified rules.
- Future implementation phase files.

## User Interview Focus

- Is Death Box the final name?
- Why does the player choose Death Box over Vortex or the Dash building?
- What should Line Shot and its upgrades do before numbers are chosen?
- Should Death Box be fragile, durable, hidden, obvious, attackable, or mainly a tech commitment?
- What should happen to Line Shot access if Death Box is destroyed or transformed away?

## Expected Deliverables

- [checklists.md](checklists.md) updated only for Death Box Phase 0 and Phase 1 items.
- [requirements.md](requirements.md) updated only for confirmed Death Box product rules.
- No implementation files edited.

## Verification

- Documentation review only.
- Run `git diff --check` before committing.
- No automated gameplay suite is required for this docs-only phase.

## Manual Testing Focus

None. This phase has no gameplay change.

## Handoff Expectations

The handoff must name the approved Death Box brief and rules, unresolved tuning questions, and
exactly one next active entity. By default, the next active entity is Vortex in
[phase-5.md](phase-5.md). If Death Box is not approved, the handoff must say that later
tech-building work remains blocked or reordered by user decision.
