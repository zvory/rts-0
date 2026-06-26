# Phase 4 - Killing Tools Brief and Spec

Status: complete; user decisions recorded in [checklists.md](checklists.md) and
[requirements.md](requirements.md).

## Goal

Complete the new-building checklist Phase 0 brief and Phase 1 rules/balance spec for Killing Tools,
formerly called Death Box in the draft. This phase should define the offensive attack tech
commitment and first Line Shot unlock before Anchorage or Positioning is designed.

## Scope

- Read [docs/context/balance.md](../../docs/context/balance.md) before editing rules/spec details.
- Use [docs/new-building-checklist.md](../../docs/new-building-checklist.md) for this building.
- Complete only the Killing Tools sections in [checklists.md](checklists.md), or mark items as
  deferred with named unknowns.
- Specify Line Shot unlocks, upgrades, transformation tradeoff, destruction consequence, and first
  playable exposure only for Killing Tools.
- Update [requirements.md](requirements.md) only when a Killing Tools decision becomes approved product
  direction.

## Out of Scope

- Anchorage or Positioning briefs/specs except for comparison questions needed to make Killing Tools'
  tradeoff clear.
- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Implementing the specified rules.
- Future implementation phase files.

## User Interview Focus

- Is Killing Tools the final name?
- Why does the player choose Killing Tools over Anchorage or Positioning?
- What should Line Shot and its upgrades do before numbers are chosen?
- Should Killing Tools be fragile, durable, hidden, obvious, attackable, or mainly a tech
  commitment?
- What should happen to Line Shot access if Killing Tools is destroyed or transformed away?

## Expected Deliverables

- [checklists.md](checklists.md) updated only for Killing Tools Phase 0 and Phase 1 items.
- [requirements.md](requirements.md) updated only for confirmed Killing Tools product rules.
- No implementation files edited.

## Verification

- Documentation review only.
- Run `git diff --check` before committing.
- No automated gameplay suite is required for this docs-only phase.

## Manual Testing Focus

None. This phase has no gameplay change.

## Handoff Expectations

The handoff must name the approved Killing Tools brief and rules, unresolved tuning questions, and
exactly one next active entity. By default, the next active entity is Anchorage in
[phase-5.md](phase-5.md). If Killing Tools is not approved, the handoff must say that later
tech-building work remains blocked or reordered by user decision.

## Handoff

Approved Killing Tools brief:

- Killing Tools replaces Death Box as the current name. It may still change later, but it is not a
  throwaway placeholder.
- Killing Tools is Ekat's offensive damage-dealing technology structure.
- First playable scope: Killing Tools unlocks base Line Shot only.
- Long-term direction: Killing Tools becomes the place to customize Ekat attacks through alternate
  build choices such as Line Shot, fan-out behavior, return behavior, or other offensive variants.
- Killing Tools has no weapon or active combat behavior; it is a tech unlock structure.
- The reason to choose Killing Tools before Anchorage or Positioning is offensive kill pressure.

Approved Killing Tools rules:

- Golem transforms into Killing Tools for free except for permanently consuming the Golem.
- At least one completed Killing Tools structure unlocks base Line Shot.
- If all completed Killing Tools structures are destroyed, base Line Shot becomes locked/disabled
  again.
- Future upgrades or attack customizations researched at Killing Tools persist after research and
  are not lost or disabled when Killing Tools structures are destroyed.
- First implementation includes no upgrades or broader attack customizations.
- Max HP: 165, matching the current R&D Complex.
- Footprint: 3x3.
- Sight: 1 tile by default, matching current R&D Complex sight.
- Armor: armored.
- Supply: none provided or used.
- No weapon or active combat behavior.
- AI support and local prediction may remain disabled indefinitely for Ekat.

Unresolved tuning questions:

- Final name, if Killing Tools changes later.
- Exact transform command and hotkey.
- Exact transform completion timing and low-HP starting profile.
- Exact upgrade/customization list, costs, effects, and timing.
- Exact relationship between future basic offensive attacks and Line Shot.
- Any implementation-specific repair, tag, or vulnerability difference from ordinary tech-building
  defaults.

Next active entity:

- Anchorage in [phase-5.md](phase-5.md).

No implementation files were edited.
