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

export default scenario("coalesced_snapshots_replay_pending", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
    quickstart: true,
  },
  network: {
    mode: "profile",
    name: "coalesce-latest-only",
    tickMs: 10,
    snapshotLatencyTicks: 4,
    coalesceSnapshots: true,
    coalesceWindowTicks: 8,
    seed: 4511,
  },
  steps: [
    waitForClientPredictionReady(),
    selectOwn("worker", 0),
    issueBurst([
      { command: "move", args: { dx: 64, dy: 0 } },
      { command: "attackMove", args: { dx: 128, dy: 32, queued: true } },
    ]),
    assertClientPrediction({ pendingClientSeqs: [1, 2] }),
    capture("pending-before-coalesced-ack"),
    waitForAck(2, { timeoutMs: 10000 }),
    capture("after-coalesced-ack"),
    assertClientPrediction({ pendingClientSeqs: [], latestAckSeq: 2, minSkippedSnapshotCount: 1 }),
    assertClientCorrectionBudget({ maxPx: 160, maxSnapCorrections: 2 }),
  ],
});
