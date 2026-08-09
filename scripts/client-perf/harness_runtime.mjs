import path from "node:path";
import {
  cancelCpuProfile,
  startCpuProfile,
  stopCpuProfile,
} from "./browser_profile.mjs";

export async function resetPerfDiagnostics(page) {
  await page.evaluate(() => {
    window.__rtsPerf?.reset?.();
    window.__rtsRenderWorkerControl?.reset?.();
  });
}

export async function beginUncappedPresentations(page, warmupFrames = 30) {
  return page.evaluate(async (count) => {
    const match = window.__rts?.match;
    if (!match) throw new Error("Uncapped Match performance controller is unavailable.");
    const controller = await import("/src/match_perf_benchmark.js");
    window.__rtsUncappedPerfController = controller;
    controller.beginUncappedPerfBenchmark(match);
    const carried = await controller.renderUncappedPerfFrame(match);
    if (carried?.status !== "presented") throw new Error(`Uncapped drain frame ended as ${carried?.status || "missing"}.`);
    const outcomes = await runCompletionPacedFrames(match, controller, Math.max(2, Number(count) || 30));
    return summarizeOutcomes(outcomes);

    async function runCompletionPacedFrames(activeMatch, activeController, total) {
      const active = new Set();
      const outcomes = [];
      let submitted = 0;
      const submit = () => {
        submitted += 1;
        const pending = Promise.resolve(activeController.renderUncappedPerfFrame(activeMatch))
          .then((outcome) => ({ outcome, completedAtMs: performance.now() }));
        active.add(pending);
        pending.then(() => active.delete(pending), () => active.delete(pending));
      };
      while (submitted < Math.min(2, total)) submit();
      while (outcomes.length < total) {
        const completed = await Promise.race(active);
        outcomes.push(completed);
        if (submitted < total) submit();
      }
      return outcomes;
    }

    function summarizeOutcomes(outcomes) {
      return {
        submitted: outcomes.length,
        completed: outcomes.filter(({ outcome }) => outcome?.status === "presented").length,
        failed: outcomes.filter(({ outcome }) => outcome?.status !== "presented").length,
      };
    }
  }, warmupFrames);
}

export async function preparePresentationWindow(page, uncapped, warmupFrames = 30) {
  if (uncapped) {
    const warmup = await beginUncappedPresentations(page, warmupFrames);
    if (warmup.failed !== 0 || warmup.completed !== warmup.submitted) {
      throw new Error(`Uncapped warmup failed completion accounting: ${JSON.stringify(warmup)}`);
    }
  }
  await resetPerfDiagnostics(page);
}

export async function runUncappedPresentationWindow(page, durationMs) {
  return page.evaluate(async (requestedDurationMs) => {
    const match = window.__rts?.match;
    const controller = window.__rtsUncappedPerfController;
    if (!match?.uncappedPerfBenchmark || typeof controller?.renderUncappedPerfFrame !== "function") {
      throw new Error("Uncapped Match performance controller is not active.");
    }
    const startMs = performance.now();
    const startTick = Number(match.state?.tick || 0);
    const deadlineMs = startMs + Math.max(100, Number(requestedDurationMs) || 1000);
    const active = new Set();
    const outcomes = [];
    let submitted = 0;
    const submit = () => {
      submitted += 1;
      const pending = Promise.resolve(controller.renderUncappedPerfFrame(match))
        .then((outcome) => ({ outcome, completedAtMs: performance.now() }));
      active.add(pending);
      pending.then(() => active.delete(pending), () => active.delete(pending));
    };
    submit();
    submit();
    while (active.size > 0) {
      const completed = await Promise.race(active);
      outcomes.push(completed);
      if (completed.completedAtMs < deadlineMs) submit();
      if (performance.now() >= deadlineMs) break;
    }
    const drain = await Promise.all(active);
    outcomes.push(...drain);
    const endedAtMs = performance.now();
    const withinWindow = outcomes.filter(({ completedAtMs }) => completedAtMs <= deadlineMs);
    const presentedWithinWindow = withinWindow.filter(({ outcome }) => outcome?.status === "presented");
    const failed = outcomes.filter(({ outcome }) => outcome?.status !== "presented");
    return {
      source: window.__rtsGpuCompletePresentations === true
        ? "match.liveFrame.gpuComplete"
        : "match.liveFrame.workerAcknowledged",
      requestedDurationMs,
      windowDurationMs: deadlineMs - startMs,
      totalElapsedMs: endedAtMs - startMs,
      startTick,
      endTick: Number(match.state?.tick || 0),
      submitted,
      completed: presentedWithinWindow.length,
      completedPerSecond: Math.round((presentedWithinWindow.length / ((deadlineMs - startMs) / 1000)) * 100) / 100,
      drainedAfterDeadline: outcomes.length - withinWindow.length,
      superseded: 0,
      failed: failed.length,
      pipelineDepth: 2,
      gpuCompletion: window.__rtsGpuCompletePresentations === true ? "gl.finish" : "asynchronous",
    };
  }, durationMs);
}

export async function measurePresentationWindow(page, durationMs, uncapped) {
  if (uncapped) return runUncappedPresentationWindow(page, durationMs);
  await new Promise((resolve) => setTimeout(resolve, durationMs));
  return null;
}

export function uncappedPresentationErrors(measurement, worker = {}) {
  if (!measurement) return [];
  const accountingMatches = worker.submitted === measurement.submitted
    && worker.dispatched === measurement.submitted
    && worker.presented === measurement.submitted
    && worker.completed === measurement.submitted;
  const drainedCleanly = worker.superseded === 0 && worker.failed === 0 && worker.staleResponses === 0
    && worker.carriedInFlight === 0 && worker.carriedCompleted === 0 && !worker.inFlight && !worker.pending;
  const windowClean = measurement.failed === 0 && measurement.drainedAfterDeadline <= measurement.pipelineDepth;
  return [
    ...(!accountingMatches ? ["Uncapped presentation accounting did not drain every submitted worker frame"] : []),
    ...(!drainedCleanly ? ["Uncapped presentation window superseded, failed, or left worker frames pending"] : []),
    ...(!windowClean ? ["Uncapped presentation window failed or drained more work than its bounded pipeline depth"] : []),
  ];
}

export async function endUncappedPresentations(page) {
  return page.evaluate(() => {
    const controller = window.__rtsUncappedPerfController;
    const match = window.__rts?.match;
    const result = controller?.endUncappedPerfBenchmark?.(match) || { resumed: false };
    delete window.__rtsUncappedPerfController;
    return result;
  });
}

export async function startRenderWorkerProfile(page, intervalUs) {
  if (intervalUs == null) return null;
  const worker = page.workers().find((candidate) => {
    try {
      return new URL(candidate.url()).pathname.endsWith("/src/renderer/pixi_render_worker.js");
    } catch {
      return false;
    }
  });
  return worker ? startCpuProfile(page, intervalUs, worker.client) : null;
}

export async function stopRenderWorkerProfile(controller, artifactDir) {
  const outputPath = controller ? path.join(artifactDir, "render-worker-cpu-profile.cpuprofile") : null;
  await stopCpuProfile(controller, outputPath);
  return outputPath;
}

export function cancelRenderWorkerProfile(controller) {
  return cancelCpuProfile(controller);
}

export function buildPresentationMetrics(summary, durationMs) {
  const seconds = Math.max(0.001, Number(durationMs) / 1000);
  const worker = summary?.renderWorker;
  const completed = worker?.mode === "pixi-webgl-module-worker"
    ? Number(worker.completed || 0)
    : Number(summary?.perf?.summary?.frameCount || 0);
  return {
    source: worker?.mode === "pixi-webgl-module-worker" ? "renderWorker.completed" : "frame.work",
    completed,
    completedPerSecond: Math.round((completed / seconds) * 100) / 100,
    submitted: Number(worker?.submitted ?? completed),
    superseded: Number(worker?.superseded || 0),
    failed: Number(worker?.failed || 0),
    displayAgeMs: worker?.displayAgeMs || phaseTiming(summary, "match.renderer"),
    queueAgeMs: worker?.queueAgeMs || null,
    mainSubmitMs: worker?.mainSubmitMs || null,
    workerUpdateMs: worker?.workerUpdateMs || null,
    workerPresentMs: worker?.workerPresentMs || null,
  };
}

export function renderWorkerErrors(worker) {
  if (worker?.mode !== "pixi-webgl-module-worker") return [];
  const errors = [];
  if (worker.backendInfo?.backend !== "webgl") errors.push("Pixi render worker did not report WebGL");
  if (worker.failed !== 0) errors.push(`Pixi render worker reported ${worker.failed} failed frames`);
  return errors;
}

export function presentationConsoleLine(presentations) {
  if (!presentations) return null;
  return `presentations ${presentations.completedPerSecond.toFixed(2)}/s completed=${presentations.completed} `
    + `superseded=${presentations.superseded} failed=${presentations.failed}`;
}

export function workloadTimeoutScale(args, defaultRate = 1) {
  const rate = Number(args?.cpuThrottleRate || defaultRate);
  return Number.isFinite(rate) && rate > 1 ? rate : 1;
}

export function scaledTimeoutMs(timeoutMs, scale) {
  const timeout = Number(timeoutMs);
  const factor = Number(scale);
  if (!Number.isFinite(timeout) || timeout <= 0) return timeoutMs;
  if (!Number.isFinite(factor) || factor <= 1) return timeout;
  return Math.ceil(timeout * factor);
}

export function snapshotPacketBudgetSummary(report) {
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

function phaseTiming(summary, label) {
  const phase = summary?.perf?.summary?.phases?.find((candidate) => candidate?.label === label);
  if (!phase) return null;
  return {
    count: numberOrNull(phase.count),
    avg: numberOrNull(phase.avgMs),
    p95: numberOrNull(phase.p95Ms),
    max: numberOrNull(phase.maxMs),
  };
}

function numberOrNull(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function stringOrNull(value) {
  return typeof value === "string" ? value : null;
}
