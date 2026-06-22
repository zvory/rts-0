import {
  assertClientCorrectionBudget,
  assertOrderPlansMatch,
  capture,
  issueBurst,
  scenario,
  selectOwn,
  waitForAck,
  waitForClientPredictionReady,
  waitForSnapshot,
} from "../dsl.mjs";

export default scenario("queued_move_order_stages_survive_replay", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
  },
  network: {
    mode: "profile",
    name: "jittered-queued-move",
    tickMs: 10,
    snapshotLatencyTicks: 8,
    snapshotJitterTicks: 4,
    headOfLineEvery: 5,
    headOfLineTicks: 10,
    seed: 4513,
  },
  steps: [
    waitForClientPredictionReady(),
    selectOwn("worker", 0),
    issueBurst([
      { command: "move", args: { x: 3392, y: 3216 } },
      { command: "move", args: { x: 3488, y: 3216, queued: true } },
      { command: "attackMove", args: { x: 3568, y: 3280, queued: true } },
    ]),
    waitForAck(3, { timeoutMs: 12000 }),
    waitForSnapshot({ minTickDelta: 4, timeoutMs: 12000 }),
    capture("after-queued-replay"),
    assertOrderPlansMatch({ unit: "worker" }),
    assertClientCorrectionBudget({ maxPx: 192, maxSnapCorrections: 2 }),
  ],
});
