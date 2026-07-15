import type { Page } from "puppeteer-core";

export interface InteractStartupStatus {
  ready?: boolean;
  launchError?: string;
  [key: string]: unknown;
}

export async function waitForInteractStartup(
  page: Pick<Page, "evaluate" | "waitForFunction">,
  timeoutMs: number,
): Promise<InteractStartupStatus | null> {
  await page.waitForFunction(
    () => {
      const status = window.__rtsInteract?.status?.();
      return status?.ready === true || !!status?.launchError;
    },
    { timeout: timeoutMs },
  );
  return page.evaluate(() => window.__rtsInteract?.status?.() || null);
}
