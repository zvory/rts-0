# Entrenchment Requirements

Status: Draft product requirements. This document describes the desired player-facing behavior,
not implementation details, code ownership, or approved phase scope.

## Purpose

Entrenchment lets infantry turn stationary positions into durable fighting positions. It should
reward holding ground, make prepared infantry harder to clear with direct fire or area fire, and
leave persistent trench ground that either side can later use.

## Upgrade

- Add a Training Centre research upgrade named `Entrenchment`.
- Entrenchment costs 100 steel and 0 oil.
- Entrenchment takes 10 seconds to research.
- Once researched, that player's eligible infantry can create new trenches by staying stationary.
- Existing trenches are neutral battlefield terrain. Eligible infantry from any player or allied
  team can use an existing trench even if that player has not researched Entrenchment.

## Eligible Units

- Riflemen are eligible.
- Machine Gunners are eligible.
- Workers/Engineers are eligible.
- Mortar Teams are not eligible.
- Ekat, Golems, and the Ekat faction are ignored for this feature.
- Vehicles, buildings, support weapons other than Machine Gunners, and non-infantry entities are
  not eligible.

## Creating Trenches

- An eligible infantry unit owned by a player with Entrenchment research creates and occupies a new
  trench after staying stationary on untrenched ground for 3 seconds.
- The unit must not receive entrenchment benefits during the 3-second dig-in period.
- Ordinary firing, weapon facing, body facing, and target changes do not cancel the dig-in period.
- Actual commanded movement cancels or prevents creating a new trench.
- A created trench remains permanently after the unit moves away.

## Using Existing Trenches

- Eligible infantry that stops on an existing trench receives entrenchment benefits without waiting
  for the 3-second dig-in period.
- Eligible infantry stopped slightly offset from an existing trench should slot into the trench.
- Slotting movement is a small positional correction toward the trench and does not count as
  movement for entrenchment purposes.
- A unit being slotted into a trench can still shoot.
- A unit moving normally through or past a trench does not receive entrenchment benefits until it
  stops and occupies the trench.

## Entrenchment Benefits

An eligible infantry unit receives these benefits only while it is stationary in an active trench:

- Its weapon range is increased by 1 tile.
- Direct shots against it have a 70% chance to miss.
- Area-of-effect damage against it is reduced by 70%.
- Projectiles do not over-penetrate through it.
- It does not take secondary over-penetration damage.

Methamphetamines and Entrenchment can both affect the same Rifleman. A Rifleman keeps the
Methamphetamines fire-rate benefit while entrenched, but Entrenchment benefits apply only while the
unit is stationary in a trench.

## Area Damage

- The area-damage reduction is intended for area-of-effect damage as a general category, not only
  current Mortar or Artillery damage.
- Future area-of-effect weapons should naturally interact with the Entrenchment reduction unless
  their product requirements explicitly say otherwise.
- Direct single-target weapon damage is not area damage.

## Trench Visual Requirements

- A newly created trench must show brown dug-in ground around the entrenching unit's position.
- Nearby trenches should visually connect into a continuous brown trench area where practical.
- Trench ground remains visible after the unit leaves.
- The occupied-unit visual indicator is not decided yet. Do not implement the final occupied-unit
  visual treatment until the user approves the direction.

## Non-Goals

- Do not define implementation phases in this document.
- Do not include code, protocol, data-model, test, or rendering implementation details here.
- Do not include Mortar Teams, Ekat, Golems, or Ekat-faction mechanics in the feature scope.
- Do not finalize the occupied-unit visual treatment in this requirements draft.
