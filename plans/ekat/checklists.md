# Ekat Phase 0/1 Working Checklists

Status: not started. This file is the working checklist for the active Ekat planning gate.

Sources:

- [docs/new-unit-checklist.md](../../docs/new-unit-checklist.md)
- [docs/new-building-checklist.md](../../docs/new-building-checklist.md)
- [plans/ekat/requirements.md](requirements.md)

Do not edit implementation files until every required Phase 0 and Phase 1 item below is either
approved or explicitly deferred by the user.

## Global Gate

- [ ] Confirm whether the new requirements replace, hide, debug-gate, or evolve the current playable
      Ekat hero/Zamok slice.
- [ ] Confirm whether Ekat starts with no combat abilities until buildings unlock them.
- [ ] Confirm whether Ekat has no natural health regeneration.
- [ ] Confirm whether Steel, Oil, and Supply remain the only resources for this slice.
- [ ] Confirm whether AI support remains blocked for Ekat.
- [ ] Confirm whether local prediction remains disabled for Ekat.
- [ ] Record patch-note bullets as draft, factual notes only.
- [ ] Confirm no implementation files were edited during Phase 0/1.

## Phase 0: Unit And Actor Briefs

### Ekat Hero/Body

- [ ] Name and identity are approved.
- [ ] Player-facing UI description is approved.
- [ ] Strategic purpose is approved.
- [ ] Expected counters and failure modes are approved.
- [ ] Direct mining fantasy and player action are approved.
- [ ] Unusual interactions are listed: Zamok proximity, fog, queueing, abilities, Golem consumption,
      death, replay, AI, and prediction.
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

## Phase 0: Building Briefs

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

## Phase 1: Ekat Hero/Body Rules

- [ ] Cost is specified.
- [ ] Supply impact is specified.
- [ ] Starting loadout and creation source are specified.
- [ ] Hotkeys and command-card exposure are specified.
- [ ] Hit points are specified.
- [ ] Armor, armored status, tags, status immunities, and vulnerabilities are specified.
- [ ] Sight range is specified.
- [ ] Collision size, selection size, and render size are specified.
- [ ] Movement speed and movement semantics are specified.
- [ ] Direct mining target rules, range/proximity rules, cadence, and income are specified.
- [ ] Health recovery, no-regeneration policy, Golem consumption, death, and revival are specified.
- [ ] Baseline combat policy is specified.
- [ ] Ability unlock behavior is specified for Dash, Line Shot, and Magic Anchor.
- [ ] AI availability and intended AI usage are specified.

## Phase 1: Golem Rules

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

## Phase 1: Zamok/Home Structure Rules

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

## Phase 1: Death Box Rules

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

## Phase 1: Vortex Rules

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

## Phase 1: Dash Building Rules

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
