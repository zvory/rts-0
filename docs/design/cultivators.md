# Cultivators: first iteration

## Approved brief and rules (Phases 0 and 1)

The user requested implementation by reusing the Resource Depot and Engineer, with the
Cultivators depot named **Nexus** and its worker named **Serf**. Serfs construct Nexus and Portal buildings.
This is catalog reuse, with no new entity kinds, combat, abilities, or numerical balance.
The Nexus now has approved lotus-roof sect headquarters art, with natural wood/stone and
player-tinted roofs; its shared simulation identity is unchanged.
The existing global `ResourceDepot`, `Worker`, `SteelMine`, and `PumpJack` identities remain.

Nexus inherits every Resource Depot stat, cost (450 Steel, 100 Oil), five-second construction,
footprint, fog, damage, cancellation, repair, placement, production, and mining rule. It trains
Serfs and automatically creates free Steel Mines and Oil Pumpjacks using the existing jobs.
Serfs inherit all Worker stats, cost, training time, movement, construction and presentation;
the faction build catalog permits only Nexus. Existing hotkeys and audio are reused.

The standard start contains one completed Nexus, one Serf, two completed Novices, six Steel
Mines, one Oil Pumpjack, 25 Steel and zero Oil, matching Kriegsia's economy without its starting Riflemen. Resource-patch
placement uses the existing loadout logic. The Disciple starts in the defensive line near the Nexus.
No research is available. Losing the Nexus interrupts its economy in exactly the same way as losing a Resource Depot. Expansion has
the same cost and vulnerability; this slice provides an economy to develop the faction further.

Normal human lobby selection and recorded replay lifecycle admit Cultivators. Existing AI
profiles retain their assigned factions; no new Cultivators AI is defined. Prediction continues
to use its existing supported-faction checks. Further art, audio, combat roster and buildings are deferred to future user-directed work.

## Portal production building

### Approved brief and rules (Phases 0 and 1)

The **Portal** is the Cultivators' first dedicated production building and its name is final. It
trains the faction's Novice from grid slot 1 (`Q`) and researched Adepts from the next
three slots: Wind (`W`), War (`E`), and Magic (`A`). Each Adept requires its matching
completed Pavilion Dao research and costs 150 Steel, 35 Oil, three supply, and 300 ticks
(ten seconds). No Novice input or Proving Grounds is needed. Normal manual/repeat queues,
payment, cancellation/refunds, blocked exits and rally behavior apply. Combat-earned
evolution and Tournament Grounds are unchanged. Its immediate player tradeoff is 150 Steel
invested in a vulnerable structure before it can produce a front-line military unit. Opponents
counter it through ordinary scouting and building destruction; losing it cancels access to new
Disciple production but has no additional special effect.

Cultivator Serfs build the Portal through the normal Build command. It costs 150 Steel and
zero Oil, occupies a 3x3 footprint, and takes five seconds to construct, matching the Cultivator
Nexus. The last timing detail interprets the request's repeated "Portal" reference as the existing
Nexus; it is intentionally isolated here for easy tuning. The Portal has the Barracks baseline of
165 HP, armored building status, one tile of sight, normal building collision and terrain
placement, ordinary construction cancellation/refund, repair, fog memory, minimap, damage, and
death behavior. It provides and consumes no supply, has no weapon, has no prerequisite beyond a
Cultivator Serf, and has no build limit. It supports ordinary Disciple production and rally
orders, but exposes no research, aura, storage, or other economy action. Cultivator AI support
remains deferred.

### Disciple brief and rules (Phases 0 and 1)

The **Novice** (internal entity id `warrior`, formerly Disciple) is a Cultivator melee unit trained from a completed
Portal. Its command-card description is: "Durable melee infantry with a short sword reach and 50% armor penetration. Slow
attacks deal heavy damage." It costs 50 Steel and zero Oil, consumes 2 Supply, and takes 300 ticks
(about 10 seconds) to train with no prerequisite. It has 58 HP, Small armor classification,
11-tile sight, a 26 px collision/selection/render radius, and ordinary ground movement at 2.0
px/tick. HP scales with the halved steel cost: 115 × 50/100 = 57.5, rounded to 58.
Adept and Master stats are unchanged.

Its sword has 0.75-tile reach beyond collision radii, deals 23 base damage, attacks every 91 ticks,
and applies 50% armor penetration. The 23 damage makes two successful hits lethal to a 45-HP
Rifleman. The 91-tick cooldown reduces its former 64-tick attack rate by 30%, rounded
to whole simulation ticks. Idle and Attack Move use a separate five-tile engagement range (with normal attacker
radius/range slack) to acquire and approach visible legal enemies. The sword still requires melee
reach to deal damage. Attack Move preserves its original destination and resumes it after the
target dies, becomes hidden/illegal, or leaves engagement range; Idle stops its approach. Hold
Position and Point stay in place, and plain Move keeps its route. Other unit kinds default
engagement range to their current attack range.
The sword uses general-purpose SmallArms target preference but has no projectile, tracer,
muzzle flash or area damage. Novices, all three Adept kinds, and their Masters use rooted footing while they have a combat
target and no movement path: resistance 60 instead of ordinary infantry combat resistance 3.
Moving or untargeted Disciples retain soft footing (1). Rooted combatants are exempt from the
extra traffic centerline lateral nudge; ordinary overlap separation still applies, so equally
friendly rooted units split an overlap evenly when space permits. Opponents block movement and
never receive contact pushes, regardless of footing; friendly traffic may stay compressed against
an enemy. Wind’s ability is displayed as **Blink** (the internal `windDash` id and D hotkey
remain unchanged). Blink chooses the nearest unoccupied legal landing within cast range. The shared footing system also
reduces stationary Magic Anchor pull with the higher resistance.
It holds position while attacking and otherwise uses
the ordinary attack, attack-move, hold, rally, fog, blocker, damage, and death rules.

The first implementation uses clearly placeholder matte-white/off-white unarmored Chinese
swordsman art with a plain robe, minimal topknot, runtime team tint, and a sword-swipe attack pose.
It supports Lab/Interact spawning, but existing AI profiles
do not train it. Final art, bespoke audio, AI composition, and post-playtest tuning are deferred.

The completed building is a black-and-blue spinning ground portal: a dark framed 3x3 platform
surrounds a layered vortex with counter-rotating rings, a pulsing black core, blue energy arcs,
and numerous orbiting and inward-falling particles. Construction remains readable through the
standard translucent scaffold treatment; completion activates the full animation. Team color is
kept to small frame accents so the blue vortex remains visually stable across players. Damaged,
destroyed, and selection states reuse global building presentation. Bespoke Portal audio and
production/channeling states are deferred until units are specified.

## Three Dao Pavilion prototype

### Approved brief and rules (Phases 0 and 1)

The user authorized the **Three Dao Pavilion** and three research
options that unlock direct Adept production at Portals. This is a Cultivator-only technology building
constructed by Serfs, available in normal games and Lab. Its investment competes with Portal
production; opponents can scout and destroy it through ordinary building combat.

Reuse the Training Centre baseline: 100 Steel, 25 Oil, 560 ticks construction, 120 HP,
armored, one tile sight, no weapon, no supply, and no production or rally. The approved
circular pavilion uses a 3x3 placement and collision footprint.
Require a completed Nexus and Portal. Use the next Serf build-card slot (A), with ordinary
placement, collision, fog memory, repair, construction cancellation/refund, damage and death.
There is no build limit. Approved PNG art is a circular stone study dais with three scripture
scrolls and a three-lobed canopy. The canopy and finial receive team tint; timber, stone, and
parchment retain natural colors. The square atlas preserves the generated proportions.

Research Dao of Wind, Dao of Magic, and Dao of War from slots Q/W/E. Each
uses player-wide purchase-order pricing: first 50 Steel / 100 Oil / 20 seconds,
second 100 Steel / 200 Oil / 40 seconds, third 200 Steel / 400 Oil / 80 seconds.
See [balance](balance.md#three-dao-pavilion-prototype) for payment and refund rules.
Use existing player-wide permanent research and queue rules, including duplicate rejection,
cancellation/refund and loss of unfinished research on destruction. All three can be completed;
completed research survives building destruction. Each research unlocks the matching
150-Steel / 35-Oil Adept production at Portals. The former paid training hall is retired; see
[Proving Grounds](proven-grounds.md). Combat-earned evolution at
60 insight requires neither research nor a Pavilion. See [Disciple insight and Dao evolution](dao-insight.md) for stats,
abilities, insight credit, icons, and hotkeys. Existing checkpoint/replay and owner-only
upgrade projection carry the stable ids through the generic research system.
Automatic AI evolution remains deferred.

## Shop

The [Shop](shop.md) is an inert durable gourd building constructed by Serfs: 200 steel, 200 oil,
40 seconds, 1,000 HP, armored, 3×4 footprint. No prerequisites or services yet. Its approved art
has an open covered counter, 愛 on the side, white team-painted cork/rope/canopy, and fresh impact
rocks around the buried base.
