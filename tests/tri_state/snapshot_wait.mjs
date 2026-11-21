import { TICK_HZ } from "../../client/src/config.js";

// Live scenarios can run at normal speed, even when other test servers are accelerated.
// Allow twice the requested simulation time plus scheduling slack, with the existing
// ten-second floor for short waits. Explicit deadlines remain caller-controlled.
export function snapshotTimeoutMs(minTickDelta = 1) {
  return Math.max(10000, Math.ceil(Math.max(0, minTickDelta) * 1000 / TICK_HZ) * 2 + 1000);
}
