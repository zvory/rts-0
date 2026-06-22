import {
  assertClientPrediction,
  capture,
  injectClientSnapshot,
  issueBurst,
  scenario,
  selectOwn,
  setClientSnapshotDelivery,
} from "../dsl.mjs";

export default scenario("stale_snapshot_ignored", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
  },
  network: { mode: "direct" },
  steps: [
    selectOwn("worker", 0),
    setClientSnapshotDelivery(false),
    issueBurst([
      { command: "move", args: { dx: 32, dy: 0 } },
      { command: "attackMove", args: { dx: 64, dy: 32, queued: true } },
    ]),
    injectClientSnapshot("skipped", { ackSeq: 1, tickDelta: 4 }),
    injectClientSnapshot("stale", { ackSeq: 2, tickBack: 1 }),
    capture("stale-ack-two-ignored"),
    assertClientPrediction({
      pendingClientSeqs: [2],
      latestAckSeq: 1,
      minStaleSnapshotCount: 1,
    }),
  ],
});
