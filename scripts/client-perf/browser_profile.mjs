import fs from "node:fs";

export async function configurePageEmulation(page, rawRate = 1) {
  const rate = Number(rawRate || 1);
  if (!Number.isFinite(rate) || rate <= 1) return null;
  const session = await page.target().createCDPSession();
  try {
    await session.send("Emulation.setCPUThrottlingRate", { rate });
    return session;
  } catch (error) {
    await session.detach?.().catch(() => {});
    throw error;
  }
}

export function puppeteerViewport(viewport, deviceScaleFactor = 1) {
  return {
    width: viewport.width,
    height: viewport.height,
    deviceScaleFactor,
  };
}

export async function startCpuProfile(page, rawIntervalUs, existingSession = null) {
  if (rawIntervalUs == null || rawIntervalUs === "") return null;
  const interval = parseCpuProfileInterval(rawIntervalUs);
  const session = existingSession || await page.target().createCDPSession();
  const ownsSession = existingSession == null;
  try {
    await session.send("Profiler.enable");
    await session.send("Profiler.setSamplingInterval", { interval });
    await session.send("Profiler.start");
    return { session, started: true, ownsSession };
  } catch (error) {
    if (ownsSession) await session.detach?.().catch(() => {});
    throw error;
  }
}

export async function stopCpuProfile(controller, outputPath) {
  if (!controller?.started) return null;
  try {
    const { profile } = await controller.session.send("Profiler.stop");
    controller.started = false;
    fs.writeFileSync(outputPath, `${JSON.stringify(profile)}\n`);
    return outputPath;
  } finally {
    await detachOwnedSession(controller);
  }
}

export async function cancelCpuProfile(controller) {
  if (!controller) return;
  if (controller.started) {
    await controller.session.send("Profiler.stop").catch(() => {});
    controller.started = false;
  }
  await detachOwnedSession(controller);
}

export async function cleanupBrowserProfile({ controller, emulationSession, page }) {
  await cancelCpuProfile(controller);
  await emulationSession?.detach?.().catch(() => {});
  await page?.close?.().catch(() => {});
}

export function parseCpuProfileInterval(raw, label = "CPU profile sampling interval") {
  const value = Number(raw);
  if (!Number.isInteger(value) || value < 100 || value > 100_000) {
    throw new Error(`${label} must be an integer between 100 and 100000`);
  }
  return value;
}

async function detachOwnedSession(controller) {
  if (!controller?.ownsSession) return;
  controller.ownsSession = false;
  await controller.session.detach?.().catch(() => {});
}
