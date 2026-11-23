import assert from "node:assert/strict";
import { MatchHealth } from "../client/src/match_health.js";
import { MatchNetReporter } from "../client/src/match_net_reporter.js";

const previous = Object.fromEntries(["window", "document", "performance", "__RTS_BUILD__"].map((key) => [key, globalThis[key]]));
let now = 0;
let accept = true;
let presented = 5;
const reports = [];
const listeners = new Map();
const listen = (name, fn) => listeners.set(name, fn);
const unlisten = (name, fn) => { if (listeners.get(name) === fn) listeners.delete(name); };
globalThis.performance = { now: () => now };
globalThis.document = { hidden: false, hasFocus: () => true, addEventListener: listen, removeEventListener: unlisten };
globalThis.window = { addEventListener: listen, removeEventListener: unlisten, setInterval: () => 123 };
globalThis.__RTS_BUILD__ = "observed-build";
try {
  const health = new MatchHealth({ net: {}, statusBadge: null, snapshotMs: 33 });
  const reporter = new MatchNetReporter({
    net: { netReport: (report) => { reports.push(report); return accept; } },
    health,
    snapshotProcessingReport: { reset() {}, snapshotApplySummary: () => ({}), predictionApplySummary: () => ({}) },
    getRenderWorkerDiagnostics: () => ({ presented, workerUpdateMs: { samples: 2048, max: 45.2, p95: 8.3 } }),
  });
  reporter.startNetReports();
  for (const gap of [16, 16, 40, 60, 120]) health.noteFrameGap(gap, now += gap);
  now = 10000;
  reporter.sendNetReport();
  assert.equal(reports.at(-1).performanceSummary, null);
  assert.equal(health.reportStats.frameCount, 0, "exceptional window reset");
  assert.equal(health.performanceWindow.frameCount, 5, "minute aggregate survives exceptional reset");
  document.hidden = true;
  listeners.get("visibilitychange")();
  document.hidden = false;
  listeners.get("visibilitychange")();
  listeners.get("blur")();
  listeners.get("focus")();
  now = 60000;
  presented = 100;
  accept = false;
  reporter.sendNetReport();
  const summary = reports.at(-1).performanceSummary;
  assert.equal(reports.at(-1).clientBuildId, "observed-build");
  assert.equal(summary.elapsedMs, 60000);
  assert.equal(summary.frameCount, 5);
  assert.equal(summary.frameTotalMs, 252);
  assert.equal(summary.frameGapMaxMs, 120);
  assert.equal(summary.framesOver33Ms, 3);
  assert.equal(summary.framesOver50Ms, 2);
  assert.equal(summary.framesOver100Ms, 1);
  assert.equal(summary.frameGapP95Ms, 250);
  assert.equal(summary.frameGapP99Ms, 250);
  assert.equal(summary.workerPresentedCount, 95);
  assert.equal(summary.workerCountersValid, true);
  assert.equal(summary.visibleThroughout, false, "hide/show cannot masquerade as a foreground minute");
  assert.equal(summary.focusedThroughout, false, "blur/refocus invalidates focus for the window");
  assert.equal(summary.visibilityChangeCount, 2);
  assert.deepEqual(summary.workerRollingUpdate, { samples: 2048, maxMs: 45, p95Ms: 8 });
  assert.equal(health.performanceWindow.frameCount, 5, "rejected send preserves minute aggregate");
  now = 61000;
  accept = true;
  reporter.sendNetReport();
  assert.equal(reports.at(-1).performanceSummary.frameCount, 5);
  assert.equal(health.performanceWindow.frameCount, 0);
  assert.equal(reporter.visibleThroughout, true);
  assert.equal(reporter.focusedThroughout, true);
  health.noteFrameGap(20, 62000);
  presented = 10; // host diagnostic reset: never report a misleading negative delta
  now = 62000;
  reporter.sendNetReport({ final: true });
  assert.equal(reports.at(-1).performanceSummary.workerCountersValid, false);
  assert.equal(reports.at(-1).performanceSummary.elapsedMs, 1000);
  assert.equal(reports.at(-1).performanceSummary.frameCount, 1);
  reporter.finishPerformanceWindow();
  now = 180000;
  reporter.sendNetReport();
  assert.equal(reports.at(-1).performanceSummary, null, "stopped gameplay does not emit idle throughput windows");
  reporter.stopNetReports();
  assert.equal(listeners.size, 0, "all visibility/focus listeners removed at teardown");
} finally {
  for (const [key, value] of Object.entries(previous)) {
    if (value === undefined) delete globalThis[key]; else globalThis[key] = value;
  }
}
console.log("client performance summary contracts passed");
