import { capture, forceFailure, scenario } from "../dsl.mjs";

export default scenario("forced_failure_artifact", {
  setup: {
    kind: "artifactOnly",
  },
  network: { mode: "none" },
  steps: [
    capture("before-forced-failure"),
    forceFailure("intentional tri-state forced failure artifact"),
  ],
});
