// Sample complete semantic snapshots, retaining transient events/resource updates between frames.
// Drawing and presentation policy belong to the regular client, not this export adapter.
export class ReplaySampler {
  constructor(durationTicks, stepTicks) {
    this.durationTicks = durationTicks;
    this.stepTicks = stepTicks;
    this.nextTick = 0;
    this.last = null;
    this.events = [];
    this.resourceDeltas = [];
  }

  push(snapshot, write) {
    const { visibleTiles, exploredTiles, events, resourceDeltas, ...rest } = snapshot;
    const sample = { visibleTiles, exploredTiles, snapshot: rest };
    const emit = state => {
      write({
        tick: this.nextTick,
        ...(state.visibleTiles ? {
          visibleBits: packTiles(state.visibleTiles),
          exploredBits: packTiles(state.exploredTiles),
        } : {}),
        snapshot: { ...state.snapshot, tick: this.nextTick,
          events: this.events.map(entry => entry.event), resourceDeltas: this.resourceDeltas },
        timedEvents: this.events,
      });
      this.events = [];
      this.resourceDeltas = [];
      this.nextTick += this.stepTicks;
    };
    while (this.nextTick < snapshot.tick && this.nextTick <= this.durationTicks) emit(this.last || sample);
    for (const event of events || []) this.events.push({ tick: snapshot.tick, event });
    this.resourceDeltas.push(...(resourceDeltas || []));
    if (this.nextTick === snapshot.tick && this.nextTick <= this.durationTicks) emit(sample);
    this.last = sample;
  }
}

export function packTiles(tiles) {
  const bytes = Buffer.alloc(Math.ceil(tiles.length / 8));
  for (let i = 0; i < tiles.length; i++) if (tiles[i]) bytes[i >> 3] |= 1 << (i & 7);
  return bytes.toString('base64');
}
