export class LocalLaneUnavailable {
  constructor({ artifacts } = {}) {
    this.artifacts = artifacts || null;
    this.frames = [];
  }

  async start() {
    return this.record("start");
  }

  async applyStep(step) {
    return this.record("step", { op: step?.op || null });
  }

  async capture(label) {
    return this.record("capture", { label });
  }

  async close() {}

  record(event, extra = {}) {
    const frame = {
      event,
      localLane: "unavailable",
      reason: "rts-sim-wasm tri-state adapter lands in Phase 3.5",
      ...extra,
    };
    this.frames.push(frame);
    this.artifacts?.local(frame);
    return frame;
  }
}
