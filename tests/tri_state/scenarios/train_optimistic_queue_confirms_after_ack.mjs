import {
  assertClientOptimisticUi,
  assertClientPrediction,
  capture,
  issue,
  scenario,
  selectOwn,
  setClientSnapshotDelivery,
  waitForAck,
} from "../dsl.mjs";

export default scenario("train_optimistic_queue_confirms_after_ack", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
    quickstart: true,
  },
  network: { mode: "direct" },
  steps: [
    selectOwn("city_centre", 0),
    setClientSnapshotDelivery(false),
    issue("train", { unit: "worker" }),
    capture("optimistic-train-issued"),
    assertClientOptimisticUi({ productionCount: 1, productionQueue: 1 }),
    setClientSnapshotDelivery(true),
    waitForAck(1),
    capture("train-authoritative-confirmed"),
    assertClientOptimisticUi({ productionCount: 0 }),
    assertClientPrediction({ latestAckSeq: 1, pendingCommandCount: 0, uiConfirmedCount: 1 }),
  ],
});
