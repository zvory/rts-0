import {
  assertRemoteClientOwnedPosition,
  capture,
  issue,
  scenario,
  selectOwn,
  waitForSnapshot,
} from "../dsl.mjs";

export default scenario("remote_client_basic_move", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "disabled",
  },
  network: { mode: "direct" },
  steps: [
    selectOwn("worker", 0),
    issue("move", { dx: 160, dy: 0 }),
    waitForSnapshot({ minTickDelta: 360 }),
    capture("after-authoritative-move"),
    assertRemoteClientOwnedPosition({ unit: "worker", tolerancePx: 4 }),
  ],
});
