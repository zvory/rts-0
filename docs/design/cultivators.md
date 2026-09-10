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
