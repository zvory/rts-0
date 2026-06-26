# Ekat Phase 0/1 Working Checklists

Status: Global Gate, Ekat hero/body, Zamok, Golem, and Killing Tools decisions recorded. This file
is the working checklist for the active Ekat serial planning gate.

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
- [x] Phase 2: Zamok/Home Structure only.
- [x] Phase 3: Golem only.
- [x] Phase 4: Killing Tools only.
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
- Zamok becomes an expensive, buildable City Centre-equivalent core structure that produces Golems,
  anchors Ekat/Golem mining, provides +10 Supply, and makes Ekat die if the player has no Zamoks.
- Golems become directly controllable worker-like economy units with 4 Supply, 160 HP, 4x worker
  mining, 16 worker-like attack damage, permanent building transformation, and proximity-gated Ekat
  healing.
- Killing Tools replaces the Death Box draft name for the offensive attack tech building. Its first
  playable scope is a base Line Shot unlock from a free Golem transform; future attack
  customizations are deferred.

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

- [x] Name and identity are approved.
- [x] Player-facing UI description is approved.
- [x] Strategic purpose is approved.
- [x] Expected counters and failure modes are approved.
- [x] Relationship to "four Kriegsia engineers" is approved as direction, rejected, or revised.
- [x] Mining, transformation, and consumption roles are approved at a high level.
- [x] Initial exposure is approved: playable, debug-only, hidden, or blocked.
- [x] Known unknowns are explicit.

Approved Golem brief:

- Name and identity: Golem is Ekat's directly controllable worker-like economy body and tech
  conversion piece.
- Player-facing description: heavy worker that mines, fights weakly in worker terms, can be
  consumed to heal Ekat, and can permanently become Ekat tech buildings.
- Strategic purpose: Golems concentrate worker value into fewer, more important bodies. The player
  chooses between economy, Ekat recovery, and permanent tech-building transformation.
- Four-worker relationship: Golem uses 4 Supply, has 160 HP, mines at 4x worker rate, and attacks
  for 16 damage using worker-like attack semantics.
- Mining role: Golems mine Steel or Oil near Zamok using the same Zamok proximity anchor direction
  approved for Ekat economy.
- Transformation role: Golems are permanently consumed when transformed into a tech building or any
  other approved Golem-transformed structure.
- Consumption role: Ekat can consume a nearby owned Golem to heal to full HP; exact range is
  deferred.
- Failure mode: killing or forcing commitment of Golems attacks Ekat's economy, tech path, and
  emergency healing reserve at the same time.
- Initial exposure: playable when the Ekat direction replaces the current prototype slice.
- Known unknowns: exact Golem build cost, build time, hotkey, command-card details, healing range,
  transformed-building starting HP profile, exact transform completion timing, size/render tuning,
  and any future cap beyond normal Supply.

## Entity Brief Items: Buildings

### Zamok/Home Structure

- [x] Name and identity are approved.
- [x] Player-facing UI description is approved.
- [x] Strategic purpose is approved.
- [x] Relationship to Ekat mining, Golem production, supply, revival, and victory is approved.
- [x] Creation rule is approved: match-start only, buildable, transformable, summonable, or another
      rule.
- [x] Expected counters and failure modes are approved.
- [x] Initial exposure is approved: playable, debug-only, hidden, or blocked.
- [x] Known unknowns are explicit.

Approved Zamok brief:

- Name and identity: Zamok is Ekat's home base structure and core building.
- Player-facing description: Ekat's core structure. Produces Golems, anchors mining, and provides
  Supply.
- Strategic purpose: Zamok fills the same role for Ekat that City Centre fills for Kriegsia, but
  with Ekat visuals and Golem production instead of worker production.
- Creation rule: each Ekat player starts with one Zamok, and additional Zamoks can be built at an
  expensive cost.
- Ekat/Golem relationship: Zamok anchors Ekat direct mining, Golem mining, Golem production, +10
  Supply, and Ekat survival.
- Failure mode: losing the last Zamok kills Ekat; Ekat death causes immediate defeat for the first
  implementation target.
- Counterplay: opponents can scout, pressure, and destroy Zamoks to shut down mining anchors, Golem
  production, Supply, and ultimately Ekat's survival.
- Initial exposure: playable when the Ekat direction replaces the current prototype slice.
- Known unknowns: exact expansion Zamok cost, builder/source command, hotkey, build time, repair
  actor, and any tuning differences from City Centre that playtests require.

### Killing Tools

- [x] Final or placeholder name status is approved.
- [x] Player-facing UI description is approved.
- [x] Strategic purpose is approved.
- [x] Reason to choose it over Vortex and the Dash building is approved.
- [x] Line Shot unlock fantasy and upgrade direction are approved.
- [x] Creation rule from Golem transformation is approved or revised.
- [x] Expected counters and failure modes are approved.
- [x] Initial exposure is approved: playable, debug-only, hidden, or blocked.
- [x] Known unknowns are explicit.

Approved Killing Tools brief:

- Name status: Killing Tools replaces Death Box as the current name. It may still change later, but
  it is not treated as a throwaway placeholder.
- Player-facing description: offensive attack technology structure. Unlocks Line Shot now and later
  hosts Ekat attack customizations.
- Strategic purpose: Killing Tools is the damage-dealing tech choice. The first playable reason to
  choose it over Vortex or the Dash building is access to offensive kill pressure through Line Shot.
- Long-term direction: Killing Tools should eventually unlock or customize a more basic offensive
  attack package, with Line Shot, fan-out behavior, return behavior, or other build variants as
  possible choices.
- First playable scope: only the base Line Shot unlock is included; upgrades and broader attack
  customization are deferred.
- Creation rule: a Golem transforms into Killing Tools for free except for permanently consuming
  that Golem.
- Failure mode: if all completed Killing Tools structures are destroyed, Line Shot becomes locked or
  disabled again.
- Upgrade persistence: future upgrades/customizations researched through Killing Tools are not lost
  or disabled when all Killing Tools structures are gone, though Line Shot access still requires a
  completed Killing Tools structure.
- Initial exposure: playable when the Ekat direction replaces the current prototype slice.
- Known unknowns: exact transform command/hotkey, transform completion timing, low-HP starting
  profile, upgrade/customization list, upgrade costs/times, final name, and any future relationship
  between a more basic offensive attack and Line Shot.

### Vortex

- [ ] Final or placeholder name status is approved.
- [ ] Player-facing UI description is approved.
- [ ] Strategic purpose is approved.
- [ ] Reason to choose it over Killing Tools and the Dash building is approved.
- [ ] Magic Anchor unlock fantasy and upgrade direction are approved.
- [ ] Creation rule from Golem transformation is approved or revised.
- [ ] Expected counters and failure modes are approved.
- [ ] Initial exposure is approved: playable, debug-only, hidden, or blocked.
- [ ] Known unknowns are explicit.

### Dash Building

- [ ] Final name for `XYZ` is approved.
- [ ] Player-facing UI description is approved.
- [ ] Strategic purpose is approved.
- [ ] Reason to choose it over Killing Tools and Vortex is approved.
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
- Ability unlocks: Killing Tools unlocks Line Shot, Vortex unlocks Magic Anchor, and the Dash
  building currently named `XYZ` unlocks Dash.
- Temporary behavior: until unlock buildings exist, abilities should remain visible but disabled
  rather than being freely usable.
- AI/prediction: AI support and local prediction may remain disabled indefinitely.

## Entity Rules Items: Golem

- [x] Cost is specified.
- [x] Supply impact is specified.
- [x] Build source is specified.
- [x] Build hotkey is specified.
- [x] Build time is specified.
- [x] Research or tech prerequisite is specified.
- [x] Hit points are specified.
- [x] Armor, armored status, tags, status immunities, and vulnerabilities are specified.
- [x] Sight range is specified.
- [x] Collision size, selection size, and render size are specified.
- [x] Movement speed and movement semantics are specified.
- [x] Mining target rules, range/proximity rules, cadence, and income are specified.
- [x] Transformation rules are specified for each building.
- [x] Consumption healing rules are specified.
- [x] AI availability and intended AI usage are specified.

Approved Golem rules:

- Cost: exact Golem cost is deferred. The first implementation should treat the four-worker
  relationship as the default cost direction unless a later tuning pass changes it.
- Supply: 4.
- Build source: Zamok builds Golems.
- Build hotkey: deferred until command-card implementation.
- Build time: deferred.
- Research/tech prerequisite: none for baseline Golem production from Zamok unless a later phase
  explicitly adds one.
- HP: 160.
- Armor/tags/status rules: worker-like defaults unless a later implementation pass names a specific
  Ekat difference.
- Sight: worker-like by default, currently 7 tiles.
- Size/render: worker-like control semantics with Golem-specific visuals; exact collision,
  selection, and render size are deferred for implementation/art fit.
- Movement: directly controllable worker-like ground movement, currently 2.0 px/tick.
- Attack: worker-like attack semantics with 16 damage, four times current worker damage. Worker-like
  range, cooldown, and target filters are the default unless implementation finds a specific reason
  to differ.
- Mining: can mine Steel or Oil near Zamok at 4x worker mining rate. Exact drop-off/cadence details
  follow the existing worker/Zamok proximity model where possible.
- Transformation: transforming permanently consumes the Golem. The Golem disappears immediately,
  the target building immediately exists, and that building starts at low HP. Exact starting HP and
  completion timing are deferred to the per-building phases or implementation planning.
- Consumption healing: Ekat can consume a nearby owned Golem to heal to full HP. Exact proximity
  range and command flow are deferred.
- Cap: no hard cap beyond normal Supply unless a later phase adds one.
- AI/prediction: AI support and local prediction may remain disabled indefinitely for Ekat.

## Entity Rules Items: Zamok/Home Structure

- [x] Creation source is specified.
- [x] Command and hotkey are specified if buildable or interactable.
- [x] Cost, refund, and cancellation are specified if buildable.
- [x] Build or setup time is specified if buildable.
- [x] Prerequisites, uniqueness, and unlock timing are specified.
- [x] Hit points are specified.
- [x] Armor, tags, repairability, capture rules, and vulnerabilities are specified.
- [x] Footprint, placement grid, terrain restrictions, collision, and pathing interactions are
      specified.
- [x] Selection size, render size, and minimap behavior are specified.
- [x] Sight range, fog reveal, and remembered-building behavior are specified.
- [x] Supply behavior is specified.
- [x] Mining proximity, Golem production, hero revival, or other economy/tech behavior is specified.
- [x] Death behavior and victory relevance are specified.
- [x] AI availability and intended AI usage are specified.

Approved Zamok rules:

- Creation source: each Ekat player starts with one free Zamok. Additional Zamoks are buildable and
  expensive; exact builder/source command is deferred.
- Command and hotkey: deferred until the additional-Zamok builder/source is finalized.
- Cost/refund/cancellation: expensive expansion Zamok direction approved; exact cost, refund, and
  cancellation rules are deferred.
- Build/setup time: buildable expansion direction approved; exact build time is deferred.
- Prerequisites/uniqueness: Zamok is not unique. Additional Zamoks need no tech prerequisite unless
  a later implementation pass introduces one.
- Stats and footprint: use City Centre-equivalent values by default, including current 600 HP, 3x3
  footprint, and 1-tile sight.
- Armor/tags/repairability/capture/vulnerability: use City Centre-equivalent defaults by default.
  The exact repair actor is deferred because Ekat does not use Kriegsia workers.
- Placement/pathing/render/minimap/fog: use City Centre-equivalent defaults by default.
- Supply: each Zamok provides +10 Supply.
- Economy/tech: Zamok anchors Ekat direct mining and Golem mining using City Centre-equivalent
  proximity semantics. Zamok produces/builds Golems.
- Weapon: no default weapon or defensive attack in the first target.
- Death/victory: if a player has no Zamoks, Ekat dies. If Ekat dies, that player loses.
- AI/prediction: AI support and local prediction may remain disabled indefinitely for Ekat.

## Entity Rules Items: Killing Tools

- [x] Creation source is specified.
- [x] Command and hotkey are specified.
- [x] Transform cost, consumed Golem behavior, refund, and cancellation are specified.
- [x] Transform time is specified.
- [x] Prerequisites, build limit, and unlock timing are specified.
- [x] Hit points are specified.
- [x] Armor, tags, repairability, capture rules, and vulnerabilities are specified.
- [x] Footprint, placement grid, terrain restrictions, collision, and pathing interactions are
      specified.
- [x] Selection size, render size, and minimap behavior are specified.
- [x] Sight range, fog reveal, and remembered-building behavior are specified.
- [x] Supply behavior is specified.
- [x] Line Shot unlock, upgrade costs, upgrade times, and loss-on-destruction behavior are
      specified.
- [x] Death behavior is specified.
- [x] AI availability and intended AI usage are specified.

Approved Killing Tools rules:

- Creation source: Golem transformation.
- Command and hotkey: deferred until command-card implementation.
- Transform cost/refund/cancellation: free Steel/Oil transform, permanently consumes the Golem, and
  has no refund after the immediate transform begins.
- Transform time: the Golem disappears immediately and the Killing Tools structure appears at low
  HP, following the approved Golem morph direction. Exact completion timing and starting HP profile
  are deferred.
- Prerequisites/build limit: requires an owned Golem; no additional tech prerequisite or hard build
  limit unless a later phase adds one.
- Max HP: 165, matching the current R&D Complex.
- Armor/tags: armored. Other tags, repairability, capture rules, and vulnerabilities use ordinary
  owner-only tech-building defaults unless a later implementation pass names a specific difference.
- Footprint: 3x3.
- Placement/pathing/render/minimap/fog: use ordinary tech-building defaults by default.
- Sight: 1 tile by default, matching current R&D Complex sight.
- Supply: no Supply provided or used.
- Weapon: no weapon or active combat behavior.
- Unlock behavior: at least one completed Killing Tools unlocks base Line Shot. If all completed
  Killing Tools structures are destroyed, Line Shot becomes locked/disabled again.
- Upgrade behavior: first implementation includes no upgrades. Future Killing Tools upgrades or
  attack customizations persist after research and are not lost or disabled by Killing Tools
  destruction.
- Death behavior: destroying the last completed Killing Tools removes Line Shot access but does not
  erase future researched upgrades/customizations.
- AI/prediction: AI support and local prediction may remain disabled indefinitely for Ekat.

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
