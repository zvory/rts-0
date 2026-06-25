# Ekat Requirements

Status: Draft product requirements. This document describes the desired player-facing faction
shape, not implementation details or approved phase scope.

## Core Direction

Ekat remains a hero-centric faction built around one primary hero, the Zamok, and Golems rather than
Kriegsia-style workers, barracks, factories, and broad unit production. The faction should feel like
a compact economy and tech engine that converts field presence into a small number of powerful
choices.

## Economy

- Ekat can mine Steel and Oil directly.
- The player mines by right-clicking Ekat onto a Steel or Oil patch near a Zamok.
- Ekat's direct mining income should match the income rate of four Kriegsia engineers.
- This direct mining is a baseline faction action, not an unlocked combat ability.

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

- Ekat has no combat abilities by default.
- Line Shot, Magic Anchor, and Dash must be unlocked through their associated Golem-converted
  buildings.
- Each ability family can have upgrades associated with its building.

## Health And Recovery

- Ekat has no natural health regeneration.
- Ekat can consume a Golem to return to full HP.
- Golem consumption is the faction's primary recovery rule unless later requirements add another
  healing path.

## Out Of Scope For This Requirements Draft

- Exact unit stats beyond the relative Golem requirements above.
- Exact mining timing, drop-off rules, and Zamok proximity numbers.
- Exact Golem build cost, build time, command UI, and transformation timing.
- Exact ability upgrade names, costs, effects, and ordering.
- Final name for the Dash building currently called **XYZ**.
- AI support, prediction support, replay compatibility, art, sound, and implementation phases.
