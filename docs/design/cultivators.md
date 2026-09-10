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

The **Portal** is the Cultivators' first dedicated production building and its name is final. Its
command-card description is: "A dormant production gateway. Unit production is not yet
available." The Portal establishes the faction's supernatural production language now while
leaving its roster for a later unit brief. Its immediate player tradeoff is 150 Steel invested in
a vulnerable structure with no current economic or military output. Opponents counter it through
ordinary scouting and building destruction; losing it has no additional effect in this slice.

Cultivator Engineers build the Portal through the normal Build command. It costs 150 Steel and
zero Oil, occupies a 3x3 footprint, and takes five seconds to construct, matching the Cultivator
Nexus. The last timing detail interprets the request's repeated "Portal" reference as the existing
Nexus; it is intentionally isolated here for easy tuning. The Portal has the Barracks baseline of
165 HP, armored building status, one tile of sight, normal building collision and terrain
placement, ordinary construction cancellation/refund, repair, fog memory, minimap, damage, and
death behavior. It provides and consumes no supply, has no weapon, has no prerequisite beyond a
Cultivator Engineer, and has no build limit. It exposes no train, research, rally, aura, storage,
or other production/economy action yet. Cultivator AI support remains deferred.

The completed building is a black-and-blue spinning ground portal: a dark framed 3x3 platform
surrounds a layered vortex with counter-rotating rings, a pulsing black core, blue energy arcs,
and numerous orbiting and inward-falling particles. Construction remains readable through the
standard translucent scaffold treatment; completion activates the full animation. Team color is
kept to small frame accents so the blue vortex remains visually stable across players. Damaged,
destroyed, and selection states reuse global building presentation. Bespoke Portal audio and
production/channeling states are deferred until units are specified.
