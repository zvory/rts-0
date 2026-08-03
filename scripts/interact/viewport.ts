import type { Viewport } from "puppeteer-core";

import { InteractDriverError } from "./driver_error.ts";

export const DEFAULT_VIEWPORT = Object.freeze({ width: 1440, height: 900, deviceScaleFactor: 1 });
export const INTERACT_MEDIA_DPR = Object.freeze({ screenshot: 4, video: 2 });
export const MAX_CAPTURE_VIEWPORT = 2048;

export function normalizeViewport(viewport: { width: number; height: number; deviceScaleFactor?: number; dpr?: number }): Viewport {
  const width = Number(viewport?.width);
  const height = Number(viewport?.height);
  const deviceScaleFactor = Number(viewport?.deviceScaleFactor ?? viewport?.dpr ?? 1);
  if (!Number.isInteger(width) || width < 320 || width > 4096 || !Number.isInteger(height) || height < 240 || height > 4096 || !Number.isFinite(deviceScaleFactor) || deviceScaleFactor <= 0 || deviceScaleFactor > 4) {
    throw new InteractDriverError("invalidViewport", "viewport must have bounded width, height, and DPR.");
  }
  return { width, height, deviceScaleFactor };
}

export function mediaCaptureViewport(requested: Viewport | null | undefined, current: Viewport | null | undefined, defaultDpr: number) {
  const base = requested || current || DEFAULT_VIEWPORT;
  const normalized = normalizeViewport({
    width: base.width,
    height: base.height,
    deviceScaleFactor: requested?.deviceScaleFactor ?? defaultDpr,
  });
  if (normalized.width > MAX_CAPTURE_VIEWPORT || normalized.height > MAX_CAPTURE_VIEWPORT) {
    throw new InteractDriverError("invalidViewport", `capture viewport width and height must be at most ${MAX_CAPTURE_VIEWPORT}.`);
  }
  return normalized;
}
