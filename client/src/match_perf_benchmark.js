import { runMatchBenchmarkFrame } from "./frame_recovery.js";

export function beginUncappedPerfBenchmark(match) {
  if (match.uncappedPerfBenchmark) throw new Error("Uncapped performance benchmark is already active.");
  if (match.captureClock) throw new Error("Uncapped performance benchmark cannot run during fixed capture.");
  const resumeRaf = match.running && match.rafId !== undefined;
  if (match.rafId !== undefined) cancelAnimationFrame(match.rafId);
  match.rafId = undefined;
  match.uncappedPerfBenchmark = { resumeRaf };
  match.lastFrame = performance.now();
  return { resumeRaf };
}

export async function renderUncappedPerfFrame(match) {
  if (!match.uncappedPerfBenchmark) throw new Error("Uncapped performance benchmark is not active.");
  const outcome = await runMatchBenchmarkFrame(match, performance.now());
  if (outcome?.status !== "presented") {
    const detail = outcome?.error?.message || outcome?.status || "missing outcome";
    throw new Error(`Uncapped benchmark frame was not presented: ${detail}.`);
  }
  return outcome;
}

export function endUncappedPerfBenchmark(match) {
  const state = match.uncappedPerfBenchmark;
  if (!state) return { resumed: false };
  match.uncappedPerfBenchmark = null;
  match.lastFrame = performance.now();
  const resumed = state.resumeRaf && match.running;
  if (resumed) match.rafId = requestAnimationFrame(match.tickFn);
  return { resumed };
}
