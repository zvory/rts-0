import {
  assertClientCorrectionBudget,
  assertClientPrediction,
  assertClientRenderedConverged,
  capture,
  issue,
  scenario,
  selectOwn,
  waitForAck,
  waitForClientPredictionReady,
  waitForSnapshot,
} from "../dsl.mjs";

export default scenario("move_converges_after_ack_20_ticks", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
  },
  network: {
    mode: "profile",
    name: "snapshot-delay-20",
    tickMs: 10,
    snapshotLatencyTicks: 20,
    seed: 4520,
  },
  steps: [
    waitForClientPredictionReady(),
    selectOwn("worker", 0),
    issue("move", { dx: 224, dy: 0 }),
    waitForAck(1, { timeoutMs: 10000 }),
    waitForSnapshot({ minTickDelta: 3, timeoutMs: 10000 }),
    capture("after-ack"),
    assertClientPrediction({ pendingClientSeqs: [], latestAckSeq: 1, minAcknowledgedCount: 1 }),
    assertClientRenderedConverged({ unit: "worker", tolerancePx: 10 }),
    assertClientCorrectionBudget({ maxPx: 160, maxSnapCorrections: 2 }),
  ],
});
