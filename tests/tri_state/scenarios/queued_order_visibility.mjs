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
    quickstart: true,
  },
  network: { mode: "direct" },
  steps: [
    selectOwn("worker", 0),
    issue("move", { dx: 96, dy: 0 }),
    issue("attackMove", { dx: 192, dy: 64, queued: true }),
    waitForSnapshot({ minTickDelta: 3 }),
    capture("after-queued-orders"),
    assertOrderPlansMatch({ unit: "worker" }),
  ],
});
