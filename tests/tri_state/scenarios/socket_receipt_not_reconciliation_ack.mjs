import {
  assertClientPrediction,
  capture,
  issue,
  recordSocketReceipt,
  scenario,
  selectOwn,
  setClientSnapshotDelivery,
} from "../dsl.mjs";

export default scenario("socket_receipt_not_reconciliation_ack", {
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
    issue("move", { dx: 64, dy: 0 }),
    recordSocketReceipt(1, { serverTick: 0 }),
    capture("receipt-recorded-without-sim-ack"),
    assertClientPrediction({
      pendingClientSeqs: [1],
      latestAckSeq: 0,
      receiptCount: 1,
      acknowledgedCount: 0,
    }),
  ],
});
