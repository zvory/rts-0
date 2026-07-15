import fs from "node:fs";

export async function configurePageEmulation(page, rawRate = 1) {
  const rate = Number(rawRate || 1);
  if (!Number.isFinite(rate) || rate <= 1) return null;
  const session = await page.target().createCDPSession();
  await session.send("Emulation.setCPUThrottlingRate", { rate });
  return session;
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
  const interval = Number(rawIntervalUs);
  if (!Number.isInteger(interval) || interval < 100 || interval > 100_000) {
    throw new Error("RTS_CLIENT_CPU_PROFILE_INTERVAL_US must be an integer from 100 through 100000");
  }
  const session = existingSession || await page.target().createCDPSession();
  await session.send("Profiler.enable");
  await session.send("Profiler.setSamplingInterval", { interval });
  await session.send("Profiler.start");
  return { session, started: true };
}

export async function stopCpuProfile(controller, outputPath) {
  if (!controller?.started) return null;
  const { profile } = await controller.session.send("Profiler.stop");
  controller.started = false;
  fs.writeFileSync(outputPath, `${JSON.stringify(profile)}\n`);
  return outputPath;
}

export async function cancelCpuProfile(controller) {
  if (!controller?.started) return;
  await controller.session.send("Profiler.stop").catch(() => {});
  controller.started = false;
}
