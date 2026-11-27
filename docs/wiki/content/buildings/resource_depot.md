## Extractors

- Builds {{building:steel_mine|Steel Mines}} on nearby steel for free.
- Builds a {{building:pump_jack}} on nearby oil for free, taking {{AUTOMATIC_PUMP_JACK_BUILD_TICKS|s}}.
- Extractors only work within {{MINING_ANCHOR_RANGE_TILES}} tiles of a completed depot. If the depot is lost, they lose all HP over {{EXTRACTOR_DECAY_TICKS|s}}.
- A fully mined base makes {{calc: STEEL_PATCHES_PER_BASE * STEEL_LOAD / HARVEST_TICKS * TICK_HZ * 60}} steel and {{calc: OIL_PATCHES_PER_BASE * OIL_LOAD / HARVEST_TICKS * TICK_HZ * 60}} oil per minute. See {{page:mechanics/economy}}.
