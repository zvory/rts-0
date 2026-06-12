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

export default scenario("rally_optimistic_marker_confirms_after_ack", {
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
    issue("setRally", { dx: 160, dy: 80, kind: "attackMove" }),
    capture("optimistic-rally-issued"),
    assertClientOptimisticUi({ rallyCount: 1, rallyPlanLength: 1 }),
    setClientSnapshotDelivery(true),
    waitForAck(1),
    capture("rally-authoritative-confirmed"),
    assertClientOptimisticUi({ rallyCount: 0 }),
    assertClientPrediction({ latestAckSeq: 1, pendingCommandCount: 0, uiConfirmedCount: 1 }),
  ],
});
