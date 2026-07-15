export async function initializeWorkloadSetup(page, setup) {
  if (!setup) return null;
  const result = { actions: [] };
  await applySnapshotStreamSetup(page, setup, result);
  await applyActiveSupplyStressSetup(page, setup, result);
  await applyLiveLabScenarioSetup(page, setup, result);
  return result;
}

export function validateLiveLabScenarioSample(sample, expected) {
  if (!expected) return [];
  const errors = [];
  if (sample?.scenarioId !== expected.scenarioId) errors.push(`scenario ${sample?.scenarioId || "missing"} != ${expected.scenarioId}`);
  if (sample?.mapWidth !== expected.mapWidth || sample?.mapHeight !== expected.mapHeight) {
    errors.push(`map ${sample?.mapWidth}x${sample?.mapHeight} != ${expected.mapWidth}x${expected.mapHeight}`);
  }
  if (sample?.projectedEntityCount !== expected.projectedEntityCount) {
    errors.push(`projected entities ${sample?.projectedEntityCount} != ${expected.projectedEntityCount}`);
  }
  if (sample?.labMode !== true) errors.push("match is not running in Lab mode");
  if (sample?.offline !== false || sample?.websocketOpen !== true) {
    errors.push("live Lab workload does not have an open WebSocket");
  }
  return errors;
}

export function validateSnapshotStreamSample(sample, expected) {
  if (!expected) return [];
  const errors = [];
  if (
    sample?.id !== expected.id ||
    sample?.offline !== true ||
    sample?.netOffline !== true ||
    sample?.serverSimulation !== false ||
    sample?.websocket !== false ||
    sample?.websocketAttached !== false
  ) {
    errors.push("snapshot stream is not isolated from WebSocket/live simulation");
  }
  if (expected.frameCount > 0 && sample?.frameCount !== expected.frameCount) {
    errors.push(`snapshot stream has ${sample?.frameCount || 0} frames; expected ${expected.frameCount}`);
  }
  if (expected.playerId > 0 && sample?.playerId !== expected.playerId) {
    errors.push(`snapshot stream player ${sample?.playerId || 0} != ${expected.playerId}`);
  }
  if (expected.spectator != null && sample?.spectator !== expected.spectator) {
    errors.push(`snapshot stream spectator ${sample?.spectator === true} != ${expected.spectator}`);
  }
  if (
    Array.isArray(expected.teamIds) &&
    JSON.stringify(sample?.teamIds || []) !== JSON.stringify(expected.teamIds)
  ) {
    errors.push("snapshot stream team ids do not match the expected projection");
  }
  if (
    expected.visibilityTileCount > 0 &&
    sample?.visibilityTileCount !== expected.visibilityTileCount
  ) {
    errors.push(
      `snapshot stream visibility grid has ${sample?.visibilityTileCount || 0} tiles; expected ${expected.visibilityTileCount}`,
    );
  }
  return errors;
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

async function applyLiveLabScenarioSetup(page, setup, result) {
  const expected = setup.liveLabScenario;
  if (!expected) return;
  try {
    await page.waitForFunction(
      ({ scenarioId, projectedEntityCount }) => window.__rts?.labLaunch?.scenario === scenarioId &&
        window.__rts?.match?.state?._curById?.size === projectedEntityCount &&
        window.__rts?.net?.offline !== true &&
        window.__rts?.net?.ws?.readyState === WebSocket.OPEN,
      { timeout: Number(setup.liveLabScenarioWaitTimeoutMs) || 20000 },
      expected,
    );
    const action = await page.evaluate(() => {
      const app = window.__rts;
      return {
        action: "verifyLiveLabScenario",
        scenarioId: app?.labLaunch?.scenario || "",
        mapWidth: app?.match?.predictionStartInfo?.map?.width || 0,
        mapHeight: app?.match?.predictionStartInfo?.map?.height || 0,
        projectedEntityCount: app?.match?.state?._curById?.size || 0,
        labMode: !!app?.match?.labMetadata,
        offline: app?.net?.offline === true,
        websocketOpen: app?.net?.ws?.readyState === WebSocket.OPEN,
      };
    });
    const errors = validateLiveLabScenarioSample(action, expected);
    if (errors.length > 0) action.error = errors.join("; ");
    result.actions.push(action);
    result.liveLabScenario = action;
    if (action.error) result.error = action.error;
  } catch (err) {
    const message = `live Lab scenario setup failed: ${err.message}`;
    result.actions.push({ action: "verifyLiveLabScenario", error: message });
    result.error = message;
  }
}

async function applySnapshotStreamSetup(page, setup, result) {
  if (!setup.snapshotStreamId) return;

  try {
    const expected = {
      id: setup.snapshotStreamId,
      frameCount: Number(setup.snapshotStreamFrameCount) || 0,
      playerId: Number(setup.snapshotStreamPlayerId) || 0,
      spectator: typeof setup.snapshotStreamSpectator === "boolean"
        ? setup.snapshotStreamSpectator
        : null,
      teamIds: setup.snapshotStreamTeamIds || null,
      visibilityTileCount: Number(setup.snapshotStreamVisibilityTileCount) || 0,
    };
    await page.waitForFunction(
      (id) => window.__rtsSnapshotStream?.id === id &&
        window.__rtsSnapshotStream?.frameCount > 0,
      { timeout: Number(setup.snapshotStreamWaitTimeoutMs) || 12000 },
      expected.id,
    );
    if (expected.visibilityTileCount > 0) {
      await page.waitForFunction(
        (count) => window.__rts?.match?.state?.visibleTiles?.length === count,
        { timeout: Number(setup.snapshotStreamWaitTimeoutMs) || 12000 },
        expected.visibilityTileCount,
      );
    }
    const action = await page.evaluate(() => {
      const stream = window.__rtsSnapshotStream;
      const net = window.__rts?.net;
      const state = window.__rts?.match?.state;
      const frameCount = Number(stream?.frameCount) || 0;
      const playerId = Number(state?.playerId) || 0;
      const spectator = state?.spectator === true;
      const teamIds = Array.isArray(state?.players)
        ? state.players.map((player) => Number(player?.teamId) || 0)
        : [];
      const visibilityTileCount = Number(state?.visibleTiles?.length) || 0;
      return {
        action: "verifySnapshotStreamIsolation",
        id: stream?.id || "",
        frameCount,
        tickRateHz: Number(stream?.tickRateHz) || 0,
        offline: stream?.offline,
        netOffline: net?.offline,
        serverSimulation: stream?.serverSimulation,
        websocket: stream?.websocket,
        websocketAttached: net?.ws != null,
        playerId,
        spectator,
        teamIds,
        visibilityTileCount,
      };
    });
    action.expectedFrameCount = expected.frameCount || undefined;
    const errors = validateSnapshotStreamSample(action, expected);
    if (errors.length > 0) action.error = errors.join("; ");
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
