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
