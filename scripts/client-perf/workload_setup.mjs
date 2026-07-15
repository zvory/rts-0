import { applyLabHellholeSetup, validateLabHellholeSample } from "./lab_hellhole_setup.mjs";

export async function initializeWorkloadSetup(page, setup) {
  if (!setup) return null;
  const result = { actions: [] };
  await applySnapshotStreamSetup(page, setup, result);
  await applyLabHellholeSetup(page, setup, result);
  await applyActiveSupplyStressSetup(page, setup, result);
  return result;
}

export function validateActiveSupplyStressSample(sample, expected) {
  if (!expected) return [];
  const errors = [];
  if (sample?.source !== "server-authoritative-dev-scenario" || sample?.clientMutated !== false) {
    errors.push("setup was not the unmodified server-authoritative dev scenario");
  }
  if (sample?.scenarioId !== expected.scenarioId) errors.push(`scenario ${sample?.scenarioId || "missing"} != ${expected.scenarioId}`);
  if (sample?.scenarioSeed !== expected.scenarioSeed) errors.push(`scenario seed ${sample?.scenarioSeed} != ${expected.scenarioSeed}`);
  if (sample?.playerId !== expected.playerId) errors.push(`player ${sample?.playerId} != ${expected.playerId}`);
  if (sample?.spectator !== false) errors.push("measured browser is a spectator");
  if (expected.predictionRequired && (
    sample?.predictionEnabled !== true || sample?.predictionReady !== true || sample?.predictionMode === "disabled"
  )) {
    errors.push("compatible active prediction is not enabled and ready");
  }
  if (sample?.supplyUsed !== expected.targetSupply) errors.push(`supply ${sample?.supplyUsed} != ${expected.targetSupply}`);
  if (sample?.supplyCap !== expected.supplyCap) errors.push(`production supply cap ${sample?.supplyCap} != ${expected.supplyCap}`);
  if (sample?.projectedEntityCount !== expected.projectedEntityCount) {
    errors.push(`projected entities ${sample?.projectedEntityCount} != ${expected.projectedEntityCount}`);
  }
  if (JSON.stringify(sample?.countsByOwner || {}) !== JSON.stringify(expected.countsByOwner || {})) {
    errors.push("per-owner/per-kind composition differs from the workload descriptor");
  }
  return errors;
}

export function labHellholeSampleErrors(setup, setupResult, summary) {
  if (!setup?.labHellhole || !setupResult?.labHellhole) return [];
  return validateLabHellholeSample({
    setupResult: setupResult.labHellhole,
    monitor: summary?.labHellholeMonitor,
    finalFrameCount: summary?.perf?.summary?.frameCount,
  }, setup.labHellhole);
}

async function applyActiveSupplyStressSetup(page, setup, result) {
  const expected = setup.activeSupplyStress;
  if (!expected) return;
  try {
    await page.waitForFunction(
      ({ targetSupply, projectedEntityCount }) => {
        const match = window.__rts?.match;
        return match?.state?.resources?.supplyUsed === targetSupply
          && match?.state?._curById?.size === projectedEntityCount
          && match?.predictionAdapter?.diagnostics?.()?.ready === true;
      },
      { timeout: Number(setup.activeSupplyWaitTimeoutMs) || 30000 },
      expected,
    );
    const action = await page.evaluate((descriptor) => {
      const match = window.__rts?.match;
      const state = match?.state;
      const unitKinds = new Set(Object.keys(descriptor.countsByOwner?.[1] || {}));
      const countsByOwner = {};
      let projectedEntityCount = 0;
      for (const entity of state?._curById?.values?.() || []) {
        if (!entity || entity.shotReveal || entity.visionOnly) continue;
        projectedEntityCount += 1;
        if (!unitKinds.has(entity.kind)) continue;
        const owner = String(entity.owner);
        countsByOwner[owner] ||= {};
        countsByOwner[owner][entity.kind] = (countsByOwner[owner][entity.kind] || 0) + 1;
      }
      const map = state?.map;
      match?.camera?.fitWorldPoints?.([
        { x: 0, y: 0 },
        { x: Number(map?.width || 0) * Number(map?.tileSize || 0), y: Number(map?.height || 0) * Number(map?.tileSize || 0) },
      ]);
      const prediction = match?.prediction?.debugSummary?.() || {};
      const wasm = match?.predictionAdapter?.diagnostics?.() || {};
      return {
        action: "verifyActiveSupplyStress",
        source: "server-authoritative-dev-scenario",
        clientMutated: false,
        scenarioId: match?.devWatch?.id || "",
        scenarioSeed: descriptor.scenarioSeed,
        targetSupply: descriptor.targetSupply,
        playerId: state?.playerId,
        spectator: state?.spectator,
        predictionEnabled: prediction.enabled === true,
        predictionReady: wasm.ready === true,
        predictionMode: prediction.mode || "disabled",
        supplyUsed: state?.resources?.supplyUsed,
        supplyCap: state?.resources?.supplyCap,
        projectedEntityCount,
        countsByOwner,
        wholeMapCamera: true,
        rendererFrame: Number(match?.renderer?._renderFrameCount || 0),
      };
    }, expected);
    const errors = validateActiveSupplyStressSample(action, expected);
    if (errors.length > 0) action.error = errors.join("; ");
    result.actions.push(action);
    result.activeSupplyStress = action;
    if (action.error) {
      result.error = action.error;
      return;
    }
    await page.waitForFunction(
      (rendererFrame) => Number(window.__rts?.match?.renderer?._renderFrameCount || 0) >= rendererFrame + 2,
      { timeout: 5000 },
      action.rendererFrame,
    );
    action.explicitFramesAfterAssertions = 2;
  } catch (err) {
    const message = `active supply-stress setup failed: ${err.message}`;
    result.actions.push({ action: "verifyActiveSupplyStress", error: message });
    result.error = message;
  }
}

async function applySnapshotStreamSetup(page, setup, result) {
  if (!setup.snapshotStreamId) return;

  try {
    const expectedFrameCount = Number(setup.snapshotStreamFrameCount) || 0;
    await page.waitForFunction(
      (id) => window.__rtsSnapshotStream?.id === id &&
        window.__rtsSnapshotStream?.frameCount > 0,
      { timeout: Number(setup.snapshotStreamWaitTimeoutMs) || 12000 },
      setup.snapshotStreamId,
    );
    const action = await page.evaluate(({ id, expectedFrameCount }) => {
      const stream = window.__rtsSnapshotStream;
      const net = window.__rts?.net;
      const frameCount = Number(stream?.frameCount) || 0;
      const isolated = stream?.id === id && stream?.offline === true &&
        stream?.serverSimulation === false && stream?.websocket === false &&
        net?.offline === true && net?.ws == null;
      const expectedArtifact = expectedFrameCount <= 0 || frameCount === expectedFrameCount;
      let error;
      if (!isolated) {
        error = "snapshot stream is not isolated from WebSocket/live simulation";
      } else if (!expectedArtifact) {
        error = `snapshot stream has ${frameCount} frames; expected ${expectedFrameCount}`;
      }
      return {
        action: "verifySnapshotStreamIsolation",
        id,
        frameCount,
        expectedFrameCount: expectedFrameCount || undefined,
        tickRateHz: Number(stream?.tickRateHz) || 0,
        offline: !!stream?.offline,
        serverSimulation: !!stream?.serverSimulation,
        websocket: !!stream?.websocket,
        error,
      };
    }, { id: setup.snapshotStreamId, expectedFrameCount });
    result.actions.push(action);
    if (action.error) result.error = action.error;
  } catch (err) {
    const message = `timed out waiting for offline snapshot stream ${setup.snapshotStreamId}: ${err.message}`;
    result.actions.push({
      action: "verifySnapshotStreamIsolation",
      id: setup.snapshotStreamId,
      error: message,
    });
    result.error = message;
  }
}
