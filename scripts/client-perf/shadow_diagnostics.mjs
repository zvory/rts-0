export async function installShadowDiagnostics(page, workload) {
  await page.evaluateOnNewDocument((configuration) => {
    window.__rtsPerfWorkloadId = configuration.workloadId;
    window.__rtsGpuShadowTiming = configuration.gpuShadowTimingEnabled;
    window.__rtsGpuCompletePresentations = configuration.gpuCompletePresentationsEnabled;
    window.__rtsCastShadowsEnabled = configuration.castShadowsEnabled;
  }, {
    workloadId: workload.id,
    gpuShadowTimingEnabled: workload.setup?.gpuShadowTimingEnabled === true,
    uncappedPresentationsEnabled: workload.setup?.uncappedPresentationsEnabled === true,
    gpuCompletePresentationsEnabled: workload.setup?.gpuCompletePresentationsEnabled === true,
    castShadowsEnabled: workload.setup?.castShadowsEnabled !== false,
  });
}

export async function applyShadowSetup(page, setup, result) {
  if (setup.gpuShadowTimingEnabled === true) {
    result.actions.push({ action: "gpuShadowTiming", enabled: true });
  }
  if (setup.uncappedPresentationsEnabled === true) {
    result.actions.push({
      action: "uncappedPresentations",
      enabled: true,
      gpuCompletion: setup.gpuCompletePresentationsEnabled === true ? "gl.finish" : "asynchronous",
    });
  }
  result.actions.push({ action: "castShadows", enabled: setup.castShadowsEnabled !== false });
  const action = await page.evaluate((enabled) => {
    if (typeof window.__rts?.setProjectedUnitShadowsEnabled !== "function") {
      return { action: "projectedUnitShadows", error: "App shadow preference control unavailable" };
    }
    window.__rts.setProjectedUnitShadowsEnabled(enabled);
    return { action: "projectedUnitShadows", enabled };
  }, setup.projectedUnitShadowsEnabled === true);
  result.actions.push(action);
  if (action.error) result.error = action.error;
}
