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

export default scenario("stop_corrects_predicted_motion", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
  },
  network: {
    mode: "profile",
    name: "stop-under-delayed-snapshots",
    tickMs: 10,
    snapshotLatencyTicks: 12,
    snapshotJitterTicks: 3,
    seed: 4514,
  },
  steps: [
    waitForClientPredictionReady(),
    selectOwn("worker", 0),
    issue("move", { dx: 256, dy: 0 }),
    capture("after-predicted-move"),
    issue("stop"),
    waitForAck(2, { timeoutMs: 12000 }),
    waitForSnapshot({ minTickDelta: 4, timeoutMs: 12000 }),
    capture("after-stop-ack"),
    assertClientPrediction({ pendingClientSeqs: [], latestAckSeq: 2, minAcknowledgedCount: 2 }),
    assertClientRenderedConverged({ unit: "worker", tolerancePx: 10 }),
    assertClientCorrectionBudget({ maxPx: 192, maxSnapCorrections: 2 }),
  ],
});
