# Ekat Requirements

Status: Draft product requirements. This document describes the desired player-facing faction
shape, not implementation details or approved phase scope.

Active planning gate: [plan.md](plan.md). The current work is serial Phase 0/1 planning only: each
unit or building gets its own user-reviewed brief and rules spec before the next entity starts. Do
not implement Rust, JavaScript, protocol, balance, art, tests, or scenario files from this
requirements draft until the user approves the serial briefs/specs and explicitly authorizes an
implementation phase.

## Planning Workflow

- Use [docs/new-unit-checklist.md](../../docs/new-unit-checklist.md) for Ekat and Golem.
- Use [docs/new-building-checklist.md](../../docs/new-building-checklist.md) for Zamok, Killing
  Tools, Anchorage, and the Dash building.
- Work serially: global identity gate, then Ekat, then Zamok, then Golem, then Killing Tools, then
  Anchorage, then the Dash building. Do not brief or spec multiple entities in parallel unless the
  user explicitly overrides the active plan.
- Treat the current Ekat body and ability runtime as already implemented. The next Ekat-body work
  is stat rework plus ability availability gating, not a from-scratch hero or ability design pass.
- Treat each building as its own user-reviewed design object. For each one, confirm the fantasy,
  strategic reason to choose it, counterplay, unlock behavior, loss/destruction consequences, and
  first playable scope before implementation.
- Reconcile this draft with the current playable Ekat hero/Zamok slice before changing behavior.
  In particular, this draft says Ekat has no combat abilities by default and no natural health
  regeneration, while the current implementation exposes Ekat abilities and regeneration.

## Core Direction

Ekat remains a hero-centric faction built around one primary hero, the Zamok, and Golems rather than
Kriegsia-style workers, barracks, factories, and broad unit production. The faction should feel like
a compact economy and tech engine that converts field presence into a small number of powerful
choices.

Ekat is the faction name and the hero/body name. The name is one word, spelled `Ekat`, and is short
for Ekaterina. Do not style it as `E-Kat`, `E-K-A-T`, `E-Cat`, or `E-C-A-T`.

The new direction replaces the current playable Ekat/Zamok slice when it is ready. The current body,
visuals, and ability runtime are reused, but the current always-available abilities and natural
health regeneration are not the target behavior.

## Visual Theme

- [Rodchenko](https://www.paratype.com/fonts/pt/rodchenko) is the theme/display font direction for
  Ekat's English-language faction identity.
- Ekat's Russian feel should come from Constructivist-inspired typography and layout, not
  fake-Cyrillic glyph substitutions or pseudo-Russian letter swaps.
- Use Rodchenko as the starting reference for faction names, titles, and other high-emphasis text;
  validate licensing and delivery details before any shippable implementation.

## Economy

- Ekat can mine Steel and Oil directly.
- Ekat can mine either Steel or Oil at a time.
- The player mines by right-clicking Ekat onto a Steel or Oil patch near a Zamok.
- Ekat's direct mining requires Zamok proximity.
- Golem mining also requires Zamok proximity.
- Zamok proximity for Ekat and Golem mining should use the same role and implementation shape as
  City Centre mining proximity.
- Ekat's direct mining income should match the income rate of four Kriegsia engineers.
- This direct mining is a baseline faction action, not an unlocked combat ability.

## Zamok Home Structure

- Zamok is Ekat's home base structure and core building.
- Zamok and City Centre fill the same faction role: use City Centre-equivalent structure semantics
  wherever possible, with Ekat-specific visuals and Golem production.
- Each Ekat player starts with one Zamok.
- Additional Zamoks can be built, but should be expensive. Exact cost, builder, command, hotkey, and
  build time are deferred.
- Zamok provides +10 Supply.
- Zamok builds Golems.
- Zamok anchors both Ekat direct mining and Golem mining.
- If a player has no Zamoks, Ekat dies. If Ekat dies, that player loses.
- Zamok has no default weapon or defensive attack in the first target.

## Golems

- The Ekat faction can build Golems through Zamok.
- Golems are directly controllable worker-like units with Ekat-specific economy, transformation,
  and healing mechanics.
- A Golem is roughly equivalent to four Kriegsia engineers combined:
  - 4x supply use.
  - 4x HP.
  - 4x mining speed.
- Golems can attack, using four times worker damage.
- Golems are the faction's main economic and tech-conversion piece, not a broad army roster.
- Golems permanently transform into tech buildings and any other approved Golem-transformed
  structures.
- Golem transformation should work like a Zerg-style building morph: the Golem disappears
  immediately, the building immediately comes into existence, and the building starts at low HP.
- Ekat can consume a Golem for full healing only when the Golem is near Ekat. Exact range is
  deferred.

## Tech Buildings

Ekat unlocks hero abilities by transforming Golems into buildings. The transformation is free, and
the building choice determines which ability family becomes available.

Shared first-target rules for Ekat tech buildings:

- A tech building transform is free except for permanently consuming the transforming Golem.
- Ability access requires at least one completed structure for that ability family.
- If all completed structures for an ability family are destroyed, that ability becomes
  locked/disabled again.
- Researched upgrades or customizations persist once researched and are not lost or disabled just
  because the associated tech building is gone.
- First-target tech buildings use the same stat profile unless a later phase explicitly changes it:
  3x3 footprint, 165 HP to match the current R&D Complex, 1-tile sight, armored, no weapon, and no
  active combat behavior.
- First implementation targets skip upgrades unless the active entity phase explicitly includes one.

- **Killing Tools** is the current name for the offensive attack tech building formerly drafted as
  Death Box. The name may still change, but it is no longer treated as a throwaway placeholder.
- **Killing Tools** unlocks base Line Shot in the first implementation target.
- In the longer-term direction, Killing Tools becomes the place to customize Ekat's offensive
  attacks. Future customizations may include Line Shot, fan-out behavior, return behavior, or other
  build-like attack variants.
- **Anchorage** is the current name for the anchor-placement tech building formerly drafted as
  Vortex.
- **Anchorage** unlocks the current Magic Anchor implementation in the first implementation target.
- In the longer-term direction, Anchorage becomes the place to create and customize anchors. The
  current Magic Anchor implementation should probably be renamed to Vortex later, and future anchors
  may do other things.
- **XYZ** is a placeholder building name for Dash and Dash upgrades.

### Killing Tools

- Killing Tools is Ekat's offensive damage-dealing technology structure.
- A Golem transforms into Killing Tools for free except for permanently consuming that Golem.
- At least one completed Killing Tools structure unlocks base Line Shot.
- If all completed Killing Tools structures are destroyed, base Line Shot becomes locked/disabled
  again.
- Future Killing Tools upgrades or customizations should persist once researched; they are not lost
  or disabled just because all Killing Tools structures are gone. Line Shot access itself still
  requires a completed Killing Tools structure.
- The first implementation target includes only the base Line Shot unlock; upgrades and broader
  attack customization are deferred.
- Killing Tools has no weapon or active combat behavior. It is a tech unlock structure.
- Killing Tools uses a 3x3 footprint, 165 HP to match the current R&D Complex, and armored status.
- Exact command, hotkey, transform completion timing, low-HP starting profile, upgrade list, upgrade
  costs, and upgrade times are deferred.

### Anchorage

- Anchorage is Ekat's anchor-placement technology structure.
- A Golem transforms into Anchorage for free except for permanently consuming that Golem.
- At least one completed Anchorage structure unlocks the current Magic Anchor implementation.
- If all completed Anchorage structures are destroyed, Magic Anchor becomes locked/disabled again.
- Future Anchorage upgrades or anchor customizations should persist once researched; they are not
  lost or disabled just because all Anchorage structures are gone. Magic Anchor access itself still
  requires a completed Anchorage structure.
- The first implementation target includes only the base Magic Anchor unlock; upgrades and broader
  anchor customization are deferred.
- Anchorage has no weapon or active combat behavior. It is a tech unlock structure.
- Anchorage uses the shared Ekat tech-building stat profile: 3x3 footprint, 165 HP, 1-tile sight,
  and armored status.
- Killing Tools is expected to be the first-priority tech choice for raw pressure. Anchorage's exact
  competitive reason versus Killing Tools and Dash tech is deferred for playtesting.
- Exact command, hotkey, transform completion timing, low-HP starting profile, anchor customization
  list, upgrade costs, and upgrade times are deferred.

## Abilities

- Ekat has no combat abilities by default and no basic attack.
- Locked abilities should remain visible in the command card but disabled.
- Line Shot, Magic Anchor, and Dash must be unlocked through their associated Golem-converted
  buildings.
- Each ability family can have upgrades associated with its building.

## Health And Recovery

- Ekat has no natural health regeneration.
- Ekat can consume a Golem to return to full HP.
- Golem consumption is the faction's primary recovery rule unless later requirements add another
  healing path.
- Until Golem consumption exists, damaged Ekat has no recovery.
- If Ekat dies, that player loses immediately for the first implementation target. Future revival,
  cloning, or comeback mechanics are possible later but are not part of the current target.

## Ekat Hero Body Direction

- Each player starts with one unique Ekat hero body.
- Future cloning abilities may create exceptions, but the baseline hero identity remains unique.
- Ekat has no build cost and is not normally produced.
- Ekat uses 0 Supply.
- Starting HP target: 150, with future scaling deferred to a later mechanism.
- Movement speed target: 1.6 px/tick, matching Rifleman speed.
- Sight target: 9 tiles.
- Body radius, selection feel, render presentation, armor/tags, and existing ability runtime should
  be reused unless a later implementation pass finds a specific mismatch.
- AI support and local prediction may remain disabled for Ekat indefinitely.

## Out Of Scope For This Requirements Draft

- Exact mining timing, drop-off rules, and Zamok proximity numbers.
- Exact Golem build cost, build time, command UI, healing range, and transformation building HP
  profile.
- Exact Killing Tools attack customizations, Anchorage anchor customizations, ability upgrade names,
  costs, effects, and ordering.
- Final name for the Dash building currently called **XYZ**.
- Ekat HP scaling mechanics, cloning mechanics, revival/comeback mechanics, AI support, prediction
  support, replay compatibility, art, sound, and implementation phases.
