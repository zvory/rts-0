# Phase 5 - Anchorage Brief and Spec

Status: complete; user decisions recorded in [checklists.md](checklists.md) and
[requirements.md](requirements.md).

## Goal

Complete the new-building checklist Phase 0 brief and Phase 1 rules/balance spec for Anchorage,
formerly called Vortex in the draft. This phase should define the anchor-placement tech commitment
and first Magic Anchor unlock before Positioning is designed.

## Scope

- Read [docs/context/balance.md](../../../docs/context/balance.md) before editing rules/spec details.
- Use [docs/new-building-checklist.md](../../../docs/new-building-checklist.md) for this building.
- Complete only the Anchorage sections in [checklists.md](checklists.md), or mark items as deferred
  with named unknowns.
- Specify Magic Anchor unlocks, upgrades, transformation tradeoff, destruction consequence, and
  first playable exposure only for Anchorage.
- Update [requirements.md](requirements.md) only when an Anchorage decision becomes approved product
  direction.

## Out of Scope

- Positioning brief/spec except for comparison questions needed to make Anchorage's tradeoff
  clear.
- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Implementing the specified rules.
- Future implementation phase files.

## User Interview Focus

- Is Anchorage the final name?
- Why does the player choose Anchorage over Killing Tools or Positioning?
- What should Magic Anchor and its upgrades do before numbers are chosen?
- Should Anchorage change battlefield space, defense, pursuit, escape, or economy?
- What should happen to Magic Anchor access if Anchorage is destroyed or transformed away?

## Expected Deliverables

- [checklists.md](checklists.md) updated only for Anchorage Phase 0 and Phase 1 items.
- [requirements.md](requirements.md) updated only for confirmed Anchorage product rules.
- No implementation files edited.

## Verification

- Documentation review only.
- Run `git diff --check` before committing.
- No automated gameplay suite is required for this docs-only phase.

## Manual Testing Focus

None. This phase has no gameplay change.

## Handoff Expectations

The handoff must name the approved Anchorage brief and rules, unresolved tuning questions, and exactly
one next active entity. By default, the next active entity is Positioning in
[phase-6.md](phase-6.md). If Anchorage is not approved, the handoff must say that Positioning work
remains blocked or reordered by user decision.

## Handoff

Approved Anchorage brief:

- Anchorage replaces Vortex as the current building name.
- The current Magic Anchor implementation should probably be renamed to Vortex later.
- Anchorage is Ekat's anchor-placement technology structure: the player places anchors, and those
  anchors do things.
- First playable scope: Anchorage unlocks the current Magic Anchor implementation only.
- Long-term direction: Anchorage hosts future anchor customizations or alternate anchor builds.
- Killing Tools is expected to be the first-priority tech for raw pressure; Anchorage's exact
  strategic priority versus Killing Tools and Positioning is deferred to playtesting.
- Anchorage has no weapon or active combat behavior; it is a tech unlock structure.

Approved Anchorage rules:

- Golem transforms into Anchorage for free except for permanently consuming the Golem.
- At least one completed Anchorage structure unlocks the current Magic Anchor implementation.
- If all completed Anchorage structures are destroyed, Magic Anchor becomes locked/disabled again.
- Future upgrades or anchor customizations researched at Anchorage persist after research and are
  not lost or disabled when Anchorage structures are destroyed.
- First implementation includes no upgrades or broader anchor customizations.
- Max HP: 165, matching Killing Tools and the current R&D Complex.
- Footprint: 3x3.
- Sight: 1 tile by default, matching Killing Tools and current R&D Complex sight.
- Armor: armored.
- Supply: none provided or used.
- No weapon or active combat behavior.
- AI support and local prediction may remain disabled indefinitely for Ekat.

Unresolved tuning questions:

- Final ability name for the current Magic Anchor implementation, likely Vortex.
- Exact transform command and hotkey.
- Exact transform completion timing and low-HP starting profile.
- Exact anchor/customization list, costs, effects, and timing.
- Exact strategic reason to choose Anchorage before Killing Tools or Positioning.
- Any implementation-specific repair, tag, or vulnerability difference from ordinary tech-building
  defaults.

Next active entity:

- Positioning in [phase-6.md](phase-6.md).

No implementation files were edited.
