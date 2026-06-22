import {
  assertLocalDisabledReason,
  assertLocalReady,
  capture,
  scenario,
} from "../dsl.mjs";

export default scenario("local_lane_initializes_from_start", {
  setup: {
    kind: "liveRoom",
    prediction: "disabled",
    localBaseline: "none",
  },
  network: { mode: "direct" },
  steps: [
    capture("start-no-baseline"),
    assertLocalReady(),
    assertLocalDisabledReason("baselineNotImported"),
  ],
});
