import {
  assertClientPrediction,
  capture,
  injectClientSnapshot,
  issueBurst,
  scenario,
  selectOwn,
  setClientSnapshotDelivery,
} from "../dsl.mjs";

export default scenario("ack_three_leaves_four_five_pending", {
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
      { command: "move", args: { dx: 24, dy: 0 } },
      { command: "move", args: { dx: 48, dy: 0, queued: true } },
      { command: "attackMove", args: { dx: 72, dy: 32, queued: true } },
      { command: "move", args: { dx: 96, dy: 32, queued: true } },
      { command: "attackMove", args: { dx: 120, dy: 64, queued: true } },
    ]),
    capture("five-pending-before-ack"),
    injectClientSnapshot("skipped", { ackSeq: 3, tickDelta: 3 }),
    capture("ack-three-leaves-tail"),
    assertClientPrediction({
      pendingClientSeqs: [4, 5],
      latestAckSeq: 3,
      acknowledgedCount: 3,
    }),
  ],
});
