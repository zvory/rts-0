import {
  assertLocalDisabledReason,
  issue,
  scenario,
  selectOwn,
} from "../dsl.mjs";

export default scenario("unsupported_command_is_explicit", {
  setup: {
    kind: "liveRoom",
    prediction: "disabled",
  },
  network: { mode: "direct" },
  steps: [
    selectOwn("worker", 0),
    issue("build", { building: "depot", tileX: 1, tileY: 1 }),
    assertLocalDisabledReason("buildPredictionUnsupported"),
  ],
});
