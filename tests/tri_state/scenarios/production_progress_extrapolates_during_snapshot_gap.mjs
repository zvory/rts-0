import {
  assertClientRenderedProductionProgress,
  capture,
  issue,
  scenario,
  selectOwn,
  setClientSnapshotDelivery,
  waitForSnapshot,
  waitMs,
} from "../dsl.mjs";

export default scenario("production_progress_extrapolates_during_snapshot_gap", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
    quickstart: false,
  },
  network: { mode: "direct" },
  steps: [
    selectOwn("city_centre", 0),
    issue("train", { unit: "worker" }),
    waitForSnapshot({ minTickDelta: 2 }),
    capture("authoritative-production-started"),
    setClientSnapshotDelivery(false),
    waitMs(500),
    assertClientRenderedProductionProgress({
      kind: "city_centre",
      minProgress: 0.01,
      maxProgress: 0.98,
      predicted: true,
    }),
    setClientSnapshotDelivery(true),
    waitForSnapshot({ minTickDelta: 1 }),
    capture("authoritative-production-resumed"),
  ],
});
