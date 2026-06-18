import {
  assertTickAdvanced,
  capture,
  scenario,
  setRoomTimeSpeed,
  stepRoomTime,
} from "../dsl.mjs";

export default scenario("dev_scenario_step_tick", {
  setup: {
    kind: "devScenario",
    devScenario: {
      id: "vehicle_small_block_baseline",
      unit: "scout_car",
      count: 5,
      blocker: "none",
    },
  },
  network: { mode: "direct" },
  steps: [
    setRoomTimeSpeed(0),
    capture("paused"),
    stepRoomTime(),
    assertTickAdvanced({ delta: 1 }),
    capture("after-one-step"),
  ],
});
