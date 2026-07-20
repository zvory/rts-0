import type { KeyInput, Page } from "puppeteer-core";

export interface MouseDragInput {
  button?: "left" | "right";
  from: { x: number; y: number };
  to: { x: number; y: number };
  steps?: number;
  durationMs?: number;
  holdKeys?: Array<"attack" | "shift">;
}

type DragErrorFactory = (
  code: string,
  message: string,
  details?: Record<string, unknown>,
) => Error;

export async function performMouseDrag(
  page: Page,
  {
    button = "left",
    from,
    to,
    steps = 24,
    durationMs = 750,
    holdKeys = [],
  }: MouseDragInput,
  createError: DragErrorFactory,
) {
  const viewport = await page.evaluate(() => {
    const element = document.getElementById("viewport");
    if (!element) return null;
    const rect = element.getBoundingClientRect();
    return { left: rect.left, top: rect.top, width: rect.width, height: rect.height };
  });
  if (!viewport || viewport.width <= 0 || viewport.height <= 0) {
    throw createError("viewportUnavailable", "The rendered game viewport is unavailable for mouse input.");
  }
  for (const [label, point] of [["from", from], ["to", to]] as const) {
    if (point.x < 0 || point.y < 0 || point.x >= viewport.width || point.y >= viewport.height) {
      throw createError("outsideViewport", `drag.${label} must lie inside the current game viewport.`, {
        point,
        viewport: { width: viewport.width, height: viewport.height },
      });
    }
  }

  const keyNames: KeyInput[] = holdKeys.map((key) => key === "attack" ? "a" : "Shift");
  const pressedKeys: KeyInput[] = [];
  let mouseDown = false;
  try {
    await page.mouse.move(viewport.left + from.x, viewport.top + from.y);
    for (const key of keyNames) {
      await page.keyboard.down(key);
      pressedKeys.push(key);
    }
    await page.mouse.down({ button });
    mouseDown = true;
    const frameDelayMs = durationMs / steps;
    for (let index = 1; index <= steps; index += 1) {
      const progress = index / steps;
      await page.mouse.move(
        viewport.left + from.x + (to.x - from.x) * progress,
        viewport.top + from.y + (to.y - from.y) * progress,
      );
      if (frameDelayMs > 0) await sleep(frameDelayMs);
    }
    await page.mouse.up({ button });
    mouseDown = false;
  } finally {
    if (mouseDown) await page.mouse.up({ button }).catch(() => {});
    for (const key of pressedKeys.reverse()) await page.keyboard.up(key).catch(() => {});
  }
  await sleep(50);
  return {
    button,
    from: { ...from },
    to: { ...to },
    steps,
    durationMs,
    holdKeys: [...holdKeys],
    viewport: { width: viewport.width, height: viewport.height },
  };
}

function sleep(ms: number) {
  return new Promise<void>((resolve) => setTimeout(resolve, ms));
}
