# Ekat Phase 0/1 Working Checklists

Status: not started. This file is the working checklist for the active Ekat serial planning gate.

Sources:

- [docs/new-unit-checklist.md](../../docs/new-unit-checklist.md)
- [docs/new-building-checklist.md](../../docs/new-building-checklist.md)
- [plans/ekat/requirements.md](requirements.md)

Do not edit implementation files until every required per-entity Phase 0 brief item and Phase 1
rules item below is either approved or explicitly deferred by the user.

## Serial Queue

Only one entity may be active at a time. Complete the active entity's Phase 0 brief and Phase 1
rules/balance spec, get user review, and then move to the next entity.

- [ ] Phase 0: Global Gate.
- [ ] Phase 1: Ekat Hero/Body only.
- [ ] Phase 2: Zamok/Home Structure only.
- [ ] Phase 3: Golem only.
- [ ] Phase 4: Death Box only.
- [ ] Phase 5: Vortex only.
- [ ] Phase 6: Dash Building only.
- [ ] User explicitly approved moving beyond serial Phase 0/1 planning into implementation.

If the user changes the order, update [plan.md](plan.md), the relevant phase handoff, and this
queue before continuing. Do not fill checklist sections for later entities early.

## Global Gate

- [ ] Confirm whether the new requirements replace, hide, debug-gate, or evolve the current playable
      Ekat hero/Zamok slice.
- [ ] Confirm that the existing Ekat body and ability runtime are reused instead of redesigned.
- [ ] Confirm whether Ekat starts with no combat abilities until buildings unlock them.
- [ ] Confirm whether Ekat has no natural health regeneration.
- [ ] Confirm whether Steel, Oil, and Supply remain the only resources for this slice.
- [ ] Confirm whether AI support remains blocked for Ekat.
- [ ] Confirm whether local prediction remains disabled for Ekat.
- [ ] Record patch-note bullets as draft, factual notes only.
- [ ] Confirm no implementation files were edited during Phase 0/1.

## Entity Brief Items: Units And Actors

### Existing Ekat Hero/Body Stats And Ability Gate

- [ ] Current body and ability runtime are confirmed as implemented and reused.
- [ ] Player-facing stat rework goal is approved.
- [ ] Expected counters and failure modes from the stat rework are approved.
- [ ] Ability availability goal is approved: hidden, locked, debug-only, or another first slice.
- [ ] Unusual interactions are listed: command card, queued abilities, recast return, Magic Anchor,
      Golem consumption, death, replay, AI, and prediction.
- [ ] Initial exposure is approved: playable, debug-only, hidden, or blocked.
- [ ] Known unknowns are explicit.

### Golem

- [ ] Name and identity are approved.
- [ ] Player-facing UI description is approved.
- [ ] Strategic purpose is approved.
- [ ] Expected counters and failure modes are approved.
- [ ] Relationship to "four Kriegsia engineers" is approved as direction, rejected, or revised.
- [ ] Mining, transformation, and consumption roles are approved at a high level.
- [ ] Initial exposure is approved: playable, debug-only, hidden, or blocked.
- [ ] Known unknowns are explicit.

## Entity Brief Items: Buildings

### Zamok/Home Structure

- [ ] Name and identity are approved.
- [ ] Player-facing UI description is approved.
- [ ] Strategic purpose is approved.
- [ ] Relationship to Ekat mining, Golem production, supply, revival, and victory is approved.
- [ ] Creation rule is approved: match-start only, buildable, transformable, summonable, or another
      rule.
- [ ] Expected counters and failure modes are approved.
- [ ] Initial exposure is approved: playable, debug-only, hidden, or blocked.
- [ ] Known unknowns are explicit.

### Death Box

- [ ] Final or placeholder name status is approved.
- [ ] Player-facing UI description is approved.
- [ ] Strategic purpose is approved.
- [ ] Reason to choose it over Vortex and the Dash building is approved.
- [ ] Line Shot unlock fantasy and upgrade direction are approved.
- [ ] Creation rule from Golem transformation is approved or revised.
- [ ] Expected counters and failure modes are approved.
- [ ] Initial exposure is approved: playable, debug-only, hidden, or blocked.
- [ ] Known unknowns are explicit.

### Vortex

- [ ] Final or placeholder name status is approved.
- [ ] Player-facing UI description is approved.
- [ ] Strategic purpose is approved.
- [ ] Reason to choose it over Death Box and the Dash building is approved.
- [ ] Magic Anchor unlock fantasy and upgrade direction are approved.
- [ ] Creation rule from Golem transformation is approved or revised.
- [ ] Expected counters and failure modes are approved.
- [ ] Initial exposure is approved: playable, debug-only, hidden, or blocked.
- [ ] Known unknowns are explicit.

### Dash Building

- [ ] Final name for `XYZ` is approved.
- [ ] Player-facing UI description is approved.
- [ ] Strategic purpose is approved.
- [ ] Reason to choose it over Death Box and Vortex is approved.
- [ ] Dash unlock fantasy and upgrade direction are approved.
- [ ] Creation rule from Golem transformation is approved or revised.
- [ ] Expected counters and failure modes are approved.
- [ ] Initial exposure is approved: playable, debug-only, hidden, or blocked.
- [ ] Known unknowns are explicit.

## Entity Rules Items: Existing Ekat Hero/Body Stats And Ability Gate

- [ ] Cost is specified.
- [ ] Supply impact is specified.
- [ ] Starting loadout and creation source are specified.
- [ ] Hotkeys and command-card exposure are specified.
- [ ] Hit points are specified.
- [ ] Armor, armored status, tags, status immunities, and vulnerabilities are specified.
- [ ] Sight range is specified.
- [ ] Collision size, selection size, and render size are specified.
- [ ] Movement speed and movement semantics are specified.
- [ ] Health recovery, no-regeneration policy, Golem consumption, death, and revival are specified.
- [ ] Baseline combat policy is specified.
- [ ] Ability unlock behavior is specified for Dash, Line Shot, and Magic Anchor.
- [ ] Temporary behavior before unlock buildings exist is specified: hidden/locked, debug-only, or
      blocked from shipping.
- [ ] AI availability and intended AI usage are specified.

## Entity Rules Items: Golem

- [ ] Cost is specified.
- [ ] Supply impact is specified.
- [ ] Build source is specified.
- [ ] Build hotkey is specified.
- [ ] Build time is specified.
- [ ] Research or tech prerequisite is specified.
- [ ] Hit points are specified.
- [ ] Armor, armored status, tags, status immunities, and vulnerabilities are specified.
- [ ] Sight range is specified.
- [ ] Collision size, selection size, and render size are specified.
- [ ] Movement speed and movement semantics are specified.
- [ ] Mining target rules, range/proximity rules, cadence, and income are specified.
- [ ] Transformation rules are specified for each building.
- [ ] Consumption healing rules are specified.
- [ ] AI availability and intended AI usage are specified.

## Entity Rules Items: Zamok/Home Structure

- [ ] Creation source is specified.
- [ ] Command and hotkey are specified if buildable or interactable.
- [ ] Cost, refund, and cancellation are specified if buildable.
- [ ] Build or setup time is specified if buildable.
- [ ] Prerequisites, uniqueness, and unlock timing are specified.
- [ ] Hit points are specified.
- [ ] Armor, tags, repairability, capture rules, and vulnerabilities are specified.
- [ ] Footprint, placement grid, terrain restrictions, collision, and pathing interactions are
      specified.
- [ ] Selection size, render size, and minimap behavior are specified.
- [ ] Sight range, fog reveal, and remembered-building behavior are specified.
- [ ] Supply behavior is specified.
- [ ] Mining proximity, Golem production, hero revival, or other economy/tech behavior is specified.
- [ ] Death behavior and victory relevance are specified.
- [ ] AI availability and intended AI usage are specified.

## Entity Rules Items: Death Box

- [ ] Creation source is specified.
- [ ] Command and hotkey are specified.
- [ ] Transform cost, consumed Golem behavior, refund, and cancellation are specified.
- [ ] Transform time is specified.
- [ ] Prerequisites, build limit, and unlock timing are specified.
- [ ] Hit points are specified.
- [ ] Armor, tags, repairability, capture rules, and vulnerabilities are specified.
- [ ] Footprint, placement grid, terrain restrictions, collision, and pathing interactions are
      specified.
- [ ] Selection size, render size, and minimap behavior are specified.
- [ ] Sight range, fog reveal, and remembered-building behavior are specified.
- [ ] Supply behavior is specified.
- [ ] Line Shot unlock, upgrade costs, upgrade times, and loss-on-destruction behavior are
      specified.
- [ ] Death behavior is specified.
- [ ] AI availability and intended AI usage are specified.

## Entity Rules Items: Vortex

- [ ] Creation source is specified.
- [ ] Command and hotkey are specified.
- [ ] Transform cost, consumed Golem behavior, refund, and cancellation are specified.
- [ ] Transform time is specified.
- [ ] Prerequisites, build limit, and unlock timing are specified.
- [ ] Hit points are specified.
- [ ] Armor, tags, repairability, capture rules, and vulnerabilities are specified.
- [ ] Footprint, placement grid, terrain restrictions, collision, and pathing interactions are
      specified.
- [ ] Selection size, render size, and minimap behavior are specified.
- [ ] Sight range, fog reveal, and remembered-building behavior are specified.
- [ ] Supply behavior is specified.
- [ ] Magic Anchor unlock, upgrade costs, upgrade times, and loss-on-destruction behavior are
      specified.
- [ ] Death behavior is specified.
- [ ] AI availability and intended AI usage are specified.

## Entity Rules Items: Dash Building

- [ ] Final name replaces `XYZ`.
- [ ] Creation source is specified.
- [ ] Command and hotkey are specified.
- [ ] Transform cost, consumed Golem behavior, refund, and cancellation are specified.
- [ ] Transform time is specified.
- [ ] Prerequisites, build limit, and unlock timing are specified.
- [ ] Hit points are specified.
- [ ] Armor, tags, repairability, capture rules, and vulnerabilities are specified.
- [ ] Footprint, placement grid, terrain restrictions, collision, and pathing interactions are
      specified.
- [ ] Selection size, render size, and minimap behavior are specified.
- [ ] Sight range, fog reveal, and remembered-building behavior are specified.
- [ ] Supply behavior is specified.
- [ ] Dash unlock, upgrade costs, upgrade times, and loss-on-destruction behavior are specified.
- [ ] Death behavior is specified.
- [ ] AI availability and intended AI usage are specified.

## Future Implementation Permission

- [ ] User explicitly approved proceeding beyond Phase 1.
- [ ] Approved implementation scope is named exactly.
- [ ] Blocked mechanics are named exactly.
- [ ] Required design docs and context capsules for the approved implementation scope are listed.
