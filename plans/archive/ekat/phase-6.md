# Phase 6 - Positioning Brief and Spec

Status: complete; user decisions recorded in [checklists.md](checklists.md) and
[requirements.md](requirements.md).

## Goal

Complete the new-building checklist Phase 0 brief and Phase 1 rules/balance spec for Positioning,
formerly the Dash building named `XYZ` in the draft. This phase should define the Projection tech
commitment and final building name before any implementation phase is written.

## Scope

- Read [docs/context/balance.md](../../../docs/context/balance.md) before editing rules/spec details.
- Use [docs/new-building-checklist.md](../../../docs/new-building-checklist.md) for this building.
- Complete only the Positioning sections in [checklists.md](checklists.md), or mark items as
  deferred with named unknowns.
- Specify Projection unlocks, upgrades, transformation tradeoff, destruction consequence, final
  name, and first playable exposure only for this building.
- Update [requirements.md](requirements.md) only when a Positioning decision becomes approved
  product direction.

## Out of Scope

- Reopening Killing Tools or Anchorage specs except for explicit user-requested comparison updates.
- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Implementing the specified rules.
- Future implementation phase files unless the user explicitly approves moving beyond the Phase 6
  gate.

## User Interview Focus

- What is the final name for the building currently called `XYZ`?
- Why does the player choose Positioning over Killing Tools or Anchorage?
- What should Projection and its upgrades do before numbers are chosen?
- Is Projection primarily escape, engage, repositioning, mining tempo, or something else?
- What should happen to Projection access if this building is destroyed or transformed away?

## Expected Deliverables

- [checklists.md](checklists.md) updated only for Positioning Phase 0 and Phase 1 items.
- [requirements.md](requirements.md) updated only for confirmed Positioning product rules.
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

The handoff must name the approved Positioning brief and rules, list explicit user-approved
numbers and rules across the serial plan, identify unresolved tuning questions, and state exactly
what a future implementation phase may build. If any major faction mechanic is not approved, the
handoff must say that implementation remains blocked for that mechanic.

## Handoff

Approved Positioning brief:

- Positioning replaces `XYZ` as the building name.
- Projection replaces Dash as the player-facing ability name for the current Dash runtime.
- Positioning is Ekat's mobility and repositioning technology structure.
- First playable scope: Positioning unlocks base Projection only.
- Long-term direction: Positioning hosts broader movement and mobility customizations.
- Positioning is deliberately flexible: escape, engage, repositioning, and outplay potential are all
  valid uses.
- There is no settled reason to choose Positioning before Killing Tools yet; that competitive role
  is deferred to playtesting.
- Positioning has no weapon or active combat behavior; it is a tech unlock structure.

Approved Positioning rules:

- Golem transforms into Positioning for free except for permanently consuming the Golem.
- At least one completed Positioning structure unlocks Projection.
- If all completed Positioning structures are destroyed, Projection becomes locked/disabled again.
- Future upgrades or movement/mobility customizations researched at Positioning persist after
  research and are not lost or disabled when Positioning structures are destroyed.
- First implementation includes no upgrades or broader movement customizations.
- Max HP: 165, matching Killing Tools, Anchorage, and the current R&D Complex.
- Footprint: 3x3.
- Sight: 1 tile by default, matching the other first-target Ekat tech buildings.
- Armor: armored.
- Supply: none provided or used.
- No weapon or active combat behavior.
- AI support and local prediction may remain disabled indefinitely for Ekat.

Approved serial-plan rules summary:

- Ekat is one word, short for Ekaterina, and is both the faction and hero/body name.
- Zamok is the Ekat home/core structure and City Centre-equivalent mining/Golem-production anchor.
- Ekat starts unique at match start, costs 0, uses 0 Supply, has 150 HP, 1.6 px/tick movement,
  9-tile sight, no basic attack, no natural regeneration, and Golem consumption as her current
  recovery path.
- Ekat death causes immediate defeat for the first implementation target.
- If a player has no Zamoks, Ekat dies.
- Golems are directly controllable worker-like economy/tech units built by Zamok: 4 Supply, 160 HP,
  4x worker mining, 16 worker-like attack damage, worker-like movement and sight defaults, and no
  hard cap beyond Supply.
- Ekat can consume a nearby owned Golem to heal to full HP; first playable range is 2 tiles and the
  command consumes the nearest owned living Golem in range.
- Golem transformations are free except for permanent Golem consumption and immediately create a
  low-HP tech building.
- Killing Tools unlocks Line Shot.
- Anchorage unlocks the current Magic Anchor implementation, with likely future ability rename to
  Vortex.
- Positioning unlocks Projection, the renamed current Dash implementation.
- First-target Ekat tech buildings share the same default profile: 3x3 footprint, 165 HP, 1-tile
  sight, armored, no Supply, no weapon, no active combat behavior, no first-pass upgrades.
- Destroying all completed structures for an ability family removes that ability access, while
  researched upgrades/customizations persist.

Unresolved tuning questions:

- Exact Golem cost, build time, hotkey, command-card details, healing range, and size/render tuning.
- Exact Zamok expansion cost, builder/source command, hotkey, build time, refund/cancellation, and
  repair actor.
- Exact tech-building transform commands, hotkeys, completion timing, and low-HP starting profiles.
- Exact Line Shot/Killing Tools attack customizations.
- Exact Anchorage anchor customizations and final ability name for the current Magic Anchor runtime.
- Exact Positioning movement/mobility customizations.
- Exact strategic reason to choose Anchorage or Positioning before Killing Tools.
- Future Ekat HP scaling, cloning, revival/comeback, AI, prediction, replay, art, and sound scope.

Future Implementation Permission:

- Implementation is not yet approved. This Phase 6 planning gate records the briefs and rules, but
  Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, and
  deployment changes remain blocked until the user explicitly approves a named implementation
  scope.

No implementation files were edited.
