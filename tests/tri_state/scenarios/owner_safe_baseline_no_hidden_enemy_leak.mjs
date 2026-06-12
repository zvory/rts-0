import {
  assertLocalBaselineOwnerSafe,
  capture,
  scenario,
} from "../dsl.mjs";

export default scenario("owner_safe_baseline_no_hidden_enemy_leak", {
  setup: {
    kind: "liveRoom",
    prediction: "disabled",
    quickstart: true,
  },
  network: { mode: "direct" },
  steps: [
    capture("baseline-artifact"),
    assertLocalBaselineOwnerSafe(),
  ],
});
