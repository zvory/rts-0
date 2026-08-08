export async function installShadowDiagnostics(page, workload) {
  await page.evaluateOnNewDocument((configuration) => {
    window.__rtsPerfWorkloadId = configuration.workloadId;
    window.__rtsGpuShadowTiming = configuration.gpuShadowTimingEnabled;
  }, {
    workloadId: workload.id,
    gpuShadowTimingEnabled: workload.setup?.gpuShadowTimingEnabled === true,
  });
}

export async function applyShadowSetup(page, setup, result) {
  if (setup.gpuShadowTimingEnabled === true) {
    result.actions.push({ action: "gpuShadowTiming", enabled: true });
  }
  if (setup.projectedUnitShadowsEnabled !== true) return;
  const action = await page.evaluate(() => {
    if (typeof window.__rts?.setProjectedUnitShadowsEnabled !== "function") {
      return { action: "projectedUnitShadows", error: "App shadow preference control unavailable" };
    }
    window.__rts.setProjectedUnitShadowsEnabled(true);
    return { action: "projectedUnitShadows", enabled: true };
  });
  result.actions.push(action);
  if (action.error) result.error = action.error;
}
