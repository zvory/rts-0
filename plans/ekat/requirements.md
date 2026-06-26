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
- Use [docs/new-building-checklist.md](../../docs/new-building-checklist.md) for Zamok, Death Box,
  Vortex, and the Dash building.
- Work serially: global identity gate, then Ekat, then Zamok, then Golem, then Death Box, then
  Vortex, then the Dash building. Do not brief or spec multiple entities in parallel unless the
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

- Ekat can build Golems.
- A Golem is roughly equivalent to four Kriegsia engineers combined:
  - 4x supply use.
  - 4x HP.
  - 4x mining speed.
- Golems are the faction's main economic and tech-conversion piece, not a broad army roster.

## Tech Buildings

Ekat unlocks hero abilities by transforming Golems into buildings. The transformation is free, and
the building choice determines which ability family becomes available.

- **Death Box** unlocks Line Shot and Line Shot upgrades.
- **Vortex** unlocks Magic Anchor and Magic Anchor upgrades.
- **XYZ** is a placeholder building name for Dash and Dash upgrades.

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
- Exact Golem build cost, build time, command UI, and transformation timing.
- Exact ability upgrade names, costs, effects, and ordering.
- Final name for the Dash building currently called **XYZ**.
- Ekat HP scaling mechanics, cloning mechanics, revival/comeback mechanics, AI support, prediction
  support, replay compatibility, art, sound, and implementation phases.
