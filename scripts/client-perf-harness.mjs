#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  decodeMessagePack,
  formatBakeoffMarkdown,
  runSnapshotCodecBakeoff,
} from "./snapshot-codec-bakeoff.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "..");
const SERVER_DIR = path.join(REPO_ROOT, "server");
const TESTS_DIR = path.join(REPO_ROOT, "tests");
const DEFAULT_OUTPUT_ROOT = path.join(REPO_ROOT, "target", "client-perf");
const DEFAULT_CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const DEFAULT_VIEWPORT = Object.freeze({ width: 1440, height: 900 });
const DEFAULT_DURATION_MS = 6000;
const DEFAULT_CODEC_SAMPLE_LIMIT = 240;
const DEFAULT_CPU_THROTTLE_RATE = 1;
const DEFAULT_DEVICE_SCALE_FACTOR = 1;
const DEFAULT_MATRIX_REPEAT_COUNT = 1;
const DEFAULT_MATRIX_CPU_THROTTLES = Object.freeze([1, 2]);
const DEFAULT_MATRIX_DEVICE_SCALE_FACTORS = Object.freeze([1]);
export const MATRIX_VIEWPORT_PRESETS = Object.freeze({
  small: Object.freeze({ label: "small", width: 1024, height: 768 }),
  default: Object.freeze({ label: "default", ...DEFAULT_VIEWPORT }),
  large: Object.freeze({ label: "large", width: 1920, height: 1080 }),
});
const DEFAULT_MATRIX_VIEWPORTS = Object.freeze([MATRIX_VIEWPORT_PRESETS.default]);
export const RENDER_TARGET_FPS = 120;
export const RENDER_FRAME_BUDGET_MS = 8.33;
export const RENDER_FRAME_BUDGET_TARGETS = Object.freeze([
  Object.freeze({ fps: 60, frameBudgetMs: 16.67 }),
  Object.freeze({ fps: 120, frameBudgetMs: 8.33 }),
  Object.freeze({ fps: 240, frameBudgetMs: 4.17 }),
  Object.freeze({ fps: 480, frameBudgetMs: 2.08 }),
]);
export const RECURRING_PHASE_WARN_MS = 1;
export const RECURRING_PHASE_HIGH_WARN_MS = 2;
const MAX_RECURRING_WARNINGS = 8;
const INCIDENT_REPLAY_SOURCE = process.env.RTS_CLIENT_PERF_INCIDENT_REPLAY
  ? path.resolve(process.env.RTS_CLIENT_PERF_INCIDENT_REPLAY)
  : null;
const WORKLOADS = Object.freeze([
  {
    id: "vehicle-wall-stress",
    description: "No-fog dev scenario with 15 tanks moving through a wall chokepoint.",
    kind: "devScenario",
    url: "/dev/scenarios?id=scout_car_wall_chokepoint&unit=tank&count=15",
  },
  {
    id: "selected-unit-hud-stress",
    description: "No-fog dev scenario with four selected tanks to exercise HUD and selection overlays.",
    kind: "devScenario",
    url: "/dev/scenarios?id=scout_car_snaking_corridor&unit=tank&count=4",
    setup: {
      selectFirstEntities: 4,
      minSelectedCount: 1,
    },
  },
  ...(INCIDENT_REPLAY_SOURCE ? [{
    id: "incident-120-commander-endgame",
    description: "Paused Commander-perspective replay at the 244-entity late-game render incident.",
    kind: "replayArtifact",
    source: INCIDENT_REPLAY_SOURCE,
    replayName: "incident-120-commander-endgame",
    url: "/?replayArtifact=incident-120-commander-endgame",
    setup: {
      visionSelectionPlayerId: 8,
      setRoomTimeSpeed: 8,
      waitRoomTimeTo: 29643,
      roomTimeWaitTimeoutMs: 90000,
      setRoomTimeSpeedAfterWait: 0,
      waitForMinEntities: 240,
      entityWaitTimeoutMs: 30000,
      resetPerfAfterSetup: true,
    },
  }] : []),
]);
const RENDER_LAG_WORKLOAD_IDS = Object.freeze(WORKLOADS.map((workload) => workload.id));

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.list) {
    for (const workload of WORKLOADS) {
      console.log(`${workload.id}\t${workload.description}`);
    }
    return;
  }

  const selected = selectedWorkloads(args);
  const outputRoot = path.resolve(args.outputRoot || DEFAULT_OUTPUT_ROOT);
  fs.mkdirSync(outputRoot, { recursive: true });
  const puppeteer = await loadPuppeteer();
  const chrome = findChrome(args.chrome);
  const server = await startOrReuseServer(args);
  const browser = await launchBrowser(puppeteer, chrome, args);

  try {
    if (args.stressMatrix) {
      await runStressMatrix({ selected, server, browser, outputRoot, args, chrome });
    } else {
      await runSelectedWorkloads({ selected, server, browser, outputRoot, args, chrome });
    }
  } finally {
    await browser.close().catch(() => {});
    await server.close();
  }
}

async function runSelectedWorkloads({ selected, server, browser, outputRoot, args, chrome }) {
  const results = [];
  let failed = 0;
  for (const workload of selected) {
    const result = await runWorkload({ workload, server, browser, outputRoot, args, chrome });
    results.push(result);
    reportWorkloadResult(result);
    if (result.status !== "passed") failed += 1;
  }
  const comparison = results.length > 0 ? writeRenderLagComparisonSummary(results, outputRoot, args) : null;
  if (comparison) console.log(`render lag comparison summary: ${comparison.summaryJson}`);

  if (failed > 0) {
    process.exitCode = 1;
  } else if (results.length > 0) {
    console.log(`client perf artifacts: ${outputRoot}`);
  }
}

async function runStressMatrix({ selected, server, browser, outputRoot, args, chrome }) {
  const cells = buildRenderStressMatrixCells({
    workloads: selected,
    cpuThrottles: args.matrixCpuThrottles,
    viewports: args.matrixViewports,
    deviceScaleFactors: args.matrixDeviceScaleFactors,
    repeatCount: args.matrixRepeatCount,
  });
  const results = [];
  let failed = 0;
  console.log(`render stress matrix cells: ${cells.length}`);
  for (const cell of cells) {
    const runArgs = {
      ...args,
      viewport: {
        width: cell.viewport.width,
        height: cell.viewport.height,
        deviceScaleFactor: cell.deviceScaleFactor,
      },
      cpuThrottleRate: cell.cpuThrottleRate,
      deviceScaleFactor: cell.deviceScaleFactor,
      activeMatrixCell: cell,
    };
    const result = await runWorkload({
      workload: cell.workload,
      server,
      browser,
      outputRoot,
      args: runArgs,
      chrome,
    });
    results.push(result);
    reportWorkloadResult(result);
    if (result.status !== "passed") failed += 1;
  }

  const matrix = results.length > 0 ? writeRenderStressMatrixSummary(results, outputRoot, args) : null;
  if (matrix) {
    console.log(`render stress matrix summary: ${matrix.summaryJson}`);
    console.log(`render stress matrix markdown: ${matrix.summaryMarkdown}`);
    const firstFailure = matrix.summary.rankedFailures?.[0] || null;
    if (firstFailure) {
      const missed = firstFailure.nextMissedBudget;
      console.log(
        `first failing stress cell: ${firstFailure.workloadId} ${firstFailure.configLabel} `
        + `misses ${missed.fps} FPS by ${formatMs(Math.abs(missed.p95MarginMs))}; `
        + `top phase ${firstFailure.topMeasuredPhase?.label || "unknown"}`,
      );
    } else {
      console.log("first failing stress cell: none");
    }
  }

  if (failed > 0) {
    process.exitCode = 1;
  } else if (results.length > 0) {
    console.log(`client perf artifacts: ${outputRoot}`);
  }
}

function reportWorkloadResult(result) {
  const status = result.status === "passed" ? "PASS" : "FAIL";
  const matrix = result.matrixCell ? ` ${result.matrixCell.configLabel} repeat ${result.matrixCell.repeatIndex}/${result.matrixCell.repeatCount}` : "";
  console.log(`${status} ${result.workloadId}${matrix} ${result.artifactDir}`);
  const budgetText = formatRenderBudgetConsole(result.renderBudget);
  if (budgetText) {
    for (const line of budgetText.split("\n")) console.log(`  ${line}`);
  }
  const diagnosticsText = formatRenderDiagnosticsConsole(result.renderDiagnostics);
  if (diagnosticsText) {
    for (const line of diagnosticsText.split("\n")) console.log(`  ${line}`);
  }
  if (result.status !== "passed") {
    for (const error of result.errors) console.error(`  ${error}`);
  }
}

async function runWorkload({ workload, server, browser, outputRoot, args, chrome }) {
  const timestamp = timestampForPath(new Date());
  const artifactDir = path.join(outputRoot, workload.id, timestamp);
  fs.mkdirSync(artifactDir, { recursive: true });
  const consoleErrors = [];
  const pageErrors = [];
  const requestFailures = [];
  const errors = [];
  let tracePath = null;
  let summary = null;
  let snapshotCodecBakeoff = null;
  let workloadSetup = null;

  try {
    await prepareWorkload(workload);
    const page = await browser.newPage();
    const cdpSession = await configurePageEmulation(page, args);
    if (args.snapshotCodecBakeoff) {
      await installSnapshotCodecCapture(page, args.snapshotCodecMaxSamples);
    }
    page.on("console", (message) => {
      const text = message.text();
      if (message.type() === "error") consoleErrors.push(text);
    });
    page.on("pageerror", (error) => pageErrors.push(error.message));
    page.on("requestfailed", (request) => {
      if (!request.url().includes("favicon")) {
        requestFailures.push(`${request.failure()?.errorText || "request failed"} ${request.url()}`);
      }
    });

    if (args.trace) {
      tracePath = path.join(artifactDir, "trace.json");
      await page.tracing.start({
        path: tracePath,
        screenshots: false,
        categories: ["devtools.timeline", "disabled-by-default-devtools.timeline"],
      });
    }

    await page.setViewport(puppeteerViewport(args.viewport, args.deviceScaleFactor));
    await page.evaluateOnNewDocument((workloadId) => {
      window.__rtsPerfWorkloadId = workloadId;
    }, workload.id);
    const targetUrl = new URL(workload.url, server.baseUrl).href;
    const startedAt = new Date().toISOString();
    const timeoutScale = workloadTimeoutScale(args);
    await page.goto(targetUrl, { waitUntil: "domcontentloaded", timeout: scaledTimeoutMs(args.navTimeoutMs, timeoutScale) });
    await page.waitForFunction(
      () => !!window.__rts?.match && !!window.__rtsPerf?.summary,
      { timeout: scaledTimeoutMs(args.startTimeoutMs, timeoutScale) },
    );
    await page.waitForSelector("#viewport canvas", { timeout: scaledTimeoutMs(5000, timeoutScale) });
    workloadSetup = await applyWorkloadSetup(page, workload);
    if (workload.setup?.resetPerfAfterSetup) {
      await page.evaluate(() => window.__rtsPerf?.reset?.());
    }
    await page.waitForFunction(
      () => (window.__rtsPerf?.summary?.()?.frameCount || 0) >= 30,
      { timeout: scaledTimeoutMs(Math.max(args.durationMs, 1000) + 10000, timeoutScale) },
    );
    await sleep(args.durationMs);

    summary = await collectPageSummary(page);
    if (args.snapshotCodecBakeoff) {
      const frames = normalizeCapturedSnapshotCodecFrames(await collectSnapshotCodecFrames(page));
      const framesPath = path.join(artifactDir, "snapshot-frames.jsonl");
      fs.writeFileSync(framesPath, frames.map((frame) => JSON.stringify(frame)).join("\n") + "\n");
      if (frames.length > 0) {
        const bakeoff = runSnapshotCodecBakeoff({
          frames,
          label: workload.id,
        });
        const summaryPath = path.join(artifactDir, "snapshot-codec-bakeoff.json");
        const markdownPath = path.join(artifactDir, "snapshot-codec-bakeoff.md");
        fs.writeFileSync(summaryPath, `${JSON.stringify(bakeoff, null, 2)}\n`);
        fs.writeFileSync(markdownPath, formatBakeoffMarkdown(bakeoff));
        snapshotCodecBakeoff = {
          samples: frames.length,
          framesJsonl: framesPath,
          summaryJson: summaryPath,
          markdown: markdownPath,
          recommendation: bakeoff.recommendation,
          candidates: bakeoff.candidates.map((candidate) => ({
            id: candidate.id,
            p95Bytes: candidate.bytes.p95,
            maxBytes: candidate.bytes.max,
            overBudgetPctX100: candidate.bytes.overBudgetPctX100,
            encodeP95Ms: candidate.encodeMs.p95,
            decodeP95Ms: candidate.decodeMs.p95,
          })),
        };
      } else {
        snapshotCodecBakeoff = {
          samples: 0,
          framesJsonl: framesPath,
          recommendation: {
            summary: "No snapshot frames were captured for this workload.",
            reason: "The page did not receive compact snapshot frames before collection ended.",
          },
          candidates: [],
        };
      }
    }
    const version = await fetchText(new URL("/version", server.baseUrl).href).catch((err) => ({
      error: err.message,
    }));
    const endedAt = new Date().toISOString();
    const renderBudget = buildRenderBudgetReport(summary.perf?.summary, summary.perf?.reportSummary);
    const renderDiagnostics = buildRenderDiagnosticsReport(summary.perf?.summary, summary.perf?.reportSummary);
    const artifact = {
      schemaVersion: 1,
      status: "passed",
      workload: {
        id: workload.id,
        kind: workload.kind,
        description: workload.description,
        url: workload.url,
        replayName: workload.replayName || null,
      },
      run: {
        startedAt,
        endedAt,
        durationMs: args.durationMs,
        targetUrl,
        baseUrl: server.baseUrl,
        reusedServer: server.reused,
        cpuThrottleRate: args.cpuThrottleRate,
      },
      workloadSetup,
      stressMatrix: args.activeMatrixCell ? serializeMatrixCell(args.activeMatrixCell) : null,
      browser: {
        chrome,
        viewport: puppeteerViewport(args.viewport, args.deviceScaleFactor),
        requestedDeviceScaleFactor: args.deviceScaleFactor,
        userAgent: summary.userAgent,
        devicePixelRatio: summary.devicePixelRatio,
      },
      build: version,
      websocket: summary.websocket,
      health: summary.health,
      perf: summary.perf,
      renderBudget,
      renderDiagnostics,
      clientNetReport: summary.clientNetReport,
      snapshotPacketBudget: snapshotPacketBudgetSummary(summary.clientNetReport),
      snapshotCodecBakeoff,
      page: {
        title: summary.title,
        location: summary.location,
        canvas: summary.canvas,
        consoleErrors,
        pageErrors,
        requestFailures,
      },
      artifacts: {
        summaryJson: path.join(artifactDir, "summary.json"),
        traceJson: tracePath,
      },
      notes: [
        "This harness fails on runtime errors and missing summaries, not absolute FPS thresholds.",
        "Numbers are machine-local evidence for optimization work.",
      ],
    };

    if (!summary.perf?.summary || summary.perf.summary.frameCount <= 0) {
      errors.push("window.__rtsPerf.summary() was missing or empty");
    }
    if (!summary.clientNetReport) {
      errors.push("ClientNetReport snapshot could not be generated");
    }
    errors.push(...workloadSetupErrors(workload, workloadSetup));
    errors.push(...consoleErrors.map((error) => `console error: ${error}`));
    errors.push(...pageErrors.map((error) => `page error: ${error}`));
    errors.push(...requestFailures.map((error) => `request failure: ${error}`));

    if (args.trace) await page.tracing.stop();
    await cdpSession?.detach?.().catch(() => {});
    await page.close().catch(() => {});

    if (errors.length > 0) artifact.status = "failed";
    fs.writeFileSync(path.join(artifactDir, "summary.json"), `${JSON.stringify(artifact, null, 2)}\n`);
    return {
      status: artifact.status,
      workloadId: workload.id,
      artifactDir,
      errors,
      frameCount: summary.perf?.summary?.frameCount || 0,
      frameWorkP95Ms: summary.perf?.reportSummary?.frameWorkP95Ms || 0,
      rendererP95Ms: summary.perf?.reportSummary?.rendererP95Ms || 0,
      cpuThrottleRate: args.cpuThrottleRate,
      deviceScaleFactor: args.deviceScaleFactor,
      renderBudget,
      renderDiagnostics,
      topMeasuredPhase: topMeasuredPhase(renderBudget),
      matrixCell: args.activeMatrixCell ? serializeMatrixCell(args.activeMatrixCell) : null,
    };
  } catch (err) {
    errors.push(err.stack || err.message);
    try {
      if (args.trace) await browser.pages().then((pages) => pages.at(-1)?.tracing?.stop?.()).catch(() => {});
    } catch {
      // Best effort cleanup only.
    }
    const artifact = {
      schemaVersion: 1,
      status: "failed",
      workload: { id: workload.id, kind: workload.kind, description: workload.description },
      stressMatrix: args.activeMatrixCell ? serializeMatrixCell(args.activeMatrixCell) : null,
      errors,
      partialSummary: summary,
      page: { consoleErrors, pageErrors, requestFailures },
      artifacts: { summaryJson: path.join(artifactDir, "summary.json"), traceJson: tracePath },
    };
    fs.writeFileSync(path.join(artifactDir, "summary.json"), `${JSON.stringify(artifact, null, 2)}\n`);
    return {
      status: "failed",
      workloadId: workload.id,
      artifactDir,
      errors,
      cpuThrottleRate: args.cpuThrottleRate,
      deviceScaleFactor: args.deviceScaleFactor,
      matrixCell: args.activeMatrixCell ? serializeMatrixCell(args.activeMatrixCell) : null,
    };
  }
}

function snapshotPacketBudgetSummary(report) {
  if (!report) return null;
  return {
    snapshotBytesP95: numberOrNull(report.snapshotBytesP95),
    snapshotSegmentBudgetBytes: numberOrNull(report.snapshotSegmentBudgetBytes),
    snapshotOverSegmentBudgetCount: numberOrNull(report.snapshotOverSegmentBudgetCount),
    snapshotOverSegmentBudgetPctX100: numberOrNull(report.snapshotOverSegmentBudgetPctX100),
    snapshotByteSource: stringOrNull(report.snapshotByteSource),
    snapshotCodec: stringOrNull(report.snapshotCodec),
    snapshotCodecVersion: numberOrNull(report.snapshotCodecVersion),
    snapshotFrameKind: stringOrNull(report.snapshotFrameKind),
    websocketCompression: stringOrNull(report.websocketCompression),
    websocketExtensions: stringOrNull(report.websocketExtensions),
  };
}

function workloadTimeoutScale(args) {
  const rate = Number(args?.cpuThrottleRate || DEFAULT_CPU_THROTTLE_RATE);
  return Number.isFinite(rate) && rate > 1 ? rate : 1;
}

function scaledTimeoutMs(timeoutMs, scale) {
  const timeout = Number(timeoutMs);
  const factor = Number(scale);
  if (!Number.isFinite(timeout) || timeout <= 0) return timeoutMs;
  if (!Number.isFinite(factor) || factor <= 1) return timeout;
  return Math.ceil(timeout * factor);
}

export function buildRenderBudgetReport(perfSummary, reportSummary = null) {
  const phases = Array.isArray(perfSummary?.phases) ? perfSummary.phases : [];
  const framePhase = phaseByLabel(phases, "frame.work");
  const frameWorkAvgMs = numericMetric(framePhase?.avgMs);
  const frameWorkP95Ms = numericMetric(framePhase?.p95Ms ?? reportSummary?.frameWorkP95Ms);
  const frameWorkMaxMs = numericMetric(framePhase?.maxMs ?? reportSummary?.frameWorkMaxMs);
  const frameWorkBudgetMargins = buildFrameWorkBudgetMargins({
    avgMs: frameWorkAvgMs,
    p95Ms: frameWorkP95Ms,
    maxMs: frameWorkMaxMs,
  });
  const frameAttribution = buildFrameAttributionReport(phases, reportSummary, {
    avgMs: frameWorkAvgMs,
    p95Ms: frameWorkP95Ms,
    maxMs: frameWorkMaxMs,
  });
  const nextMissedBudget = nextMissedFrameWorkBudget(frameWorkBudgetMargins, "p95");
  const worstPhase = perfSummary?.worstPhase || (
    reportSummary?.worstFramePhase
      ? { label: reportSummary.worstFramePhase, count: null }
      : null
  );
  const recurringPhaseWarnings = phases
    .filter((phase) => phase?.label && phase.label !== "frame.work" && phase.label !== "frame.gap")
    .map((phase) => ({
      label: phase.label,
      count: numberOrNull(phase.count),
      avgMs: numericMetric(phase.avgMs),
      maxMs: numericMetric(phase.maxMs),
      p95Ms: numericMetric(phase.p95Ms),
      worstPhaseCount: worstPhase?.label === phase.label ? numberOrNull(worstPhase.count) : 0,
    }))
    .filter((phase) => (phase.p95Ms ?? phase.avgMs ?? 0) >= RECURRING_PHASE_WARN_MS)
    .sort((a, b) =>
      (b.p95Ms ?? 0) - (a.p95Ms ?? 0)
      || (b.maxMs ?? 0) - (a.maxMs ?? 0)
      || a.label.localeCompare(b.label),
    )
    .slice(0, MAX_RECURRING_WARNINGS)
    .map((phase) => ({
      ...phase,
      severity: (phase.p95Ms ?? 0) >= RECURRING_PHASE_HIGH_WARN_MS ? "high" : "warn",
    }));

  const warnings = [];
  if (nextMissedBudget) {
    const missedByMs = Math.abs(nextMissedBudget.p95MarginMs);
    const clears120 = frameWorkP95Ms != null && frameWorkP95Ms <= RENDER_FRAME_BUDGET_MS;
    warnings.push({
      kind: nextMissedBudget.fps <= RENDER_TARGET_FPS
        ? "frame_work_p95_over_budget"
        : "frame_work_p95_misses_headroom_budget",
      severity: nextMissedBudget.fps <= RENDER_TARGET_FPS ? "high" : "warn",
      fps: nextMissedBudget.fps,
      frameBudgetMs: nextMissedBudget.frameBudgetMs,
      p95MarginMs: nextMissedBudget.p95MarginMs,
      message: clears120
        ? `frame.work p95 ${formatMs(frameWorkP95Ms)} clears 120 FPS locally but misses the ${nextMissedBudget.fps} FPS headroom budget by ${formatMs(missedByMs)}`
        : `frame.work p95 ${formatMs(frameWorkP95Ms)} misses the ${nextMissedBudget.fps} FPS budget by ${formatMs(missedByMs)}`,
    });
  }
  const highRecurring = recurringPhaseWarnings.filter((phase) => phase.severity === "high");
  if (highRecurring.length > 0) {
    warnings.push({
      kind: "recurring_phase_over_2ms",
      severity: "warn",
      message: `recurring phase p95 above ${formatMs(RECURRING_PHASE_HIGH_WARN_MS)}: ${highRecurring.map((phase) => `${phase.label}=${formatMs(phase.p95Ms)}`).join(", ")}`,
    });
  } else if (recurringPhaseWarnings.length > 0) {
    warnings.push({
      kind: "recurring_phase_over_1ms",
      severity: "info",
      message: `recurring phase p95 above ${formatMs(RECURRING_PHASE_WARN_MS)}: ${recurringPhaseWarnings.map((phase) => `${phase.label}=${formatMs(phase.p95Ms)}`).join(", ")}`,
    });
  }

  return {
    schemaVersion: 1,
    status: warnings.length > 0 ? "warn" : "ok",
    target: {
      fps: RENDER_TARGET_FPS,
      frameBudgetMs: RENDER_FRAME_BUDGET_MS,
      frameBudgets: RENDER_FRAME_BUDGET_TARGETS,
      recurringPhaseWarnMs: RECURRING_PHASE_WARN_MS,
      recurringPhaseHighWarnMs: RECURRING_PHASE_HIGH_WARN_MS,
    },
    frameWork: {
      frameCount: numberOrNull(perfSummary?.frameCount ?? reportSummary?.frameCount),
      slowFrameCount: numberOrNull(perfSummary?.slowFrameCount ?? reportSummary?.slowFrameCount),
      avgMs: frameWorkAvgMs,
      p95Ms: frameWorkP95Ms,
      maxMs: frameWorkMaxMs,
      budgetMargins: frameWorkBudgetMargins,
      nextMissedBudget,
    },
    frameAttribution,
    worstPhase: worstPhase ? {
      label: worstPhase.label || "",
      count: numberOrNull(worstPhase.count),
    } : null,
    recurringPhaseWarnings,
    groups: {
      frame: summarizePhaseGroup(phases, (phase) => phase.label?.startsWith("frame.")),
      topLevel: summarizePhaseGroup(phases, (phase) => phase.label?.startsWith("match.")),
      rendererNested: summarizePhaseGroup(phases, (phase) => phase.label?.startsWith("renderer.")),
      minimapNested: summarizePhaseGroup(phases, (phase) => phase.label?.startsWith("minimap.")),
    },
    context: perfSummary?.context || reportSummary?.context || {},
    warnings,
    notes: [
      "Advisory only: this report does not fail CI on absolute FPS or frame timing.",
      "Frame budget margins are budget minus frame work; positive values clear the target.",
      "Do not add top-level frame.work to nested renderer/minimap phases when attributing cost.",
      "frame.unattributed is frame.work minus top-level match.* phases; high values mean the frame needs finer timing or off-RAF trace inspection.",
    ],
  };
}

export function buildRenderDiagnosticsReport(perfSummary, reportSummary = null) {
  const diagnostics = perfSummary?.renderDiagnostics || reportSummary?.renderDiagnostics || null;
  const counters = Array.isArray(diagnostics?.counters) ? diagnostics.counters.map(normalizeCounterRow) : [];
  const topCounters = counters
    .filter((counter) => counter.total > 0)
    .sort(compareDiagnosticCounterRows)
    .slice(0, 12);
  return {
    schemaVersion: 1,
    status: counters.length > 0 ? "ok" : "missing",
    topCounters,
    groups: {
      pixiObjectChurn: summarizeDiagnosticGroup(counters, ["renderer.pixi.displayObject."]),
      rigRedraws: summarizeDiagnosticGroup(counters, ["renderer.rig."]),
      graphicsClears: summarizeDiagnosticGroup(counters, ["renderer.graphics.clear."]),
      overlayRedraws: summarizeDiagnosticGroup(counters, ["renderer.redraw."]),
      minimapInvalidations: summarizeDiagnosticGroup(counters, ["minimap.invalidate.", "minimap.cache."]),
      entityViews: summarizeDiagnosticGroup(counters, ["entityViews."]),
      hudDirtyGuards: summarizeDiagnosticGroup(counters, ["hud.dirty."]),
      observerDirtyGuards: summarizeDiagnosticGroup(counters, ["observer.dirty."]),
    },
    likelyNextCounter: topCounters.find((counter) => !counter.label.endsWith(".total")) || topCounters[0] || null,
    recentLongFrames: sanitizeLongFrames(perfSummary?.recentLongFrames),
    context: perfSummary?.context || reportSummary?.context || {},
    notes: [
      "Local-only bounded counters; normal ClientNetReport uploads do not include raw frames or raw entity data.",
      "Counter totals explain churn and invalidation frequency. Use timing phases for milliseconds.",
    ],
  };
}

export function formatRenderDiagnosticsConsole(report) {
  if (!report || report.status === "missing") return "";
  const groups = Object.entries(report.groups || {})
    .filter(([, group]) => (group?.total || 0) > 0)
    .sort((a, b) => (b[1].total || 0) - (a[1].total || 0))
    .slice(0, 5)
    .map(([name, group]) => `${name}=${formatCount(group.total)}`)
    .join(" ");
  const next = report.likelyNextCounter
    ? ` top=${report.likelyNextCounter.label}:${formatCount(report.likelyNextCounter.total)}`
    : "";
  return `render diagnostics: ${groups || "no nonzero groups"}${next}`;
}

export function formatRenderBudgetConsole(report) {
  if (!report) return "";
  const frame = report.frameWork || {};
  const p95Margins = Array.isArray(frame.budgetMargins)
    ? frame.budgetMargins.map((budget) => `${budget.fps}=${formatSignedMs(budget.p95MarginMs)}`).join(" ")
    : "";
  const nextMissed = frame.nextMissedBudget
    ? ` next missed=${frame.nextMissedBudget.fps} FPS by ${formatMs(Math.abs(frame.nextMissedBudget.p95MarginMs))}`
    : " next missed=none";
  const worst = report.worstPhase?.label
    ? ` worst=${report.worstPhase.label}${report.worstPhase.count == null ? "" : ` x${report.worstPhase.count}`}`
    : "";
  const lines = [
    `render budget advisory: frame.work avg=${formatMs(frame.avgMs)} p95=${formatMs(frame.p95Ms)} max=${formatMs(frame.maxMs)} p95 margins ${p95Margins}${nextMissed} slow=${frame.slowFrameCount || 0}/${frame.frameCount || 0}${worst}`,
  ];
  const attribution = report.frameAttribution || null;
  if (attribution) {
    lines.push(
      `frame attribution: named top-level avg=${formatMs(attribution.topLevelAvgMs)} `
        + `unattributed avg=${formatMs(attribution.unattributedAvgMs)} p95=${formatMs(attribution.unattributedP95Ms)} `
        + `raf dispatch p95=${formatMs(attribution.rafDispatchP95Ms)} `
        + `coverage=${formatPercent(attribution.avgTopLevelCoveragePct)}`,
    );
  }
  if (report.recurringPhaseWarnings?.length) {
    lines.push(`recurring phase p95 >= ${formatMs(RECURRING_PHASE_WARN_MS)}: ${
      report.recurringPhaseWarnings
        .map((phase) => `${phase.label}=${formatMs(phase.p95Ms)}`)
        .join(", ")
    }`);
  }
  if (report.warnings?.length) {
    lines.push(`warnings: ${report.warnings.map((warning) => warning.message).join("; ")}`);
  }
  return lines.join("\n");
}

function writeRenderLagComparisonSummary(results, outputRoot, args) {
  const timestamp = timestampForPath(new Date());
  const artifactDir = path.join(outputRoot, "render-lag-comparison", timestamp);
  fs.mkdirSync(artifactDir, { recursive: true });
  const summaryJson = path.join(artifactDir, "summary.json");
  const summary = {
    schemaVersion: 1,
    suite: "render-lag-comparison",
    generatedAt: new Date().toISOString(),
    target: {
      fps: RENDER_TARGET_FPS,
      frameBudgetMs: RENDER_FRAME_BUDGET_MS,
      frameBudgets: RENDER_FRAME_BUDGET_TARGETS,
      recurringPhaseWarnMs: RECURRING_PHASE_WARN_MS,
      recurringPhaseHighWarnMs: RECURRING_PHASE_HIGH_WARN_MS,
    },
    command: {
      renderLagSuite: !!args.renderLagSuite,
      durationMs: args.durationMs,
      trace: !!args.trace,
      snapshotCodecBakeoff: !!args.snapshotCodecBakeoff,
      cpuThrottleRate: args.cpuThrottleRate,
      viewport: args.viewport,
      deviceScaleFactor: args.deviceScaleFactor,
    },
    workloads: results.map((result) => ({
      id: result.workloadId,
      status: result.status,
      artifactDir: result.artifactDir,
      cpuThrottleRate: result.cpuThrottleRate ?? args.cpuThrottleRate,
      deviceScaleFactor: result.deviceScaleFactor ?? args.deviceScaleFactor,
      frameCount: result.frameCount || 0,
      frameWorkAvgMs: result.renderBudget?.frameWork?.avgMs ?? null,
      frameWorkP95Ms: result.renderBudget?.frameWork?.p95Ms ?? null,
      frameWorkMaxMs: result.renderBudget?.frameWork?.maxMs ?? null,
      frameWorkBudgetMargins: result.renderBudget?.frameWork?.budgetMargins || [],
      nextMissedBudget: result.renderBudget?.frameWork?.nextMissedBudget || null,
      frameAttribution: result.renderBudget?.frameAttribution || null,
      worstPhase: result.renderBudget?.worstPhase || null,
      warnings: result.renderBudget?.warnings || [],
      recurringPhaseWarnings: result.renderBudget?.recurringPhaseWarnings || [],
      renderDiagnostics: result.renderDiagnostics || null,
    })),
    notes: [
      "Warnings are advisory and machine-local; compare branches on the same machine.",
      "Keep beta Matt/Alex per-player reports separate from local harness measurements.",
      "Detailed timing and trace artifacts stay under target/client-perf and are ignored by git.",
    ],
  };
  fs.writeFileSync(summaryJson, `${JSON.stringify(summary, null, 2)}\n`);
  return { artifactDir, summaryJson };
}

function writeRenderStressMatrixSummary(results, outputRoot, args) {
  const timestamp = timestampForPath(new Date());
  const artifactDir = path.join(outputRoot, "render-stress-matrix", timestamp);
  fs.mkdirSync(artifactDir, { recursive: true });
  const summary = buildRenderStressMatrixSummary(results, args);
  const summaryJson = path.join(artifactDir, "summary.json");
  const summaryMarkdown = path.join(artifactDir, "summary.md");
  fs.writeFileSync(summaryJson, `${JSON.stringify(summary, null, 2)}\n`);
  fs.writeFileSync(summaryMarkdown, formatRenderStressMatrixMarkdown(summary));
  return { artifactDir, summaryJson, summaryMarkdown, summary };
}

export function buildRenderStressMatrixCells({
  workloads,
  cpuThrottles,
  viewports,
  deviceScaleFactors,
  repeatCount,
}) {
  const cells = [];
  for (const workload of workloads) {
    for (const cpuThrottleRate of cpuThrottles) {
      for (const viewport of viewports) {
        for (const deviceScaleFactor of deviceScaleFactors) {
          const configLabel = matrixConfigLabel({ cpuThrottleRate, viewport, deviceScaleFactor });
          for (let repeatIndex = 1; repeatIndex <= repeatCount; repeatIndex += 1) {
            cells.push({
              id: `${workload.id}__${configLabel}__r${repeatIndex}`,
              workload,
              workloadId: workload.id,
              cpuThrottleRate,
              viewport,
              deviceScaleFactor,
              configLabel,
              repeatIndex,
              repeatCount,
            });
          }
        }
      }
    }
  }
  return cells;
}

export function buildRenderStressMatrixSummary(results, args = {}) {
  const generatedAt = new Date().toISOString();
  const groups = new Map();
  for (const result of results) {
    const cell = result.matrixCell || {
      workloadId: result.workloadId,
      cpuThrottleRate: result.cpuThrottleRate || DEFAULT_CPU_THROTTLE_RATE,
      viewport: args.viewport || MATRIX_VIEWPORT_PRESETS.default,
      deviceScaleFactor: args.deviceScaleFactor || DEFAULT_DEVICE_SCALE_FACTOR,
      configLabel: "single-run",
      repeatIndex: 1,
      repeatCount: 1,
    };
    const key = `${cell.workloadId}|${cell.configLabel}`;
    if (!groups.has(key)) {
      groups.set(key, {
        workloadId: cell.workloadId,
        configLabel: cell.configLabel,
        cpuThrottleRate: cell.cpuThrottleRate,
        viewport: cell.viewport,
        deviceScaleFactor: cell.deviceScaleFactor,
        repeatCount: cell.repeatCount,
        samples: [],
      });
    }
    groups.get(key).samples.push(result);
  }

  const cells = [...groups.values()].map((cell) => summarizeStressMatrixCell(cell));
  const rankedFailures = cells
    .filter((cell) => cell.nextMissedBudget)
    .sort(compareStressMatrixFailures)
    .map((cell, index) => ({ rank: index + 1, ...cell }));

  return {
    schemaVersion: 1,
    suite: "render-stress-matrix",
    generatedAt,
    target: {
      fps: RENDER_TARGET_FPS,
      frameBudgetMs: RENDER_FRAME_BUDGET_MS,
      frameBudgets: RENDER_FRAME_BUDGET_TARGETS,
      recurringPhaseWarnMs: RECURRING_PHASE_WARN_MS,
      recurringPhaseHighWarnMs: RECURRING_PHASE_HIGH_WARN_MS,
    },
    command: {
      stressMatrix: true,
      renderLagSuite: !!args.renderLagSuite,
      durationMs: args.durationMs || DEFAULT_DURATION_MS,
      trace: !!args.trace,
      snapshotCodecBakeoff: !!args.snapshotCodecBakeoff,
      repeatCount: args.matrixRepeatCount || DEFAULT_MATRIX_REPEAT_COUNT,
      cpuThrottles: args.matrixCpuThrottles || DEFAULT_MATRIX_CPU_THROTTLES,
      viewports: args.matrixViewports || DEFAULT_MATRIX_VIEWPORTS,
      deviceScaleFactors: args.matrixDeviceScaleFactors || DEFAULT_MATRIX_DEVICE_SCALE_FACTORS,
    },
    cells,
    rankedFailures,
    firstFailingCell: rankedFailures[0] || null,
    notes: [
      "Warnings are advisory and machine-local; compare branches on the same machine.",
      "CPU throttling uses Chrome DevTools Protocol Emulation.setCPUThrottlingRate and is a local stress proxy, not a model of one player's hardware.",
      "Representative p95 is the average of per-sample frame.work p95 values; p95SampleMaxMs keeps the largest repeated sample visible.",
      "Traces stay opt-in per workload run.",
    ],
  };
}

function summarizeStressMatrixCell(cell) {
  const successful = cell.samples.filter((sample) => sample.status === "passed");
  const frameAvg = successful.map((sample) => sample.renderBudget?.frameWork?.avgMs).filter(Number.isFinite);
  const frameP95 = successful.map((sample) => sample.renderBudget?.frameWork?.p95Ms).filter(Number.isFinite);
  const frameMax = successful.map((sample) => sample.renderBudget?.frameWork?.maxMs).filter(Number.isFinite);
  const representative = {
    avgMs: averageMetric(frameAvg),
    p95Ms: averageMetric(frameP95),
    maxMs: frameMax.length > 0 ? roundMetric(Math.max(...frameMax)) : null,
  };
  const budgetMargins = buildFrameWorkBudgetMargins(representative);
  const nextMissedBudget = nextMissedFrameWorkBudget(budgetMargins, "p95");
  const topMeasuredPhase = topMeasuredPhaseFromSamples(successful);
  const frameAttribution = summarizeStressMatrixFrameAttribution(successful);
  return {
    workloadId: cell.workloadId,
    configLabel: cell.configLabel,
    cpuThrottleRate: cell.cpuThrottleRate,
    viewport: cell.viewport,
    deviceScaleFactor: cell.deviceScaleFactor,
    repeatCount: cell.repeatCount,
    sampleCount: cell.samples.length,
    passedSampleCount: successful.length,
    failedSampleCount: cell.samples.length - successful.length,
    status: cell.samples.every((sample) => sample.status === "passed") ? "passed" : "failed",
    artifactDirs: cell.samples.map((sample) => sample.artifactDir),
    frameWork: {
      avgMs: representative.avgMs,
      p95Ms: representative.p95Ms,
      maxMs: representative.maxMs,
      p95SampleMinMs: frameP95.length > 0 ? roundMetric(Math.min(...frameP95)) : null,
      p95SampleMaxMs: frameP95.length > 0 ? roundMetric(Math.max(...frameP95)) : null,
      budgetMargins,
    },
    frameAttribution,
    nextMissedBudget,
    topMeasuredPhase,
    recurringPhaseWarnings: mergeRecurringPhaseWarnings(successful),
    errors: cell.samples.flatMap((sample) => sample.errors || []),
  };
}

function topMeasuredPhaseFromSamples(samples) {
  const phases = [];
  for (const sample of samples) {
    const top = topMeasuredPhase(sample.renderBudget);
    if (top) phases.push(top);
  }
  if (phases.length === 0) return null;
  return phases.sort((a, b) =>
    (b.p95Ms ?? 0) - (a.p95Ms ?? 0)
    || (b.maxMs ?? 0) - (a.maxMs ?? 0)
    || a.label.localeCompare(b.label),
  )[0];
}

function topMeasuredPhase(report) {
  const recurring = Array.isArray(report?.recurringPhaseWarnings) ? report.recurringPhaseWarnings : [];
  if (recurring.length > 0) {
    const phase = recurring[0];
    return {
      label: phase.label,
      p95Ms: phase.p95Ms,
      maxMs: phase.maxMs,
      source: "recurringPhaseWarnings",
    };
  }
  if (report?.worstPhase?.label) {
    return {
      label: report.worstPhase.label,
      count: report.worstPhase.count,
      source: "worstPhase",
    };
  }
  return null;
}

function mergeRecurringPhaseWarnings(samples) {
  const byLabel = new Map();
  for (const sample of samples) {
    for (const phase of sample.renderBudget?.recurringPhaseWarnings || []) {
      const current = byLabel.get(phase.label) || {
        label: phase.label,
        samples: 0,
        maxP95Ms: null,
        maxMs: null,
        severity: "info",
      };
      current.samples += 1;
      current.maxP95Ms = maxMetric(current.maxP95Ms, phase.p95Ms);
      current.maxMs = maxMetric(current.maxMs, phase.maxMs);
      if (phase.severity === "high") current.severity = "high";
      else if (phase.severity === "warn" && current.severity !== "high") current.severity = "warn";
      byLabel.set(phase.label, current);
    }
  }
  return [...byLabel.values()]
    .sort((a, b) => (b.maxP95Ms ?? 0) - (a.maxP95Ms ?? 0) || a.label.localeCompare(b.label))
    .slice(0, MAX_RECURRING_WARNINGS);
}

function summarizeStressMatrixFrameAttribution(samples) {
  const attributions = samples
    .map((sample) => sample.renderBudget?.frameAttribution)
    .filter((attribution) => attribution && typeof attribution === "object");
  if (attributions.length === 0) return null;
  return {
    topLevelAvgMs: averageMetric(attributions.map((item) => item.topLevelAvgMs).filter(Number.isFinite)),
    unattributedAvgMs: averageMetric(attributions.map((item) => item.unattributedAvgMs).filter(Number.isFinite)),
    unattributedP95Ms: averageMetric(attributions.map((item) => item.unattributedP95Ms).filter(Number.isFinite)),
    unattributedMaxMs: maxMetricFromList(attributions.map((item) => item.unattributedMaxMs)),
    rafDispatchAvgMs: averageMetric(attributions.map((item) => item.rafDispatchAvgMs).filter(Number.isFinite)),
    rafDispatchP95Ms: averageMetric(attributions.map((item) => item.rafDispatchP95Ms).filter(Number.isFinite)),
    rafDispatchMaxMs: maxMetricFromList(attributions.map((item) => item.rafDispatchMaxMs)),
    avgTopLevelCoveragePct: averageMetric(attributions.map((item) => item.avgTopLevelCoveragePct).filter(Number.isFinite)),
    sampleCount: attributions.length,
  };
}

function compareStressMatrixFailures(a, b) {
  return (a.nextMissedBudget.fps || 0) - (b.nextMissedBudget.fps || 0)
    || (a.nextMissedBudget.p95MarginMs || 0) - (b.nextMissedBudget.p95MarginMs || 0)
    || (b.frameWork.p95Ms ?? 0) - (a.frameWork.p95Ms ?? 0)
    || a.workloadId.localeCompare(b.workloadId)
    || a.configLabel.localeCompare(b.configLabel);
}

export function formatRenderStressMatrixMarkdown(summary) {
  const lines = [
    "# Render Stress Matrix",
    "",
    `Generated: ${summary.generatedAt}`,
    "",
    "| Rank | Workload | Config | frame.work p95 | Next missed | Top phase | Artifacts |",
    "| --- | --- | --- | ---: | --- | --- | --- |",
  ];
  const rows = summary.rankedFailures?.length ? summary.rankedFailures : summary.cells || [];
  for (const row of rows) {
    const missed = row.nextMissedBudget
      ? `${row.nextMissedBudget.fps} FPS by ${formatMs(Math.abs(row.nextMissedBudget.p95MarginMs))}`
      : "none";
    const rank = row.rank || "";
    const phase = row.topMeasuredPhase?.label || "unknown";
    lines.push(
      `| ${rank} | ${row.workloadId} | ${row.configLabel} | ${formatMs(row.frameWork?.p95Ms)} | ${missed} | ${phase} | ${row.artifactDirs?.length || 0} |`,
    );
  }
  lines.push(
    "",
    "Notes:",
    "",
    "- Budget failures are advisory and machine-local.",
    "- CPU throttling is a Chrome stress control, not a hardware-identical low-end laptop model.",
    "- Inspect each cell's `summary.json` before using the ranking as optimization direction.",
    "",
  );
  return `${lines.join("\n")}\n`;
}

function averageMetric(values) {
  if (!values.length) return null;
  return roundMetric(values.reduce((sum, value) => sum + value, 0) / values.length);
}

function maxMetricFromList(values) {
  const finite = values.filter(Number.isFinite);
  return finite.length > 0 ? roundMetric(Math.max(...finite)) : null;
}

function maxMetric(a, b) {
  if (!Number.isFinite(a)) return Number.isFinite(b) ? b : null;
  if (!Number.isFinite(b)) return a;
  return Math.max(a, b);
}

function serializeMatrixCell(cell) {
  return {
    id: cell.id,
    workloadId: cell.workloadId,
    configLabel: cell.configLabel,
    cpuThrottleRate: cell.cpuThrottleRate,
    viewport: cell.viewport,
    deviceScaleFactor: cell.deviceScaleFactor,
    repeatIndex: cell.repeatIndex,
    repeatCount: cell.repeatCount,
  };
}

function matrixConfigLabel({ cpuThrottleRate, viewport, deviceScaleFactor }) {
  const viewportLabel = viewport.label || `${viewport.width}x${viewport.height}`;
  return `cpu${formatNumberToken(cpuThrottleRate)}-vp${viewportLabel}-dpr${formatNumberToken(deviceScaleFactor)}`;
}

function formatNumberToken(value) {
  return String(value).replace(/\./g, "p");
}

function numberOrNull(value) {
  return Number.isFinite(value) ? value : null;
}

function normalizeCounterRow(counter) {
  return {
    label: typeof counter?.label === "string" ? counter.label : "",
    samples: numberOrZero(counter?.samples),
    frames: numberOrZero(counter?.frames),
    total: numberOrZero(counter?.total),
    maxSample: numberOrZero(counter?.maxSample),
    maxFrame: numberOrZero(counter?.maxFrame),
    avgPerFrame: numberOrZero(counter?.avgPerFrame),
    avgActiveFrame: numberOrZero(counter?.avgActiveFrame),
  };
}

function summarizeDiagnosticGroup(counters, prefixes) {
  const rows = counters
    .filter((counter) => prefixes.some((prefix) => counter.label.startsWith(prefix)))
    .sort(compareDiagnosticCounterRows);
  return {
    total: roundMetric(rows.reduce((sum, counter) => sum + counter.total, 0)),
    counters: rows.slice(0, 8),
  };
}

function compareDiagnosticCounterRows(a, b) {
  return (b.total || 0) - (a.total || 0)
    || (b.maxFrame || 0) - (a.maxFrame || 0)
    || a.label.localeCompare(b.label);
}

function sanitizeLongFrames(frames) {
  if (!Array.isArray(frames)) return [];
  return frames.slice(-8).map((frame) => ({
    at: numberOrNull(frame?.at),
    scheduledAt: numberOrNull(frame?.scheduledAt),
    rafDispatchMs: numberOrNull(frame?.rafDispatchMs),
    frameGapMs: numberOrNull(frame?.frameGapMs),
    frameWorkMs: numberOrNull(frame?.frameWorkMs),
    topLevelPhaseMs: numberOrNull(frame?.topLevelPhaseMs),
    unattributedFrameMs: numberOrNull(frame?.unattributedFrameMs),
    worstPhase: stringOrNull(frame?.worstPhase) || "",
    worstPhaseMs: numberOrNull(frame?.worstPhaseMs),
    topPhase: sanitizePhaseContext(frame?.topPhase),
    rendererNestedPhase: sanitizePhaseContext(frame?.rendererNestedPhase),
    minimapNestedPhase: sanitizePhaseContext(frame?.minimapNestedPhase),
    diagnosticCounters: Array.isArray(frame?.diagnosticCounters)
      ? frame.diagnosticCounters.slice(0, 8).map((counter) => ({
        label: stringOrNull(counter?.label) || "",
        total: numberOrNull(counter?.total),
      }))
      : [],
    context: frame?.context && typeof frame.context === "object" ? frame.context : {},
  }));
}

function sanitizePhaseContext(phase) {
  if (!phase || typeof phase !== "object") return null;
  return {
    label: stringOrNull(phase.label) || "",
    ms: numberOrNull(phase.ms),
  };
}

function stringOrNull(value) {
  return typeof value === "string" ? value : null;
}

function numberOrZero(value) {
  return Number.isFinite(value) ? value : 0;
}

function buildFrameWorkBudgetMargins({ avgMs, p95Ms, maxMs }) {
  return RENDER_FRAME_BUDGET_TARGETS.map((budget) => ({
    fps: budget.fps,
    frameBudgetMs: budget.frameBudgetMs,
    avgMarginMs: marginMs(budget.frameBudgetMs, avgMs),
    avgClears: clearsBudget(budget.frameBudgetMs, avgMs),
    p95MarginMs: marginMs(budget.frameBudgetMs, p95Ms),
    p95Clears: clearsBudget(budget.frameBudgetMs, p95Ms),
    maxMarginMs: marginMs(budget.frameBudgetMs, maxMs),
    maxClears: clearsBudget(budget.frameBudgetMs, maxMs),
  }));
}

function buildFrameAttributionReport(phases, reportSummary, frameWork) {
  const topLevelRows = summarizePhaseGroup(phases, (phase) => phase.label?.startsWith("match."));
  const topLevelAvgMs = roundMetric(topLevelRows.reduce((sum, phase) => sum + numberOrZero(phase.avgMs), 0));
  const unattributedPhase = phaseByLabel(phases, "frame.unattributed");
  const rafDispatchPhase = phaseByLabel(phases, "frame.rafDispatch");
  const fallbackUnattributedAvgMs = Number.isFinite(frameWork?.avgMs) && Number.isFinite(topLevelAvgMs)
    ? Math.max(0, frameWork.avgMs - topLevelAvgMs)
    : null;
  const unattributedAvgMs = numericMetric(unattributedPhase?.avgMs) ?? roundMetric(fallbackUnattributedAvgMs);
  const unattributedP95Ms = numericMetric(unattributedPhase?.p95Ms ?? reportSummary?.frameUnattributedP95Ms);
  const unattributedMaxMs = numericMetric(unattributedPhase?.maxMs ?? reportSummary?.frameUnattributedMaxMs);
  const avgTopLevelCoveragePct = Number.isFinite(frameWork?.avgMs) && frameWork.avgMs > 0 && Number.isFinite(topLevelAvgMs)
    ? roundMetric((topLevelAvgMs / frameWork.avgMs) * 100)
    : null;
  return {
    topLevelAvgMs,
    unattributedAvgMs,
    unattributedP95Ms,
    unattributedMaxMs,
    rafDispatchAvgMs: numericMetric(rafDispatchPhase?.avgMs),
    rafDispatchP95Ms: numericMetric(rafDispatchPhase?.p95Ms ?? reportSummary?.frameRafDispatchP95Ms),
    rafDispatchMaxMs: numericMetric(rafDispatchPhase?.maxMs ?? reportSummary?.frameRafDispatchMaxMs),
    avgTopLevelCoveragePct,
    topLevelPhaseCount: topLevelRows.length,
    topLevelPhases: topLevelRows.slice(0, 8),
    notes: [
      "topLevelAvgMs sums match.* phases only, so nested renderer.* and minimap.* rows are not double-counted.",
      "frame.unattributed is measured per frame by subtracting top-level match.* work from frame.work.",
      "frame.rafDispatch is browser callback dispatch delay before measured frame.work starts.",
    ],
  };
}

function nextMissedFrameWorkBudget(budgets, metric) {
  const clearsKey = `${metric}Clears`;
  const marginKey = `${metric}MarginMs`;
  const missed = budgets.find((budget) => budget[clearsKey] === false);
  if (!missed) return null;
  return {
    fps: missed.fps,
    frameBudgetMs: missed.frameBudgetMs,
    metric,
    [`${metric}MarginMs`]: missed[marginKey],
  };
}

function phaseByLabel(phases, label) {
  return phases.find((phase) => phase?.label === label) || null;
}

function summarizePhaseGroup(phases, predicate) {
  return phases
    .filter((phase) => phase?.label && predicate(phase))
    .map((phase) => ({
      label: phase.label,
      count: numberOrNull(phase.count),
      avgMs: numericMetric(phase.avgMs),
      maxMs: numericMetric(phase.maxMs),
      p50Ms: numericMetric(phase.p50Ms),
      p95Ms: numericMetric(phase.p95Ms),
      slowCount: numberOrNull(phase.slowCount),
    }))
    .sort((a, b) => (b.p95Ms ?? 0) - (a.p95Ms ?? 0) || (b.maxMs ?? 0) - (a.maxMs ?? 0));
}

function numericMetric(value) {
  if (Number.isFinite(value)) return value;
  if (typeof value === "string" && value.startsWith(">")) {
    const parsed = Number(value.slice(1));
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function marginMs(budgetMs, valueMs) {
  if (!Number.isFinite(valueMs)) return null;
  return roundMetric(budgetMs - valueMs);
}

function clearsBudget(budgetMs, valueMs) {
  if (!Number.isFinite(valueMs)) return null;
  return valueMs <= budgetMs;
}

function roundMetric(value) {
  if (!Number.isFinite(value)) return null;
  return Math.round(value * 100) / 100;
}

function formatMs(value) {
  if (!Number.isFinite(value)) return "n/a";
  return `${Math.round(value * 100) / 100}ms`;
}

function formatSignedMs(value) {
  if (!Number.isFinite(value)) return "n/a";
  const rounded = Math.round(value * 100) / 100;
  return `${rounded >= 0 ? "+" : ""}${rounded}ms`;
}

function formatCount(value) {
  if (!Number.isFinite(value)) return "n/a";
  return String(Math.round(value * 10) / 10);
}

function formatPercent(value) {
  if (!Number.isFinite(value)) return "n/a";
  return `${Math.round(value * 10) / 10}%`;
}

async function collectPageSummary(page) {
  return page.evaluate(() => {
    const app = window.__rts || null;
    const match = app?.match || null;
    const health = match?.health || null;
    const healthSnapshot = {
      metrics: health?.metrics?.() || null,
      reportStats: health?.reportStats ? JSON.parse(JSON.stringify(health.reportStats)) : null,
      reportStartedAt: health?.reportStartedAt || null,
    };
    const perfSnapshot = {
      summary: window.__rtsPerf?.summary?.() || null,
      reportSummary: window.__rtsPerf?.reportSummary?.() || null,
    };
    let clientNetReport = null;
    if (match && typeof match.sendNetReport === "function" && match.net) {
      const original = match.net.netReport;
      try {
        match.net.netReport = (report) => {
          clientNetReport = JSON.parse(JSON.stringify(report));
        };
        match.sendNetReport();
      } finally {
        match.net.netReport = original;
      }
    }
    const websocketExtensions = typeof match?.net?.ws?.extensions === "string" ? match.net.ws.extensions : "";
    const websocketCompression = websocketExtensions
      .toLowerCase()
      .split(",")
      .map((part) => part.trim().split(";")[0]?.trim())
      .includes("permessage-deflate")
      ? "permessage-deflate"
      : "none";
    const canvas = document.querySelector("#viewport canvas");
    return {
      title: document.title,
      location: window.location.href,
      userAgent: navigator.userAgent,
      devicePixelRatio: window.devicePixelRatio,
      websocket: {
        extensions: websocketExtensions,
        compression: websocketCompression,
        compressionNegotiated: websocketCompression === "permessage-deflate",
        bufferedAmount: match?.net?.bufferedAmount || 0,
      },
      canvas: canvas ? { width: canvas.width, height: canvas.height } : null,
      health: healthSnapshot,
      perf: perfSnapshot,
      clientNetReport,
    };
  });
}

async function applyWorkloadSetup(page, workload) {
  const setup = workload.setup || null;
  if (!setup) return null;
  const result = { actions: [] };

  if (setup.visionSelectionPlayerIndex != null || setup.visionSelectionPlayerId != null) {
    const action = await page.evaluate((replaySetup) => {
      const match = window.__rts?.match;
      const players = Array.isArray(match?.predictionStartInfo?.players)
        ? match.predictionStartInfo.players
        : [];
      const explicitId = Number(replaySetup.visionSelectionPlayerId);
      const playerFromIndex = players[Number(replaySetup.visionSelectionPlayerIndex)]?.id;
      const playerId = Number.isFinite(explicitId) && explicitId > 0 ? explicitId : Number(playerFromIndex);
      if (!Number.isFinite(playerId) || playerId <= 0 || typeof match?.net?.setVisionSelection !== "function") {
        return { action: "setVisionSelection", error: "vision selection player id or net control unavailable" };
      }
      match.net.setVisionSelection({ mode: "player", playerId });
      return { action: "setVisionSelection", playerId };
    }, setup);
    result.actions.push(action);
    if (action.error) result.error = action.error;
  }

  const speed = Number(setup.setRoomTimeSpeed);
  const seekTick = Number(setup.seekRoomTimeTo);
  if (Number.isInteger(seekTick) && seekTick >= 0) {
    const action = await page.evaluate((tick) => {
      const match = window.__rts?.match;
      if (typeof match?.net?.seekRoomTimeTo !== "function") {
        return { action: "seekRoomTimeTo", targetTick: tick, error: "replay seek control unavailable" };
      }
      match.net.seekRoomTimeTo(tick);
      return { action: "seekRoomTimeTo", targetTick: tick };
    }, seekTick);
    result.actions.push(action);
    if (action.error) {
      result.error = action.error;
    } else {
      try {
        await page.waitForFunction(
          (tick) => (window.__rts?.match?.lastSnapshotTick || 0) >= tick,
          { timeout: Number(setup.seekTimeoutMs) || 20000 },
          seekTick,
        );
      } catch (err) {
        result.error = `timed out waiting for replay seek to tick ${seekTick}: ${err.message}`;
      }
    }
  }

  if (Number.isFinite(speed) && speed >= 0) {
    const action = await setWorkloadRoomTimeSpeed(page, speed);
    result.actions.push(action);
    if (action.error && !result.error) result.error = action.error;
  }

  const waitTick = Number(setup.waitRoomTimeTo);
  if (Number.isInteger(waitTick) && waitTick >= 0) {
    try {
      await page.waitForFunction(
        (tick) => {
          const match = window.__rts?.match;
          const roomTick = Number(match?.roomTimeControls?.roomTimeState?.currentTick);
          const snapshotTick = Number(match?.lastSnapshotTick);
          return Math.max(
            Number.isFinite(roomTick) ? roomTick : 0,
            Number.isFinite(snapshotTick) ? snapshotTick : 0,
          ) >= tick;
        },
        { timeout: Number(setup.roomTimeWaitTimeoutMs) || 20000 },
        waitTick,
      );
      result.actions.push({ action: "waitRoomTimeTo", targetTick: waitTick });
    } catch (err) {
      const message = `timed out waiting for replay room time ${waitTick}: ${err.message}`;
      result.actions.push({ action: "waitRoomTimeTo", targetTick: waitTick, error: message });
      if (!result.error) result.error = message;
    }
  }

  const speedAfterWait = Number(setup.setRoomTimeSpeedAfterWait);
  if (Number.isFinite(speedAfterWait) && speedAfterWait >= 0) {
    const action = await setWorkloadRoomTimeSpeed(page, speedAfterWait);
    result.actions.push({ ...action, action: "setRoomTimeSpeedAfterWait" });
    if (action.error && !result.error) result.error = action.error;
  }

  const minEntities = Number(setup.waitForMinEntities);
  if (Number.isInteger(minEntities) && minEntities > 0) {
    try {
      await page.waitForFunction(
        (minimum) => {
          const state = window.__rts?.match?.state;
          if (!state?._curById?.values) return false;
          let count = 0;
          for (const entity of state._curById.values()) {
            if (entity && !entity.shotReveal && !entity.visionOnly) count += 1;
          }
          return count >= minimum;
        },
        { timeout: Number(setup.entityWaitTimeoutMs) || 12000 },
        minEntities,
      );
      result.actions.push({ action: "waitForMinEntities", minEntities });
    } catch (err) {
      const message = `timed out waiting for ${minEntities} visible entities: ${err.message}`;
      result.actions.push({ action: "waitForMinEntities", minEntities, error: message });
      if (!result.error) result.error = message;
    }
  }

  const selectCount = Number(setup.selectFirstEntities);
  if (!Number.isInteger(selectCount) || selectCount <= 0) {
    return result.actions.length > 0 ? result : { skipped: true };
  }

  try {
    await page.waitForFunction(
      () => {
        const state = window.__rts?.match?.state;
        return !!state?._curById?.size && typeof state.setSelection === "function";
      },
      { timeout: 5000 },
    );
  } catch (err) {
    const action = {
      action: "selectFirstEntities",
      requestedCount: selectCount,
      selectedCount: 0,
      error: `timed out waiting for selectable entities: ${err.message}`,
    };
    result.actions.push(action);
    result.selectedCount = 0;
    if (!result.error) result.error = action.error;
    return result;
  }

  const selection = await page.evaluate((selectionSetup) => {
    const UNIT_KINDS = new Set([
      "worker",
      "rifleman",
      "machine_gunner",
      "anti_tank_gun",
      "mortar_team",
      "artillery",
      "scout_car",
      "tank",
      "command_car",
      "ekat",
    ]);
    const state = window.__rts?.match?.state;
    const players = Array.isArray(window.__rts?.match?.predictionStartInfo?.players)
      ? window.__rts.match.predictionStartInfo.players
      : [];
    const ownerFromVisionSelection = players[Number(selectionSetup.visionSelectionPlayerIndex)]?.id;
    const requestedOwner = Number(selectionSetup.selectOwnerId);
    const ownerId = Number.isFinite(requestedOwner) && requestedOwner > 0
      ? requestedOwner
      : selectionSetup.selectVisionSelectionPlayer
        ? Number(ownerFromVisionSelection)
        : null;
    const entities = Array.from(state?._curById?.values?.() || [])
      .filter((entity) => {
        if (!entity || !Number.isInteger(entity.id) || entity.shotReveal || entity.visionOnly) return false;
        if (Number.isFinite(ownerId) && entity.owner !== ownerId) return false;
        if (selectionSetup.selectUnitKindsOnly && !UNIT_KINDS.has(entity.kind)) return false;
        return true;
      })
      .sort((a, b) => a.id - b.id);
    const selected = entities.slice(0, selectionSetup.selectFirstEntities);
    const selectedIds = selected.map((entity) => entity.id);
    state.setSelection(selectedIds);
    if (selectionSetup.focusSelectedEntities && selected.length > 0 && window.__rts?.match?.camera?.centerOn) {
      const center = selected.reduce(
        (sum, entity) => ({ x: sum.x + Number(entity.x || 0), y: sum.y + Number(entity.y || 0) }),
        { x: 0, y: 0 },
      );
      window.__rts.match.camera.centerOn(center.x / selected.length, center.y / selected.length);
    }
    return {
      action: "selectFirstEntities",
      requestedCount: selectionSetup.selectFirstEntities,
      selectedCount: state.selection?.size || 0,
      selectedIds: Array.from(state.selection || []),
      ownerId: Number.isFinite(ownerId) ? ownerId : null,
      unitKindsOnly: !!selectionSetup.selectUnitKindsOnly,
    };
  }, setup);
  result.actions.push(selection);
  result.selectedCount = selection.selectedCount;
  result.selectedIds = selection.selectedIds;
  return result;
}

async function setWorkloadRoomTimeSpeed(page, speed) {
  return page.evaluate((roomTimeSpeed) => {
    const match = window.__rts?.match;
    if (typeof match?.net?.setRoomTimeSpeed !== "function") {
      return { action: "setRoomTimeSpeed", speed: roomTimeSpeed, error: "room-time speed control unavailable" };
    }
    match.net.setRoomTimeSpeed(roomTimeSpeed);
    return { action: "setRoomTimeSpeed", speed: roomTimeSpeed };
  }, speed);
}

function workloadSetupErrors(workload, setupResult) {
  const minSelected = Number(workload.setup?.minSelectedCount || 0);
  if (!setupResult) return workload.setup ? [`${workload.id} setup did not run`] : [];
  const errors = [];
  if (setupResult.error) errors.push(`${workload.id} setup failed: ${setupResult.error}`);
  if (minSelected && (setupResult.selectedCount || 0) < minSelected) {
    errors.push(`${workload.id} selected ${setupResult.selectedCount || 0}; expected at least ${minSelected}`);
  }
  return errors;
}

async function prepareWorkload(workload) {
  if (workload.kind !== "replayArtifact") return;
  const targetDir = path.join(SERVER_DIR, "target", "selfplay-artifacts", workload.replayName);
  fs.mkdirSync(targetDir, { recursive: true });
  fs.copyFileSync(workload.source, path.join(targetDir, "replay.json"));
}

async function startOrReuseServer(args) {
  const fromEnv = args.baseUrl || process.env.RTS_URL;
  if (fromEnv && await isHealthy(fromEnv)) {
    return {
      baseUrl: normalizeBaseUrl(fromEnv),
      reused: true,
      close: async () => {},
    };
  }

  const port = args.port || await allocatePort();
  const baseUrl = `http://127.0.0.1:${port}/`;
  const targetDir = cargoTargetDir();
  const serverBin = process.env.RTS_SERVER_BIN || path.join(targetDir, "debug", "rts-server");
  if (!fs.existsSync(serverBin)) {
    runOrThrow("cargo", ["build", "--manifest-path", path.join(SERVER_DIR, "Cargo.toml")], {
      cwd: REPO_ROOT,
      env: { ...process.env, CARGO_TARGET_DIR: targetDir },
      stdio: "inherit",
    });
  }
  if (!fs.existsSync(serverBin)) {
    throw new Error(`server binary not found at ${serverBin}`);
  }

  const logPath = path.join(os.tmpdir(), `rts-client-perf-server-${process.pid}.log`);
  const log = fs.openSync(logPath, "w");
  const child = spawn(serverBin, [], {
    cwd: SERVER_DIR,
    env: {
      ...process.env,
      RTS_ADDR: `127.0.0.1:${port}`,
      RTS_TEST_TICK_MS: process.env.RTS_TEST_TICK_MS || "5",
      RTS_MATCH_SEED: process.env.RTS_MATCH_SEED || "1",
    },
    stdio: ["ignore", log, log],
  });
  child.on("exit", () => fs.closeSync(log));

  const deadline = Date.now() + args.serverTimeoutMs;
  while (Date.now() < deadline) {
    if (child.exitCode != null) {
      throw new Error(`server exited during startup; see ${logPath}`);
    }
    if (await isHealthy(baseUrl)) {
      return {
        baseUrl,
        reused: false,
        logPath,
        close: async () => stopChild(child),
      };
    }
    await sleep(250);
  }
  await stopChild(child);
  throw new Error(`server did not become healthy within ${args.serverTimeoutMs}ms; see ${logPath}`);
}

async function launchBrowser(puppeteer, chrome, args) {
  const profileDir = fs.mkdtempSync(path.join(os.tmpdir(), "rts-client-perf-chrome-"));
  return puppeteer.launch({
    executablePath: chrome,
    headless: "new",
    defaultViewport: args.viewport,
    args: [
      "--no-sandbox",
      `--window-size=${args.viewport.width},${args.viewport.height}`,
      `--user-data-dir=${profileDir}`,
    ],
  });
}

async function configurePageEmulation(page, args) {
  const rate = Number(args.cpuThrottleRate || DEFAULT_CPU_THROTTLE_RATE);
  if (!Number.isFinite(rate) || rate <= 1) return null;
  const session = await page.target().createCDPSession();
  await session.send("Emulation.setCPUThrottlingRate", { rate });
  return session;
}

function puppeteerViewport(viewport, deviceScaleFactor = DEFAULT_DEVICE_SCALE_FACTOR) {
  return {
    width: viewport.width,
    height: viewport.height,
    deviceScaleFactor,
  };
}

async function installSnapshotCodecCapture(page, maxSamples) {
  await page.evaluateOnNewDocument((limit) => {
    const NativeWebSocket = window.WebSocket;
    const frames = [];
    window.__rtsSnapshotCodecCapture = { frames, limit };

    class SnapshotCaptureWebSocket extends NativeWebSocket {
      constructor(...args) {
        super(...args);
        this.addEventListener("message", (event) => {
          if (frames.length >= limit) return;
          if (typeof event.data === "string") {
            if (!event.data.startsWith("{\"t\":\"snapshot\"")) return;
            frames.push({ kind: "text", text: event.data });
            return;
          }
          const bytes =
            event.data instanceof ArrayBuffer
              ? new Uint8Array(event.data)
              : ArrayBuffer.isView(event.data)
                ? new Uint8Array(event.data.buffer, event.data.byteOffset, event.data.byteLength)
                : null;
          if (!bytes || bytes.length < 6) return;
          if (bytes[0] !== 0x52 || bytes[1] !== 0x54 || bytes[2] !== 0x53 || bytes[3] !== 0x4d) {
            return;
          }
          frames.push({ kind: "binary", bytes: Array.from(bytes) });
        });
      }
    }

    for (const key of ["CONNECTING", "OPEN", "CLOSING", "CLOSED"]) {
      Object.defineProperty(SnapshotCaptureWebSocket, key, { value: NativeWebSocket[key] });
    }
    window.WebSocket = SnapshotCaptureWebSocket;
  }, maxSamples);
}

async function collectSnapshotCodecFrames(page) {
  return page.evaluate(() => window.__rtsSnapshotCodecCapture?.frames || []);
}

function normalizeCapturedSnapshotCodecFrames(frames) {
  return frames.map((frame, index) => {
    if (typeof frame === "string") return frame;
    if (frame?.kind === "text" && typeof frame.text === "string") return frame.text;
    if (frame?.kind === "binary" && Array.isArray(frame.bytes)) {
      const bytes = Uint8Array.from(frame.bytes);
      if (
        bytes.length < 6 ||
        bytes[0] !== 0x52 ||
        bytes[1] !== 0x54 ||
        bytes[2] !== 0x53 ||
        bytes[3] !== 0x4d
      ) {
        throw new Error(`captured snapshot frame ${index} has an invalid MessagePack header`);
      }
      if (bytes[4] !== 1) {
        throw new Error(`captured snapshot frame ${index} uses unsupported MessagePack version ${bytes[4]}`);
      }
      return JSON.stringify(decodeMessagePack(bytes.subarray(5)));
    }
    throw new Error(`unsupported captured snapshot frame ${index}`);
  });
}

async function loadPuppeteer() {
  ensureTestNodeModules();
  const requireFromTests = createRequire(path.join(TESTS_DIR, "package.json"));
  const resolved = requireFromTests.resolve("puppeteer-core");
  const imported = await import(pathToFileURL(resolved).href);
  return imported.default || imported;
}

function ensureTestNodeModules() {
  const packageLock = path.join(TESTS_DIR, "package-lock.json");
  const packageJson = path.join(TESTS_DIR, "package.json");
  const localNodeModules = path.join(TESTS_DIR, "node_modules");
  const localPuppeteer = path.join(localNodeModules, "puppeteer-core");
  if (fs.existsSync(localPuppeteer)) return;
  const cacheRoot = process.env.RTS_NODE_DEPS_CACHE_DIR || "/tmp/rts-node-deps";
  const hash = crypto.createHash("sha256").update(fs.readFileSync(packageLock)).digest("hex");
  const cacheNodeModules = path.join(cacheRoot, hash, "node_modules");
  if (fs.existsSync(path.join(cacheNodeModules, "puppeteer-core"))) {
    if (fs.existsSync(localNodeModules)) fs.rmSync(localNodeModules, { recursive: true, force: true });
    fs.symlinkSync(cacheNodeModules, localNodeModules, "dir");
    return;
  }
  runOrThrow("npm", ["ci", "--ignore-scripts", "--no-audit", "--fund=false"], {
    cwd: TESTS_DIR,
    stdio: "inherit",
  });
  if (!fs.existsSync(localPuppeteer)) {
    throw new Error(`puppeteer-core was not installed from ${packageJson}`);
  }
}

function selectedWorkloads(args) {
  const ids = args.renderLagSuite ? RENDER_LAG_WORKLOAD_IDS : args.workloads;
  if (ids.length === 0) return WORKLOADS;
  const byId = new Map(WORKLOADS.map((workload) => [workload.id, workload]));
  return ids.map((id) => {
    const workload = byId.get(id);
    if (!workload) throw new Error(`unknown workload ${id}; run --list`);
    return workload;
  });
}

function parseArgs(argv) {
  const args = {
    list: false,
    renderLagSuite: false,
    workloads: [],
    durationMs: DEFAULT_DURATION_MS,
    outputRoot: DEFAULT_OUTPUT_ROOT,
    viewport: { ...DEFAULT_VIEWPORT },
    deviceScaleFactor: DEFAULT_DEVICE_SCALE_FACTOR,
    cpuThrottleRate: DEFAULT_CPU_THROTTLE_RATE,
    stressMatrix: false,
    matrixRepeatCount: DEFAULT_MATRIX_REPEAT_COUNT,
    matrixCpuThrottles: [...DEFAULT_MATRIX_CPU_THROTTLES],
    matrixViewports: [...DEFAULT_MATRIX_VIEWPORTS],
    matrixDeviceScaleFactors: [...DEFAULT_MATRIX_DEVICE_SCALE_FACTORS],
    chrome: process.env.CHROME || "",
    baseUrl: "",
    port: 0,
    trace: false,
    snapshotCodecBakeoff: false,
    snapshotCodecMaxSamples: DEFAULT_CODEC_SAMPLE_LIMIT,
    navTimeoutMs: 15000,
    startTimeoutMs: 20000,
    serverTimeoutMs: 30000,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const value = () => {
      i += 1;
      if (i >= argv.length) throw new Error(`${arg} requires a value`);
      return argv[i];
    };
    if (arg === "--list") args.list = true;
    else if (arg === "--render-lag-suite") args.renderLagSuite = true;
    else if (arg === "--stress-matrix") args.stressMatrix = true;
    else if (arg === "--trace") args.trace = true;
    else if (arg === "--snapshot-codec-bakeoff") args.snapshotCodecBakeoff = true;
    else if (arg === "--snapshot-codec-max-samples") args.snapshotCodecMaxSamples = parsePositiveInt(value(), arg);
    else if (arg.startsWith("--snapshot-codec-max-samples=")) args.snapshotCodecMaxSamples = parsePositiveInt(arg.slice("--snapshot-codec-max-samples=".length), "--snapshot-codec-max-samples");
    else if (arg === "--workload") args.workloads.push(value());
    else if (arg.startsWith("--workload=")) args.workloads.push(arg.slice("--workload=".length));
    else if (arg === "--duration-ms") args.durationMs = parsePositiveInt(value(), arg);
    else if (arg.startsWith("--duration-ms=")) args.durationMs = parsePositiveInt(arg.slice("--duration-ms=".length), "--duration-ms");
    else if (arg === "--seconds") args.durationMs = parsePositiveInt(value(), arg) * 1000;
    else if (arg.startsWith("--seconds=")) args.durationMs = parsePositiveInt(arg.slice("--seconds=".length), "--seconds") * 1000;
    else if (arg === "--output-root") args.outputRoot = value();
    else if (arg.startsWith("--output-root=")) args.outputRoot = arg.slice("--output-root=".length);
    else if (arg === "--chrome") args.chrome = value();
    else if (arg.startsWith("--chrome=")) args.chrome = arg.slice("--chrome=".length);
    else if (arg === "--base-url") args.baseUrl = value();
    else if (arg.startsWith("--base-url=")) args.baseUrl = arg.slice("--base-url=".length);
    else if (arg === "--port") args.port = parsePositiveInt(value(), arg);
    else if (arg.startsWith("--port=")) args.port = parsePositiveInt(arg.slice("--port=".length), "--port");
    else if (arg === "--viewport") args.viewport = parseViewport(value());
    else if (arg.startsWith("--viewport=")) args.viewport = parseViewport(arg.slice("--viewport=".length));
    else if (arg === "--device-scale-factor" || arg === "--dpr") args.deviceScaleFactor = parsePositiveNumber(value(), arg);
    else if (arg.startsWith("--device-scale-factor=")) args.deviceScaleFactor = parsePositiveNumber(arg.slice("--device-scale-factor=".length), "--device-scale-factor");
    else if (arg.startsWith("--dpr=")) args.deviceScaleFactor = parsePositiveNumber(arg.slice("--dpr=".length), "--dpr");
    else if (arg === "--cpu-throttle") args.cpuThrottleRate = parsePositiveNumber(value(), arg);
    else if (arg.startsWith("--cpu-throttle=")) args.cpuThrottleRate = parsePositiveNumber(arg.slice("--cpu-throttle=".length), "--cpu-throttle");
    else if (arg === "--matrix-repeat") args.matrixRepeatCount = parsePositiveInt(value(), arg);
    else if (arg.startsWith("--matrix-repeat=")) args.matrixRepeatCount = parsePositiveInt(arg.slice("--matrix-repeat=".length), "--matrix-repeat");
    else if (arg === "--matrix-cpu") args.matrixCpuThrottles = parsePositiveNumberList(value(), arg);
    else if (arg.startsWith("--matrix-cpu=")) args.matrixCpuThrottles = parsePositiveNumberList(arg.slice("--matrix-cpu=".length), "--matrix-cpu");
    else if (arg === "--matrix-viewport") args.matrixViewports = parseMatrixViewportList(value(), arg);
    else if (arg.startsWith("--matrix-viewport=")) args.matrixViewports = parseMatrixViewportList(arg.slice("--matrix-viewport=".length), "--matrix-viewport");
    else if (arg === "--matrix-dpr") args.matrixDeviceScaleFactors = parsePositiveNumberList(value(), arg);
    else if (arg.startsWith("--matrix-dpr=")) args.matrixDeviceScaleFactors = parsePositiveNumberList(arg.slice("--matrix-dpr=".length), "--matrix-dpr");
    else if (arg === "-h" || arg === "--help") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown arg: ${arg}`);
    }
  }
  if (args.renderLagSuite && args.workloads.length > 0) {
    throw new Error("--render-lag-suite cannot be combined with --workload");
  }
  return args;
}

function printHelp() {
  console.log(`Usage: node scripts/client-perf-harness.mjs [options]

Options:
  --list                         List available workloads.
  --render-lag-suite             Run the full render-lag comparison workload set.
  --stress-matrix                Run workloads across CPU, viewport, DPR, and repeat matrix cells.
  --workload <id>                Run one workload; repeatable. Defaults to all workloads.
  --seconds <n>                  Browser collection time per workload. Default: ${DEFAULT_DURATION_MS / 1000}.
  --duration-ms <n>              Browser collection time per workload in milliseconds.
  --output-root <path>           Artifact root. Default: target/client-perf.
  --trace                        Also write a Chrome trace.json per workload.
  --snapshot-codec-bakeoff       Capture local snapshot frames and write codec bake-off artifacts.
  --snapshot-codec-max-samples <n> Maximum snapshot frames captured per workload. Default: ${DEFAULT_CODEC_SAMPLE_LIMIT}.
  --base-url <url>               Reuse an already-running server when healthy.
  --port <n>                     Port for a harness-started server.
  --chrome <path>                Chrome/Chromium executable. Defaults to CHROME or common paths.
  --viewport <width>x<height>    Browser viewport. Default: ${DEFAULT_VIEWPORT.width}x${DEFAULT_VIEWPORT.height}.
  --dpr <n>                      Device scale factor for a single workload run. Default: ${DEFAULT_DEVICE_SCALE_FACTOR}.
  --cpu-throttle <n>             Chrome CPU throttle factor for a single workload run. Default: ${DEFAULT_CPU_THROTTLE_RATE}.
  --matrix-repeat <n>            Repeat count per stress-matrix cell. Default: ${DEFAULT_MATRIX_REPEAT_COUNT}.
  --matrix-cpu <list>            Comma list such as 1,2,4. Default: ${DEFAULT_MATRIX_CPU_THROTTLES.join(",")}.
  --matrix-viewport <list>       Comma list of small,default,large or WxH. Default: default.
  --matrix-dpr <list>            Comma list of device scale factors. Default: ${DEFAULT_MATRIX_DEVICE_SCALE_FACTORS.join(",")}.
`);
}

function parsePositiveInt(raw, label) {
  const value = Number(raw);
  if (!Number.isInteger(value) || value <= 0) throw new Error(`${label} must be a positive integer`);
  return value;
}

function parsePositiveNumber(raw, label) {
  const value = Number(raw);
  if (!Number.isFinite(value) || value <= 0) throw new Error(`${label} must be a positive number`);
  return value;
}

export function parsePositiveNumberList(raw, label = "value") {
  const values = String(raw)
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => parsePositiveNumber(part, label));
  if (values.length === 0) throw new Error(`${label} must include at least one number`);
  return [...new Set(values)];
}

function parseViewport(raw) {
  const match = /^([1-9][0-9]*)x([1-9][0-9]*)$/.exec(raw);
  if (!match) throw new Error("--viewport must look like 1440x900");
  return { width: Number(match[1]), height: Number(match[2]) };
}

export function parseMatrixViewportList(raw, label = "--matrix-viewport") {
  const viewports = String(raw)
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => {
      const preset = MATRIX_VIEWPORT_PRESETS[part];
      if (preset) return { ...preset };
      const parsed = parseViewport(part);
      return { label: part, ...parsed };
    });
  if (viewports.length === 0) throw new Error(`${label} must include at least one viewport`);
  const byKey = new Map();
  for (const viewport of viewports) {
    byKey.set(`${viewport.width}x${viewport.height}`, viewport);
  }
  return [...byKey.values()];
}

function findChrome(explicit) {
  const candidates = [
    explicit,
    DEFAULT_CHROME,
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    which("google-chrome-stable"),
    which("google-chrome"),
    which("chromium-browser"),
    which("chromium"),
  ].filter(Boolean);
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) return candidate;
  }
  throw new Error("Chrome/Chromium not found; set CHROME=/path/to/chrome or pass --chrome");
}

function which(command) {
  const result = spawnSync("which", [command], { encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : "";
}

function cargoTargetDir() {
  if (process.env.CARGO_TARGET_DIR) return process.env.CARGO_TARGET_DIR;
  const script = path.join(REPO_ROOT, "scripts", "cargo-shared-target.sh");
  const result = spawnSync(script, ["--print-target-dir"], { encoding: "utf8" });
  if (result.status === 0 && result.stdout.trim()) return result.stdout.trim();
  return path.join(SERVER_DIR, "target");
}

async function allocatePort() {
  const net = await import("node:net");
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const port = server.address().port;
      server.close(() => resolve(port));
    });
  });
}

async function isHealthy(baseUrl) {
  try {
    const response = await fetch(normalizeBaseUrl(baseUrl), { signal: AbortSignal.timeout(1500) });
    return response.ok;
  } catch {
    return false;
  }
}

async function fetchText(url) {
  const response = await fetch(url, { signal: AbortSignal.timeout(2500) });
  if (!response.ok) throw new Error(`${url} returned HTTP ${response.status}`);
  return response.text();
}

function normalizeBaseUrl(raw) {
  const url = new URL(raw);
  url.pathname = url.pathname.endsWith("/") ? url.pathname : `${url.pathname}/`;
  return url.href;
}

function runOrThrow(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: "utf8", ...options });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with exit ${result.status}`);
  }
  return result;
}

async function stopChild(child) {
  if (child.exitCode != null) return;
  child.kill("SIGTERM");
  const exited = await Promise.race([
    new Promise((resolve) => child.once("exit", () => resolve(true))),
    sleep(3000).then(() => false),
  ]);
  if (!exited) child.kill("SIGKILL");
}

function timestampForPath(date) {
  return date.toISOString().replace(/[:.]/g, "-");
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((err) => {
    console.error(err.stack || err.message);
    process.exit(1);
  });
}
