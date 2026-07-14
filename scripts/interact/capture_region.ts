import type { Page } from "puppeteer-core";

export interface CaptureClip { x: number; y: number; width: number; height: number }
export type CaptureRegion = "viewport" | "minimap" | CaptureClip;

export async function resolveCaptureRegion(page: Page, region: CaptureRegion | null = "viewport") {
  const elements = await page.evaluate(() => {
    const rect = (id: string) => {
      const bounds = document.getElementById(id)?.getBoundingClientRect?.();
      return bounds ? { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height } : null;
    };
    return { viewport: rect("viewport"), minimap: rect("minimap") };
  });
  const viewport = normalizeClip(elements?.viewport, "The game viewport is not available for capture.");
  if (region == null || region === "viewport") return { preset: "viewport", clip: viewport, viewport };
  if (region === "minimap") {
    const minimap = normalizeClip(elements?.minimap, "The minimap is not available for capture.");
    assertInside(minimap, viewport, "The minimap must stay inside the game viewport.");
    return { preset: "minimap", clip: minimap, viewport };
  }
  const relative = normalizeClip(region, "A custom capture region requires finite x/y and width/height of at least 2 pixels.");
  const clip = {
    x: viewport.x + relative.x,
    y: viewport.y + relative.y,
    width: relative.width,
    height: relative.height,
  };
  assertInside(clip, viewport, "A custom capture region must stay inside the game viewport.");
  return { preset: "custom", clip, viewport, relative };
}

function normalizeClip(value: unknown, message: string): CaptureClip {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw regionError(message);
  const input = value as Record<string, unknown>;
  const clip = {
    x: Number(input.x), y: Number(input.y), width: Number(input.width), height: Number(input.height),
  };
  if (!Object.values(clip).every(Number.isFinite) || clip.x < 0 || clip.y < 0 || clip.width < 2 || clip.height < 2 || clip.width > 2048 || clip.height > 2048) {
    throw regionError(message);
  }
  return clip;
}

function assertInside(clip: CaptureClip, viewport: CaptureClip, message: string) {
  const epsilon = 0.5;
  if (clip.x < viewport.x - epsilon || clip.y < viewport.y - epsilon ||
      clip.x + clip.width > viewport.x + viewport.width + epsilon ||
      clip.y + clip.height > viewport.y + viewport.height + epsilon) {
    throw regionError(message);
  }
}

function regionError(message: string) {
  return Object.assign(new Error(message), { code: "invalidRegion" });
}
