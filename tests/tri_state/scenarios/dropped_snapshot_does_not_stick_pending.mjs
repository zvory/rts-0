import {
  assertClientCorrectionBudget,
  assertClientPrediction,
  capture,
  issueBurst,
  scenario,
  selectOwn,
  waitForAck,
  waitForClientPredictionReady,
} from "../dsl.mjs";

export default scenario("dropped_snapshot_does_not_stick_pending", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
  },
  network: {
    mode: "profile",
    name: "drop-every-second-snapshot",
    tickMs: 10,
    snapshotLatencyTicks: 3,
    snapshotDropEvery: 2,
    seed: 4512,
  },
  steps: [
    waitForClientPredictionReady(),
    selectOwn("worker", 0),
    issueBurst([
      { command: "move", args: { dx: 64, dy: 0 } },
      { command: "move", args: { dx: 128, dy: 0, queued: true } },
    ]),
    waitForAck(2, { timeoutMs: 10000 }),
    capture("after-later-ack"),
    assertClientPrediction({ pendingClientSeqs: [], latestAckSeq: 2, minAcknowledgedCount: 2 }),
    assertClientCorrectionBudget({ maxPx: 160, maxSnapCorrections: 2 }),
  ],
});
