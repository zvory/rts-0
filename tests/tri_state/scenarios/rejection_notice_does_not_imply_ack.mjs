import {
  assertClientPrediction,
  capture,
  expireClientCommands,
  injectClientSnapshot,
  issue,
  recordCommandRejection,
  scenario,
  selectOwn,
  setClientSnapshotDelivery,
} from "../dsl.mjs";

export default scenario("rejection_notice_does_not_imply_ack", {
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
    issue("invalidMove", { dx: 64, dy: 0 }),
    recordCommandRejection(1, "test invalid unit selection"),
    capture("rejection-diagnostic-without-ack"),
    assertClientPrediction({
      pendingClientSeqs: [1],
      latestAckSeq: 0,
      rejectionCount: 1,
      acknowledgedCount: 0,
    }),
    expireClientCommands({ elapsedMs: 20000 }),
    assertClientPrediction({
      pendingClientSeqs: [1],
      timedOutCount: 1,
    }),
    injectClientSnapshot("skipped", { ackSeq: 1, tickDelta: 3 }),
    capture("sim-ack-clears-rejected-command"),
    assertClientPrediction({
      pendingClientSeqs: [],
      latestAckSeq: 1,
      acknowledgedCount: 1,
    }),
  ],
});
