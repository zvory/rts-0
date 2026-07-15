import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  analyzeCpuProfile,
  parseCpuFlameGraphArgs,
  renderCpuFlameGraph,
  writeCpuFlameGraphArtifacts,
} from "../../scripts/client-cpu-profile-to-flamegraph.mjs";
import {
  cancelCpuProfile,
  configurePageEmulation,
  startCpuProfile,
  stopCpuProfile,
} from "../../scripts/client-perf/browser_profile.mjs";
import { parseClientFlameGraphArgs } from "../../scripts/client-flamegraph.mjs";

const syntheticProfile = {
  startTime: 1_000,
  endTime: 3_000,
  nodes: [
    {
      id: 1,
      callFrame: { functionName: "(root)", url: "", lineNumber: -1 },
      children: [2, 4],
    },
    {
      id: 2,
      callFrame: { functionName: "frame", url: "http://localhost/src/match.js", lineNumber: 10 },
      children: [3],
    },
    {
      id: 3,
      callFrame: { functionName: "hotFunction", url: "http://localhost/src/hot.js", lineNumber: 20 },
    },
    {
      id: 4,
      callFrame: { functionName: "(idle)", url: "", lineNumber: -1 },
    },
  ],
  samples: [3, 3, 2, 4],
  timeDeltas: [500, 500, 500, 500],
};

{
  const defaults = parseClientFlameGraphArgs([]);
  assert.equal(defaults.workload, "supply-300-hellhole-stream");
  assert.equal(defaults.seconds, 15);
  assert.equal(defaults.intervalUs, 500);
  assert.equal(defaults.preview, false);

  const custom = parseClientFlameGraphArgs([
    "--workload", "supply-300-active",
    "--seconds", "20",
    "--interval-us", "1000",
    "--cpu-throttle", "2",
    "--viewport", "1920x1080",
    "--dpr", "2",
    "--preview",
  ]);
  assert.equal(custom.workload, "supply-300-active");
  assert.equal(custom.seconds, 20);
  assert.equal(custom.intervalUs, 1000);
  assert.equal(custom.cpuThrottle, 2);
  assert.equal(custom.viewport, "1920x1080");
  assert.equal(custom.dpr, 2);
  assert.equal(custom.preview, true);

  assert.throws(() => parseClientFlameGraphArgs(["--interval-us", "99"]), /between 100 and 100000/);
  assert.throws(() => parseClientFlameGraphArgs(["--viewport", "wide"]), /must look like/);
  assert.throws(() => parseClientFlameGraphArgs(["--viewport", "0x900"]), /must look like/);
  assert.throws(() => parseClientFlameGraphArgs(["--unknown"]), /unknown argument/);

  assert.throws(
    () => parseCpuFlameGraphArgs(["--input", "profile", "--output", "graph", "--width", "NaN"]),
    /--width must be a positive number/,
  );
}

{
  const analysis = analyzeCpuProfile(syntheticProfile);
  assert.equal(analysis.summary.sampleCount, 4);
  assert.equal(analysis.summary.sampledUs, 2_000);
  assert.equal(analysis.summary.wallDurationUs, 2_000);
  assert.equal(analysis.summary.topSelfByFunction[0].functionName, "hotFunction");
  assert.equal(analysis.summary.topSelfByFunction[0].selfPct, 50);
  assert.equal(analysis.summary.topInclusive.find((row) => row.functionName === "frame")?.totalPct, 75);

  const svg = renderCpuFlameGraph(analysis, { title: "Synthetic flame graph", width: 1200 });
  assert.match(svg, /Synthetic flame graph/);
  assert.match(svg, /hotFunction/);
  assert.match(svg, /fill="#fb7185"/);
  assert.match(svg, /fill="#d1d5db"/);

  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "rts-client-flamegraph-test-"));
  try {
    const profilePath = path.join(temporary, "profile.cpuprofile");
    const svgPath = path.join(temporary, "flamegraph.svg");
    fs.writeFileSync(profilePath, JSON.stringify(syntheticProfile));
    const result = writeCpuFlameGraphArtifacts({ profilePath, svgPath, title: "Written graph" });
    assert.equal(fs.existsSync(svgPath), true);
    assert.equal(fs.existsSync(result.summaryPath), true);
    const summary = JSON.parse(fs.readFileSync(result.summaryPath, "utf8"));
    assert.equal(summary.topSelfByFunction[0].label, "hotFunction — /src/hot.js:21");
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}

assert.throws(() => analyzeCpuProfile({ nodes: [] }), /no call-tree nodes/);
assert.throws(() => analyzeCpuProfile({ nodes: syntheticProfile.nodes }), /no samples/);
assert.throws(
  () => analyzeCpuProfile({ ...syntheticProfile, timeDeltas: [500] }),
  /samples and time deltas differ in length/,
);
assert.throws(
  () => analyzeCpuProfile({ ...syntheticProfile, samples: [999], timeDeltas: [500] }),
  /references missing node/,
);

{
  const failingSession = {
    detached: false,
    async send() { throw new Error("CDP failure"); },
    async detach() { this.detached = true; },
  };
  const page = { target: () => ({ createCDPSession: async () => failingSession }) };
  await assert.rejects(() => configurePageEmulation(page, 2), /CDP failure/);
  assert.equal(failingSession.detached, true, "failed emulation setup detaches its session");
  failingSession.detached = false;
  await assert.rejects(() => startCpuProfile(page, "500"), /CDP failure/);
  assert.equal(failingSession.detached, true, "failed profiler setup detaches its session");
}

{
  const calls = [];
  const session = {
    detached: 0,
    async send(method, params) {
      calls.push({ method, params });
      if (method === "Profiler.stop") return { profile: syntheticProfile };
      return {};
    },
    async detach() {
      this.detached += 1;
    },
  };
  const page = { target: () => ({ createCDPSession: async () => session }) };
  assert.equal(await configurePageEmulation(page, 1), null);
  assert.equal(await configurePageEmulation(page, 2), session);
  assert.equal(calls.at(-1)?.method, "Emulation.setCPUThrottlingRate");

  await assert.rejects(() => startCpuProfile(page, "99", session), /integer between 100 and 100000/);
  const controller = await startCpuProfile(page, "500", session);
  assert.deepEqual(
    calls.slice(-3).map((call) => call.method),
    ["Profiler.enable", "Profiler.setSamplingInterval", "Profiler.start"],
  );
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "rts-client-profile-session-test-"));
  try {
    const outputPath = path.join(temporary, "profile.cpuprofile");
    assert.equal(await stopCpuProfile(controller, outputPath), outputPath);
    assert.equal(fs.existsSync(outputPath), true);
    assert.equal(JSON.parse(fs.readFileSync(outputPath, "utf8")).samples.length, 4);
    await cancelCpuProfile(controller);
    assert.equal(calls.filter((call) => call.method === "Profiler.stop").length, 1);
    assert.equal(session.detached, 0, "a caller-owned throttling session stays attached");

    const ownedController = await startCpuProfile(page, "500");
    await cancelCpuProfile(ownedController);
    assert.equal(session.detached, 1, "a profiler-owned session is detached on cancellation");
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}
