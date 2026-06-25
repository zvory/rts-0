# Phase 1 - Rules and Balance Specification

Status: planned.

## Goal

Turn the approved Phase 0 briefs into reviewable rules and balance specs for Ekat, Zamok, Golem,
Death Box, Vortex, and the Dash building. The output should be detailed enough that a later
implementation phase can estimate code, protocol, balance mirror, UI, art, test, and manual-review
scope without inventing product rules.

## Scope

- Read [docs/context/balance.md](../../docs/context/balance.md) before editing Phase 1 specs.
- Complete the Phase 1 sections in [checklists.md](checklists.md), or mark items as deferred with
  named unknowns.
- Specify exact values where the user approves them.
- Specify placeholder ranges only when the user explicitly wants a range instead of a final number.
- Identify every future cross-file contract touched by the spec: balance mirror, protocol mirror,
  `Game` API seam, fog rules, hardening limits, AI, prediction, replay, or design docs.
- Update [requirements.md](requirements.md) only when a Phase 1 rule becomes approved product
  direction.

## Out of Scope

- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Implementing the specified rules.
- Broad balance tuning beyond the first approved spec.
- Future implementation phase files unless the user explicitly approves moving beyond the Phase 1
  gate.

## Rules To Specify

### Ekat Hero/Body

- Starting state, owner, selection behavior, command-card role, and whether the current Ekat entity
  is reused.
- Cost, supply impact, buildability, respawn or revival rules, and match-start loadout.
- Hit points, armor/tags, sight, collision size, selection size, render size, movement speed, and
  movement semantics.
- Direct mining command, valid resource targets, Zamok proximity requirement, deposit cadence,
  income rate, interruption behavior, and UI feedback.
- Natural regeneration policy, Golem-consumption healing, death behavior, and comeback behavior.
- Combat policy before tech buildings unlock abilities: no attack, basic attack, or another rule.
- Ability access policy for Dash, Line Shot, and Magic Anchor before and after buildings exist.

### Zamok/Home Structure

- Starting count, uniqueness, buildability, repairability, victory relevance, and death consequence.
- Hit points, armor/tags, footprint, collision, placement rules, sight, supply provided, and
  selection/render size.
- Mining proximity radius, resource-drop-off rules, Golem production role, hero revival role, and
  any passive economy behavior.
- Whether Zamok is visible, remembered, targetable, capturable, reclaimable, or transformable.

### Golem

- Cost, supply impact, build source, build hotkey, build time, prerequisites, cap, and queueing.
- Hit points, armor/tags, sight, collision size, selection size, render size, movement speed, and
  pathing semantics.
- Mining rate, valid resource targets, Zamok proximity rules, deposit cadence, and interruption
  behavior.
- Transformation command rules, allowed target buildings, transformation time, cancellation/refund,
  vulnerability while transforming, and whether transformation is reversible.
- Consumption command rules, healing amount, target restrictions, timing, cancellation, and what
  happens if Ekat or the Golem dies during the command.

### Death Box

- Final name, command source, hotkey, transform cost, transform time, prerequisites, build limit,
  and whether it consumes a Golem permanently.
- Hit points, armor/tags, footprint, collision, placement rules, sight, supply behavior, and death
  consequence.
- Line Shot unlock rule, upgrade list, upgrade costs/times, disabled state before unlock, and
  behavior if the building dies.
- UI command-card behavior, player messaging, fog visibility, remembered-building behavior, and
  manual inspection scenario needs.

### Vortex

- Final name, command source, hotkey, transform cost, transform time, prerequisites, build limit,
  and whether it consumes a Golem permanently.
- Hit points, armor/tags, footprint, collision, placement rules, sight, supply behavior, and death
  consequence.
- Magic Anchor unlock rule, upgrade list, upgrade costs/times, disabled state before unlock, and
  behavior if the building dies.
- UI command-card behavior, player messaging, fog visibility, remembered-building behavior, and
  manual inspection scenario needs.

### Dash Building

- Final name for `XYZ`, command source, hotkey, transform cost, transform time, prerequisites,
  build limit, and whether it consumes a Golem permanently.
- Hit points, armor/tags, footprint, collision, placement rules, sight, supply behavior, and death
  consequence.
- Dash unlock rule, upgrade list, upgrade costs/times, disabled state before unlock, and behavior
  if the building dies.
- UI command-card behavior, player messaging, fog visibility, remembered-building behavior, and
  manual inspection scenario needs.

## User Engagement Process

Use the Phase 0 briefs as the starting point and ask for numbers only when the role is stable. When
the user is unsure, present a small comparison against current Kriegsia or current implemented Ekat
values, then record the user's choice, range, or deferral. Do not fill missing numbers just because
the implementation would need them later.

For each entity, ask the user to confirm:

- first playable version;
- deliberately deferred polish;
- what should be watched in playtests;
- whether the current implemented Ekat behavior should be preserved, replaced, hidden, or treated
  as debug-only.

## Expected Deliverables

- [checklists.md](checklists.md) updated with Phase 1 numbers, rules, deferrals, and blocked items.
- [requirements.md](requirements.md) updated only for confirmed product rules.
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

The handoff must name the approved spec files, list explicit user-approved numbers and rules,
identify unresolved tuning questions, and state exactly what a future implementation phase may
build. If any major faction mechanic is not approved, the handoff must say that implementation
remains blocked for that mechanic.
