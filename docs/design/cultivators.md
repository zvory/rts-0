# Cultivators: first iteration

## Approved brief and rules (Phases 0 and 1)

The user requested implementation by reusing the Resource Depot and Engineer, with the
Cultivators depot named **Nexus** and the Engineer restricted to constructing Nexus buildings.
This is catalog reuse, with no new entity kinds, art, combat, abilities, or numerical balance.
The existing global `ResourceDepot`, `Worker`, `SteelMine`, and `PumpJack` identities remain.

Nexus inherits every Resource Depot stat, cost (450 Steel, 100 Oil), five-second construction,
footprint, fog, damage, cancellation, repair, placement, production, and mining rule. It trains
Engineers and automatically creates free Steel Mines and Oil Pumpjacks using the existing jobs.
Engineers inherit all Worker stats, cost, training time, movement, construction and presentation;
the faction build catalog permits only Nexus. Existing hotkeys and audio are reused.

The standard start contains one completed Nexus, one Engineer, six Steel Mines, one Oil Pumpjack,
75 Steel and zero Oil, matching Kriegsia's economy without its starting Riflemen. Resource-patch
placement uses the existing loadout logic. No research or combat units are available. Losing the
Nexus interrupts its economy in exactly the same way as losing a Resource Depot. Expansion has
the same cost and vulnerability; this slice provides an economy to develop the faction further.

Normal human lobby selection and recorded replay lifecycle admit Cultivators. Existing AI
profiles retain their assigned factions; no new Cultivators AI is defined. Prediction continues
to use its existing supported-faction checks. Unique art, audio, combat roster and further
buildings are deferred to future user-directed work.

## Portal production building

### Approved brief and rules (Phases 0 and 1)

The **Portal** is the Cultivators' first dedicated production building and its name is final. It
trains the faction's Warrior from grid slot 1 (`Q`). Its immediate player tradeoff is 150 Steel
invested in a vulnerable structure before it can produce a front-line military unit. Opponents
counter it through ordinary scouting and building destruction; losing it cancels access to new
Warrior production but has no additional special effect.

Cultivator Engineers build the Portal through the normal Build command. It costs 150 Steel and
zero Oil, occupies a 3x3 footprint, and takes five seconds to construct, matching the Cultivator
Nexus. The last timing detail interprets the request's repeated "Portal" reference as the existing
Nexus; it is intentionally isolated here for easy tuning. The Portal has the Barracks baseline of
165 HP, armored building status, one tile of sight, normal building collision and terrain
placement, ordinary construction cancellation/refund, repair, fog memory, minimap, damage, and
death behavior. It provides and consumes no supply, has no weapon, has no prerequisite beyond a
Cultivator Engineer, and has no build limit. It supports ordinary Warrior production and rally
orders, but exposes no research, aura, storage, or other economy action. Cultivator AI support
remains deferred.

### Warrior brief and rules (Phases 0 and 1)

The **Warrior** is a tanky Cultivator melee bruiser trained from a completed Portal. Its command-card
description is: "Durable melee infantry with a short sword reach and 50% armor penetration. Slow
attacks deal heavy damage." It costs 100 Steel and zero Oil, consumes 2 Supply, and takes 300 ticks
(about 10 seconds) to train with no prerequisite. It has 135 HP, Small armor classification,
11-tile sight, a 13.5 px collision/selection/render radius, and ordinary ground movement at 1.6
px/tick. These values give it three times Rifleman HP, the same speed, and 1.5 times the radius.

Its sword has 0.5-tile reach beyond collision radii, deals 23 base damage, attacks every 32 ticks,
and applies 50% armor penetration. The 23 damage makes two successful hits lethal to a 45-HP
Rifleman; the 32-tick cooldown interprets "half Rifleman attack speed" as half as many attacks per
second. The sword uses general-purpose SmallArms target preference but has no projectile, tracer,
muzzle flash, area damage, or overpenetration. It holds position while attacking and otherwise uses
the ordinary attack, attack-move, hold, rally, fog, blocker, damage, and death rules.

The first implementation uses clearly placeholder matte-white/off-white swordsman art with runtime
team tint and a sword-swipe attack pose. It supports Lab/Interact spawning, but existing AI profiles
do not train it. Final art, bespoke audio, AI composition, and post-playtest tuning are deferred.

The completed building is a black-and-blue spinning ground portal: a dark framed 3x3 platform
surrounds a layered vortex with counter-rotating rings, a pulsing black core, blue energy arcs,
and numerous orbiting and inward-falling particles. Construction remains readable through the
standard translucent scaffold treatment; completion activates the full animation. Team color is
kept to small frame accents so the blue vortex remains visually stable across players. Damaged,
destroyed, and selection states reuse global building presentation. Bespoke Portal audio and
production/channeling states are deferred until units are specified.
