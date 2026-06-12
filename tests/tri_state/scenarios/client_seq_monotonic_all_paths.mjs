import {
  assertClientSeqsStrictlyIncreasing,
  capture,
  issue,
  scenario,
  selectOwn,
} from "../dsl.mjs";

export default scenario("client_seq_monotonic_all_paths", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "enabled",
    quickstart: true,
  },
  network: { mode: "direct" },
  steps: [
    selectOwn("worker", 0),
    issue("move", { dx: 48, dy: 0 }),
    issue("attackMove", { dx: 96, dy: 32, queued: true }),
    issue("stop"),
    selectOwn("city_centre", 0),
    issue("train", { unit: "worker" }),
    issue("setRally", { dx: 128, dy: 64, kind: "attackMove" }),
    capture("after-representative-command-paths"),
    assertClientSeqsStrictlyIncreasing({ count: 5 }),
  ],
});
