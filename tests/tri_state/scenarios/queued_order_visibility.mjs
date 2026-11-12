import {
  assertOrderPlansMatch,
  capture,
  issue,
  scenario,
  selectOwn,
  waitForSnapshot,
} from "../dsl.mjs";

export default scenario("queued_order_visibility", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "disabled",
  },
  network: { mode: "direct" },
  steps: [
    selectOwn("worker", 0),
    issue("move", { x: 3392, y: 3216 }),
    issue("attackMove", { x: 3488, y: 3280, queued: true }),
    // 360 ticks need 12 seconds at 30 Hz, before browser/CI scheduling overhead.
    waitForSnapshot({ minTickDelta: 360, timeoutMs: 25000 }),
    capture("after-queued-orders"),
    assertOrderPlansMatch({ unit: "worker" }),
  ],
});
