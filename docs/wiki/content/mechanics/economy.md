---
title: Economy
constants: STARTING_, STEEL_, OIL_, HARVEST_, EXTRACTOR_, REPAIR_, START_RESOURCE_, PLAYER_SUPPLY_, MINING_ANCHOR_
order: 20
---
Steel and oil come from extractors on resource patches. Kriegsia and Cultivators get all income this way; Ekat gathers with Golems.

## Resources

| | Steel | Oil |
|---|---|---|
| Extractor | {{building:steel_mine}} | {{building:pump_jack}} |
| Patch size | {{STEEL_PATCH_AMOUNT}} | {{OIL_GEYSER_AMOUNT}} |
| Patches per base | {{STEEL_PATCHES_PER_BASE}} | {{OIL_PATCHES_PER_BASE}} |
| Yield | {{STEEL_LOAD}} per {{HARVEST_TICKS|s}} | {{OIL_LOAD}} per {{HARVEST_TICKS|s}} |
| Per extractor per minute | {{calc: STEEL_LOAD / HARVEST_TICKS * TICK_HZ * 60}} | {{calc: OIL_LOAD / HARVEST_TICKS * TICK_HZ * 60}} |
| Patch runs out after | {{calc: STEEL_PATCH_AMOUNT / (STEEL_LOAD / HARVEST_TICKS * TICK_HZ) / 60|2}} min | {{calc: OIL_GEYSER_AMOUNT / (OIL_LOAD / HARVEST_TICKS * TICK_HZ) / 60|2}} min |

## Extractors

- A Resource Depot builds Steel Mines on nearby steel for free.
- It also builds a Pump Jack on nearby oil for free, taking {{AUTOMATIC_PUMP_JACK_BUILD_TICKS|s}}. An Engineer can build one for {{building:pump_jack.steel}} steel in {{building:pump_jack.buildTicks|s}}.
- Extractors only work within {{MINING_ANCHOR_RANGE_TILES}} tiles of a completed Resource Depot or Zamok.
- An extractor without a depot in range loses all its HP over {{EXTRACTOR_DECAY_TICKS|s}}.

## Income

| | Steel / min | Oil / min |
|---|---|---|
| Start ({{STARTING_STEEL_MINES}} Steel Mines, {{STARTING_PUMP_JACKS}} Pump Jack) | {{calc: STARTING_STEEL_MINES * STEEL_LOAD / HARVEST_TICKS * TICK_HZ * 60}} | {{calc: STARTING_PUMP_JACKS * OIL_LOAD / HARVEST_TICKS * TICK_HZ * 60}} |
| Full base ({{STEEL_PATCHES_PER_BASE}} + {{OIL_PATCHES_PER_BASE}} extractors) | {{calc: STEEL_PATCHES_PER_BASE * STEEL_LOAD / HARVEST_TICKS * TICK_HZ * 60}} | {{calc: OIL_PATCHES_PER_BASE * OIL_LOAD / HARVEST_TICKS * TICK_HZ * 60}} |

A new {{building:resource_depot}} costs {{building:resource_depot.steel}} steel and {{building:resource_depot.oil}} oil.

## Starting resources

| Faction | Steel | Oil |
|---|---|---|
| {{faction:kriegsia}} | {{faction:kriegsia.loadout.steel}} | {{faction:kriegsia.loadout.oil}} |
| {{faction:cultivators}} | {{faction:cultivators.loadout.steel}} | {{faction:cultivators.loadout.oil}} |
| {{faction:ekat}} | {{faction:ekat.loadout.steel}} | {{faction:ekat.loadout.oil}} |

## Supply

- Supply cap: {{PLAYER_SUPPLY_CAP}}.
- Each unit's supply cost is on the [unit table](table-units.html).

## Repair

- Each Engineer repairs {{REPAIR_HP_PER_SECOND}} HP per second.
- A full repair costs 1/{{REPAIR_COST_DIVISOR}} of the original steel and oil price.
