import {
  assertClientPrediction,
  capture,
  injectClientSnapshot,
  issueBurst,
  scenario,
  selectOwn,
  setClientSnapshotDelivery,
} from "../dsl.mjs";

export default scenario("ack_drops_consumed_pending_commands", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
    quickstart: true,
  },
  network: { mode: "direct" },
  steps: [
    selectOwn("worker", 0),
    setClientSnapshotDelivery(false),
    issueBurst([
      { command: "move", args: { dx: 32, dy: 0 } },
      { command: "move", args: { dx: 64, dy: 0, queued: true } },
      { command: "attackMove", args: { dx: 96, dy: 32, queued: true } },
    ]),
    capture("three-pending-before-ack"),
    assertClientPrediction({ pendingClientSeqs: [1, 2, 3], latestAckSeq: 0 }),
    injectClientSnapshot("skipped", { ackSeq: 1, tickDelta: 3 }),
    capture("ack-one-drops-only-one"),
    assertClientPrediction({
      pendingClientSeqs: [2, 3],
      latestAckSeq: 1,
      acknowledgedCount: 1,
    }),
  ],
});
