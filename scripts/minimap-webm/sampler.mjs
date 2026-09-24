// Hold entity state and retain attacks until the first sample at or after their tick.
export class ReplaySampler {
  constructor(durationTicks, stepTicks) {
    this.durationTicks = durationTicks;
    this.stepTicks = stepTicks;
    this.nextTick = 0;
    this.last = null;
    this.attacks = new Set();
  }

  push(snapshot, write) {
    const sample = {
      entities: snapshot.entities.map(e => [e.id, e.owner, e.kind, e.x, e.y, e.hp]),
    };
    const emit = state => {
      write({ ...state, tick: this.nextTick, attacks: [...this.attacks] });
      this.attacks.clear();
      this.nextTick += this.stepTicks;
    };
    while (this.nextTick < snapshot.tick && this.nextTick <= this.durationTicks) {
      emit(this.last || sample);
    }
    for (const event of snapshot.events || []) {
      if (event.e === 'attack') this.attacks.add(event.to);
    }
    if (this.nextTick === snapshot.tick && this.nextTick <= this.durationTicks) emit(sample);
    this.last = sample;
  }
}
