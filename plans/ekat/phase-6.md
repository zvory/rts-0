# Phase 6 - Dash Building Brief and Spec

Status: planned.

## Goal

Complete the new-building checklist Phase 0 brief and Phase 1 rules/balance spec for the Dash
building currently named `XYZ` only. This phase should define the Dash tech commitment and final
building name before any implementation phase is written.

## Scope

- Read [docs/context/balance.md](../../docs/context/balance.md) before editing rules/spec details.
- Use [docs/new-building-checklist.md](../../docs/new-building-checklist.md) for this building.
- Complete only the Dash Building sections in [checklists.md](checklists.md), or mark items as
  deferred with named unknowns.
- Specify Dash unlocks, upgrades, transformation tradeoff, destruction consequence, final name, and
  first playable exposure only for this building.
- Update [requirements.md](requirements.md) only when a Dash-building decision becomes approved
  product direction.

## Out of Scope

- Reopening Death Box or Vortex specs except for explicit user-requested comparison updates.
- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Implementing the specified rules.
- Future implementation phase files unless the user explicitly approves moving beyond the Phase 6
  gate.

## User Interview Focus

- What is the final name for the building currently called `XYZ`?
- Why does the player choose the Dash building over Death Box or Vortex?
- What should Dash and its upgrades do before numbers are chosen?
- Is Dash primarily escape, engage, repositioning, mining tempo, or something else?
- What should happen to Dash access if this building is destroyed or transformed away?

## Expected Deliverables

- [checklists.md](checklists.md) updated only for Dash Building Phase 0 and Phase 1 items.
- [requirements.md](requirements.md) updated only for confirmed Dash-building product rules.
- A short "Future Implementation Permission" section added to this phase document or the handoff,
  naming exactly what the user has approved for code.
- No implementation files edited.

## Verification

- Documentation review only.
- Run `git diff --check` before committing.
- No automated gameplay suite is required for this docs-only phase.

## Manual Testing Focus

None. This phase has no gameplay change.

## Handoff Expectations

The handoff must name the approved Dash-building brief and rules, list explicit user-approved
numbers and rules across the serial plan, identify unresolved tuning questions, and state exactly
what a future implementation phase may build. If any major faction mechanic is not approved, the
handoff must say that implementation remains blocked for that mechanic.
