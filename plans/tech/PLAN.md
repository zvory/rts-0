# Two-Path Tech Tree Plan

## Goal

Reshape the game into two asymmetric tech paths without requiring the in-game UI to use these
doctrine names:

- **Superior Firepower**: position grinding, attrition, long-range attacks, and layered defenses.
- **Mobile Warfare**: concentrated expensive strike forces that punch through weak points and then
  destroy economy from inside the enemy position.

Both paths should feel overpowered in the Brood War sense: each side gets extreme tools, and the
matchup is balanced by timing, scouting, investment, and counterplay rather than by making every
unit modest.

## Confirmed Design

- Training Centre is the shared base tech structure.
- Training Centre eventually unlocks Machine Gunners and Stormtroopers.
- Stormtroopers are not urgent for the first implementation pass.
- Superior Firepower path entry should allow play with Mortar Teams before AT Guns.
- Mobile Warfare path entry should allow play with Scout Cars before Tanks.
- AT Guns and Tanks both require additional investment beyond the path-entry building.
- Mobile Warfare should have a strong stage-two surge when Tanks unlock.
- Superior Firepower should be trying to survive that surge and reach Artillery.
- If Superior Firepower reaches Artillery, it should be able to grind down Mobile Warfare positions
  and destroy resource bases from range.

## Tech Shape

| Stage | Superior Firepower | Mobile Warfare |
|-------|--------------------|----------------|
| Shared | Training Centre: Machine Gunner, later Stormtrooper | Training Centre: Machine Gunner, later Stormtrooper |
| Path entry | Gun Works: Mortar Team | Vehicle Works: Scout Car |
| Commitment unlock | Gun Works upgrade: AT Gun | Vehicle Works upgrade: Tank |
| Late power | Gun Works upgrade: Artillery | Vehicle Works upgrade: Command Car |

## Unit Notes

### Stormtrooper

- Requires Training Centre.
- Move while shooting.
- Methamphetamines upgrade applies to them too.
- Uses a submachine gun: low range, high DPS, requires reloading.
- Offensive counterpart to Machine Gunners: worse in a stand-up fight, but mobile.
- Well-used Stormtroopers can pick apart an MG defense; without careful use and timing, they are
  strictly outclassed by Machine Gunners.

### Mortar Team

- Built at Gun Works.
- Path-entry Superior Firepower unit.
- Shells take 2 seconds to land and fire every 2 seconds.
- No marker indicates where rounds will hit.
- Small AOE damage.
- Semi armor piercing: only suffers half of normal armored damage reduction.
- Autocast fires at predicted enemy position 2 seconds in the future, with built-in error.
- Autocast Mortar Teams should stagger timing and choose different targets.
- Player can disable autocast and aim manually.
- Like Machine Gunners, must set up to shoot; setup happens automatically after 1 second.

### AT Gun

- Built at Gun Works.
- Requires a Gun Works upgrade before training.
- Superior Firepower commitment unlock and primary answer to Tanks.

### Scout Car

- Built at Vehicle Works.
- Path-entry Mobile Warfare unit.
- Already mostly implemented.

### Tank

- Built at Vehicle Works.
- Requires a Vehicle Works upgrade before training.
- Mobile Warfare stage-two power spike.

### Command Car

- Built at Vehicle Works.
- Late Mobile Warfare capstone.
- Ability: **Breakthrough!** AOE speed boost, doubled for units in smoke or that recently left smoke.
- Ability: **Fake Army** creates fake copies of army units in AOE and places them at a target
  location within range.
- Fake Army copies deal no damage, have 10% of the real units' HP, and disappear after 20 seconds.

### Artillery

- Built at Gun Works.
- Late Superior Firepower capstone.
- Very slow, very long-range gun that shoots at a point rather than at a unit.
- Must set up before firing and tear down before moving.
- First shot has low accuracy; accuracy improves over time.

## Phase Index

1. [Phase 1 - Tech Spine and Vehicle Works Framing](phase-1-tech-spine.md)
2. [Phase 2 - AT Gun and Tank Unlock Upgrades](phase-2-at-tank-unlocks.md)
3. [Phase 3 - Gun Works AT Gun Production](phase-3-steelworks-at-gun.md)
4. [Phase 4 - Mortar Team](phase-4-mortar-team.md)
5. [Phase 5 - Stage Timing and Playtest Balance](phase-5-stage-timing.md)
6. [Phase 6 - Capstone Units: Artillery and Command Car](phase-6-capstones.md)

## Deferred Work

- Stormtroopers should wait until the two-path spine, Mortar Team, AT Gun unlock, and Tank unlock
  are playable.
- Hard mutual exclusivity between paths is not part of the first pass. Prefer economic and timing
  pressure first: players may access both paths, but doing so should delay power spikes.

