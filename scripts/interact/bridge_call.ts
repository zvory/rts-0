import type { Page } from "puppeteer-core";
import { waitForInteractStartup } from "./bridge_startup.ts";

type JsonObject = Record<string, unknown>;
type Timeout = <T>(promise: PromiseLike<T>, timeoutMs: number, detail: string) => Promise<T>;

interface BridgeCallResult {
  ok?: boolean;
  value?: unknown;
  error?: { code?: string; message?: string; details?: JsonObject };
}

const TRANSIENT_SESSION_START_CODES = new Set(["waitingForStart", "waitingForSnapshot"]);

export async function evaluateInteractBridgeCall({
  page, method, input, timeoutMs, startupTimeoutMs, withTimeout,
}: {
  page: Page;
  method: string;
  input: JsonObject;
  timeoutMs: number;
  startupTimeoutMs: number;
  withTimeout: Timeout;
}): Promise<BridgeCallResult> {
  let result = await evaluateOnce(page, method, input, timeoutMs, withTimeout);
  if (method !== "status" && !result?.ok && TRANSIENT_SESSION_START_CODES.has(result?.error?.code || "")) {
    await waitForInteractStartup(page, startupTimeoutMs);
    result = await evaluateOnce(page, method, input, timeoutMs, withTimeout);
  }
  return result;
}

function evaluateOnce(page: Page, method: string, input: JsonObject, timeoutMs: number, withTimeout: Timeout) {
  return withTimeout(
    page.evaluate(
      ({ method: bridgeMethod, input: bridgeInput }) => window.__rtsInteract!.call(bridgeMethod, bridgeInput),
      { method, input },
    ),
    timeoutMs,
    `Interact ${method}`,
  );
}
