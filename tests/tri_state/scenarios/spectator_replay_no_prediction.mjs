import {
  assertClientPrediction,
  capture,
  scenario,
  setReplaySpeed,
} from "../dsl.mjs";

export default scenario("spectator_replay_no_prediction", {
  setup: {
    kind: "devScenario",
    prediction: "enabled",
    devScenario: {
      id: "vehicle_small_block_baseline",
      unit: "scout_car",
      count: 5,
    },
  },
  network: { mode: "direct", name: "spectator-direct" },
  steps: [
    setReplaySpeed(0),
    capture("spectator-watch"),
    assertClientPrediction({ enabled: false, mode: "disabled" }),
  ],
});
