import { assert } from "./assertions.mjs";
import {
  stressTestForegroundReady,
  stressTestHasEnoughFrames,
  stressTestHeadroom,
} from "../../client/src/stress_test.js";
import {
  analyzeSelfProfile,
  renderSelfProfileFlamegraph,
  selfProfileSummary,
} from "../../client/src/stress_test_profile.js";

{
  const trace = {
    resources: [{ url: "https://example.test/src/match.js" }],
    frames: [
      { name: "frame", resourceId: 0, line: 10, column: 1 },
      { name: "render", resourceId: 0, line: 20, column: 1 },
    ],
    stacks: [
      { frameId: 0 },
      { frameId: 1, parentId: 0 },
    ],
    samples: [
      { timestamp: 0, stackId: 1 },
      { timestamp: 10, stackId: 1 },
      { timestamp: 20, stackId: 1 },
    ],
  };
  const analysis = analyzeSelfProfile(trace);
  const summary = selfProfileSummary(analysis);
  const svg = renderSelfProfileFlamegraph(analysis, { title: "Fixture profile" });
  assert(summary.sampleCount === 3, "self-profile analysis retains the browser sample count");
  assert(summary.topSelf[0]?.name === "render", "self-profile analysis attributes leaf self time");
  assert(summary.topInclusive.some((row) => row.name === "frame"),
    "self-profile analysis retains inclusive parent time");
  assert(svg.includes("Fixture profile") && svg.includes("render"),
    "self-profile analysis renders a labeled SVG flame graph");
}

{
  assert(stressTestHeadroom(8).sustainableFps === 120,
    "8ms p95 reports the 120 FPS frame-work tier");
  assert(stressTestHeadroom(34).text.includes("less frame work"),
    "slow results explain the approximate speedup required for 60 FPS");
  assert(stressTestForegroundReady({ hidden: false, hasFocus: () => true }),
    "foreground readiness requires a visible focused document");
  assert(!stressTestForegroundReady({ hidden: true, hasFocus: () => true }) &&
    !stressTestForegroundReady({ hidden: false, hasFocus: () => false }),
  "foreground readiness rejects hidden and unfocused attempts");
  assert(stressTestHasEnoughFrames(1) && !stressTestHasEnoughFrames(0),
    "one frame keeps sub-1 FPS hardware reportable without accepting a stalled renderer");
}

{
  const { stressTestLaunchConfig } = await import("../../client/src/stress_test_launch.js");
  const config = stressTestLaunchConfig({
    pathname: "/stress-test",
    search: "?label=Matt%20%3Cbad%3E%20laptop&seconds=99",
  });
  assert(config?.id === "supply-300-hellhole", "stress route selects the canonical snapshot stream");
  assert(config?.label === "Matt _bad_ laptop", "stress route bounds and sanitizes artifact labels");
  assert(config?.durationSeconds === 25, "stress route bounds the measurement window below stream rollover");
  assert(stressTestLaunchConfig({ pathname: "/stress-test", search: "" })?.durationSeconds === 5,
    "stress route uses a five-second default measurement window when seconds is absent");
}
