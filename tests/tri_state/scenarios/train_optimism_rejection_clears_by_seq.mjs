import {
  assertClientOptimisticUi,
  assertClientPrediction,
  capture,
  issue,
  recordCommandRejection,
  scenario,
  selectOwn,
  setClientSnapshotDelivery,
} from "../dsl.mjs";

export default scenario("train_optimism_rejection_clears_by_seq", {
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
    recordCommandRejection(1, "Not enough steel"),
    capture("optimistic-train-rejected"),
    assertClientOptimisticUi({ productionCount: 0 }),
    assertClientPrediction({ rejectionCount: 1, pendingClientSeqs: [1] }),
    setClientSnapshotDelivery(true),
  ],
});
