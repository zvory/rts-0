# Ekat Phase 0/1 Working Checklists

Status: Global Gate and Ekat hero/body decisions recorded. This file is the working checklist for
the active Ekat serial planning gate.

Sources:

- [docs/new-unit-checklist.md](../../docs/new-unit-checklist.md)
- [docs/new-building-checklist.md](../../docs/new-building-checklist.md)
- [plans/ekat/requirements.md](requirements.md)

Do not edit implementation files until every required per-entity Phase 0 brief item and Phase 1
rules item below is either approved or explicitly deferred by the user.

## Serial Queue

Only one entity may be active at a time. Complete the active entity's Phase 0 brief and Phase 1
rules/balance spec, get user review, and then move to the next entity.

- [x] Phase 0: Global Gate.
- [x] Phase 1: Ekat Hero/Body only.
- [ ] Phase 2: Zamok/Home Structure only.
- [ ] Phase 3: Golem only.
- [ ] Phase 4: Death Box only.
- [ ] Phase 5: Vortex only.
- [ ] Phase 6: Dash Building only.
- [ ] User explicitly approved moving beyond serial Phase 0/1 planning into implementation.

If the user changes the order, update [plan.md](plan.md), the relevant phase handoff, and this
queue before continuing. Do not fill checklist sections for later entities early.

## Global Gate

- [x] Confirm whether the new requirements replace, hide, debug-gate, or evolve the current playable
      Ekat hero/Zamok slice.
- [x] Confirm that the existing Ekat body and ability runtime are reused instead of redesigned.
- [x] Confirm whether Ekat starts with no combat abilities until buildings unlock them.
- [x] Confirm whether Ekat has no natural health regeneration.
- [x] Confirm whether Steel, Oil, and Supply remain the only resources for this slice.
- [x] Confirm whether AI support remains blocked for Ekat.
- [x] Confirm whether local prediction remains disabled for Ekat.
- [x] Record patch-note bullets as draft, factual notes only.
- [x] Confirm no implementation files were edited during Phase 0/1.

Approved global decisions:

- Ekat is the faction name and hero/body name. It is one word, short for Ekaterina.
- Zamok is the home base structure and core building.
- The new direction replaces the current playable Ekat/Zamok slice when ready.
- The current Ekat body, visuals, abilities, and ability runtime are reused.
- Ekat starts without unlocked combat abilities; locked abilities stay visible but disabled.
- Ekat has no natural health regeneration.
- Steel, Oil, and Supply remain the only resources for this slice.
- AI support and local prediction may remain disabled for Ekat indefinitely.

Draft patch-note bullets:

- Ekat will move from the current prototype slice toward a Zamok-proximity hero economy with Golem
  tech conversion.
- Ekat abilities remain mechanically familiar but become locked until the matching tech buildings
  unlock them.
- Ekat loses natural regeneration and relies on Golem consumption for full healing once that
  mechanic exists.
- Ekat's starting combat body target changes from 300 HP and current fast movement to 150 HP and
  1.6 px/tick Rifleman movement speed.

## Entity Brief Items: Units And Actors

### Existing Ekat Hero/Body Stats And Ability Gate

- [x] Current body and ability runtime are confirmed as implemented and reused.
- [x] Player-facing stat rework goal is approved.
- [x] Expected counters and failure modes from the stat rework are approved.
- [x] Ability availability goal is approved: hidden, locked, debug-only, or another first slice.
- [x] Unusual interactions are listed: command card, queued abilities, recast return, Magic Anchor,
      Golem consumption, death, replay, AI, and prediction.
- [x] Initial exposure is approved: playable, debug-only, hidden, or blocked.
- [x] Known unknowns are explicit.

Approved Ekat hero/body brief:

- Ekat is unique and starts at match start. Future cloning abilities are possible but deferred.
- Ekat has no basic attack and no unlocked combat abilities at match start.
- Locked Dash, Line Shot, and Magic Anchor command-card entries should be visible but disabled.
- The current visuals, body feel, and ability runtime are reused.
- The stat rework makes Ekat fragile at match start: 150 HP, 1.6 px/tick Rifleman movement speed,
  9-tile sight, existing radius/selection/render feel, no default attack, no regeneration, and no
  recovery until Golem consumption exists.
- Ekat dying causes immediate player loss for the first implementation target.
- Initial exposure remains playable when this direction replaces the current prototype slice.
- Expected failure mode: Ekat can be punished if exposed, overextended, or separated from the
  Zamok/Golem economy; the faction's recovery depends on preserving or producing Golems.
- Known unknowns: HP scaling mechanism, future cloning, future revival/comeback rules, exact Golem
  consumption command flow, replay support, and any future AI/prediction support.

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

- [x] Cost is specified.
- [x] Supply impact is specified.
- [x] Starting loadout and creation source are specified.
- [x] Hotkeys and command-card exposure are specified.
- [x] Hit points are specified.
- [x] Armor, armored status, tags, status immunities, and vulnerabilities are specified.
- [x] Sight range is specified.
- [x] Collision size, selection size, and render size are specified.
- [x] Movement speed and movement semantics are specified.
- [x] Health recovery, no-regeneration policy, Golem consumption, death, and revival are specified.
- [x] Baseline combat policy is specified.
- [x] Ability unlock behavior is specified for Dash, Line Shot, and Magic Anchor.
- [x] Temporary behavior before unlock buildings exist is specified: hidden/locked, debug-only, or
      blocked from shipping.
- [x] AI availability and intended AI usage are specified.

Approved Ekat hero/body rules:

- Cost: none; Ekat is a starting hero and is not normally produced.
- Supply: 0.
- Starting loadout and creation source: one unique Ekat starts with each Ekat player at match start.
- Command-card exposure: Dash, Line Shot, and Magic Anchor stay visible but disabled while locked.
- HP: 150 at match start; future HP scaling mechanism deferred.
- Armor/tags/status rules: reuse current Ekat defaults unless a later implementation pass names a
  specific mismatch.
- Sight: 9 tiles.
- Size: reuse current collision radius, selection size, render size, and visual feel.
- Movement: 1.6 px/tick, matching Rifleman movement speed, and otherwise use ordinary current Ekat
  movement semantics.
- Recovery: no natural regeneration; no recovery until Golem consumption exists; Golem consumption
  heals Ekat to full HP once implemented.
- Death/revival: Ekat death causes immediate player loss for the first implementation target; no
  revival rule in this slice.
- Baseline combat: no basic attack.
- Ability unlocks: Death Box unlocks Line Shot, Vortex unlocks Magic Anchor, and the Dash building
  currently named `XYZ` unlocks Dash.
- Temporary behavior: until unlock buildings exist, abilities should remain visible but disabled
  rather than being freely usable.
- AI/prediction: AI support and local prediction may remain disabled indefinitely.

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
