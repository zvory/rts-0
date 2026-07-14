export async function initializeWorkloadSetup(page, setup) {
  if (!setup) return null;
  const result = { actions: [] };
  await applySnapshotStreamSetup(page, setup, result);
  return result;
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
