# Warrior Unit Requirements

Status: Phase 0 and Phase 1 complete; implementation authorized by the originating user request.

## Phase 0: Unit brief

- **Name:** Warrior.
- **Role:** Tanky Cultivator melee bruiser trained from the Portal.
- **Player-facing description:** Durable melee infantry with a short sword reach and 50% armor
  penetration. Slow attacks deal heavy damage.
- **Strategic purpose:** The Warrior gives Cultivators a sturdy front-line unit that can absorb
  sustained fire and threaten both infantry and armored targets once it closes the distance. Its
  short reach and slow attack cadence let ranged units kite or focus it before contact.
- **Counters:** Riflemen and other ranged units should exploit distance and focus fire; concentrated
  damage remains effective despite the Warrior's high HP. The Warrior has no ranged attack,
  projectile, tracer, or area damage.
- **Unusual interactions:** The unit uses ordinary ground movement, collision, fog, selection,
  rally, production, attack, attack-move, hold, and death rules. A sword strike is direct damage
  after the normal attack activation gate, has 50% armor penetration, does not overpenetrate, and
  emits only melee-appropriate swipe feedback. It is not Rifleman infantry and receives no
  Rifleman-only research or entrenchment behavior.
- **Availability:** Normal Cultivator unit trained at a completed Portal with no prerequisite.
  Lab/Interact spawning is supported. Existing AI profiles do not train it in the first pass.
- **Visual direction:** A clearly placeholder, matte-white/off-white unarmored Chinese swordsman sprite that
  can receive runtime team tint, rendered 50% larger in radius than a Rifleman. The attack pose
  reads as a sword swipe; projectile, muzzle-flash, and tracer presentation are prohibited.
- **Patch-note draft:** Cultivator Portals can now train Warriors: 100 Steel, 2 Supply, 135 HP, and a
  heavy 23-damage sword strike with 0.5-tile reach and 50% armor penetration.

Known unknowns: final art, audio, AI composition, and post-playtest balance tuning are deferred.

## Phase 1: Rules and balance specification

| Field | Specification |
| --- | --- |
| Cost | 100 Steel / 0 Oil |
| Supply | 2 |
| Build source | Cultivator Portal |
| Build hotkey | Portal grid slot 1 (`Q`) |
| Build time | 300 ticks (~10 seconds), matching Rifleman |
| Prerequisite | None beyond owning a completed Portal |
| HP / armor | 135 HP (3x Rifleman's 45); Small armor classification, with an unarmored visual |
| Sight | 11 tiles, matching Rifleman |
| Collision / selection / render radius | 13.5 px (1.5x Rifleman's 9 px) |
| Movement | 1.6 px/tick ordinary ground movement and pathing, matching Rifleman |
| Movement while attacking | Holds position while striking, matching ordinary non-moving-fire infantry |
| Sword | 23 base damage, enough to kill a 45-HP Rifleman in exactly two successful hits |
| Reach | 0.5 tiles beyond normal attacker/target collision radii |
| Attack cadence | 32-tick cooldown, interpreted as half the Rifleman's attacks per second |
| Armor penetration | 50%; general-purpose SmallArms target preference, with partial damage through armored protection |
| Windup / resolution | Existing immediate direct-fire activation timing; no separate travel time or projectile |
| Overpenetration / area | None |
| Feedback | Sword-swipe animation only; no tracer, projectile, muzzle flash, or ranged impact line |
| Target filters | Ordinary hostile ground entities under existing visibility, fog, blocker, and explicit-attack rules |
| Abilities / economy | No abilities, gathering, building, repair, or special economy interaction |
| AI | Legal to spawn, produce, rally, and command; not added to current AI build plans in this pass |

The existing unit kind encoding, train command, entity snapshot shape, and generic fire event are
sufficient. Add one new `warrior` entity-kind tag and one `warrior_sword` weapon-kind tag to the
existing mirrored protocol vocabularies; no new command, snapshot field, or event shape is needed.

## Deferred checklist

- Final non-placeholder Warrior art and bespoke audio.
- Cultivator AI production/composition and matchup tuning.
- Post-playtest cost, durability, reach, cadence, and armor-penetration tuning.
- Archer brief and implementation; it is explicitly outside this unit pass.
