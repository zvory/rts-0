import { runMatchCaptureFrame } from "./frame_recovery.js";
import { CaptureRenderClock, RenderClock } from "./visual_clock.js";

export function createMatchRenderClock() {
  return { renderClock: new RenderClock(), captureClock: null, captureRafWasRunning: false };
}

export function enterFixedCapture(match) {
  if (match.captureClock) throw new Error("Fixed capture is already active.");
  match.captureRafWasRunning = match.running && match.rafId !== undefined;
  if (match.rafId !== undefined) cancelAnimationFrame(match.rafId);
  match.rafId = undefined;
  match.captureClock = new CaptureRenderClock(match.renderClock.now());
  match.renderClock = match.captureClock;
  match.renderer.setRenderClock(match.captureClock);
  match.lastFrame = match.captureClock.now();
  return { visualStartMs: match.captureClock.now() };
}

export function renderFixedCaptureFrame(match, visualTimeMs) {
  if (!match.captureClock) throw new Error("Fixed capture is not active.");
  match.captureClock.advanceTo(visualTimeMs);
  runMatchCaptureFrame(match, visualTimeMs);
  return { visualTimeMs, tick: match.state.tick, rendererFrame: match.renderer._renderFrameCount };
}

export function exitFixedCapture(match) {
  if (!match.captureClock) return { resumed: false };
  const resumed = match.captureRafWasRunning && match.running;
  match.renderClock = new RenderClock();
  match.renderer.setRenderClock(match.renderClock);
  match.captureClock = null;
  match.lastFrame = match.renderClock.now();
  match.captureRafWasRunning = false;
  if (resumed) match.rafId = requestAnimationFrame(match.tickFn);
  return { resumed };
}
