import { TICK_HZ } from "./config.js";

export const LIVE_APM_WINDOW_SECONDS = 10;

export class ApmTracker {
  constructor({ tickHz = TICK_HZ, windowSeconds = LIVE_APM_WINDOW_SECONDS } = {}) {
    this.tickHz = Math.max(1, Number(tickHz) || TICK_HZ);
    this.windowSeconds = Math.max(1, Number(windowSeconds) || LIVE_APM_WINDOW_SECONDS);
    this.actionTicks = [];
  }

  recordAction(tick) {
    const actionTick = Math.max(0, Math.floor(Number(tick) || 0));
    this.actionTicks.push(actionTick);
  }

  currentApm(tick) {
    const currentTick = Math.max(0, Math.floor(Number(tick) || 0));
    const windowTicks = this.tickHz * this.windowSeconds;
    const oldestTick = currentTick - windowTicks;
    this.actionTicks = this.actionTicks.filter((actionTick) =>
      actionTick > oldestTick && actionTick <= currentTick);
    return Math.round(this.actionTicks.length * 60 / this.windowSeconds);
  }

  reset() {
    this.actionTicks.length = 0;
  }
}
