function sortedNumbers(values) {
  return [...(values || [])].map(Number).sort((a, b) => a - b);
}

function stableJson(value) {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (!value || typeof value !== "object") return JSON.stringify(value);
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`).join(",")}}`;
}

export function validateLabHellholeFacts(facts, descriptor) {
  const errors = [];
  if (facts?.scenarioId !== descriptor.scenarioId) {
    errors.push(`scenario is ${facts?.scenarioId || "missing"}; expected ${descriptor.scenarioId}`);
  }
  if (facts?.map !== descriptor.map) {
    errors.push(`map is ${facts?.map || "missing"}; expected ${descriptor.map}`);
  }
  if (!facts?.labMode || facts?.visionMode !== "all") {
    errors.push("room is not a full-world Lab projection");
  }
  if (stableJson(sortedNumbers(facts?.playerIds)) !== stableJson(sortedNumbers(descriptor.playerIds))) {
    errors.push(`players ${stableJson(facts?.playerIds)} do not match ${stableJson(descriptor.playerIds)}`);
  }
  if (stableJson(sortedNumbers(facts?.godModePlayers)) !== stableJson(sortedNumbers(descriptor.godModePlayers))) {
    errors.push(`god mode ${stableJson(facts?.godModePlayers)} does not match ${stableJson(descriptor.godModePlayers)}`);
  }
  for (const playerId of descriptor.playerIds) {
    const supply = Number(facts?.supplyByOwner?.[playerId]);
    if (supply !== descriptor.supplyUsed) {
      errors.push(`player ${playerId} supply is ${supply}; expected ${descriptor.supplyUsed}`);
    }
    const actualCounts = facts?.countsByOwner?.[playerId] || {};
    const expectedCounts = descriptor.countsByOwner[playerId] || {};
    if (stableJson(actualCounts) !== stableJson(expectedCounts)) {
      errors.push(`player ${playerId} unit counts do not match the checked-in descriptor`);
    }
    for (const kind of descriptor.requiredUnitKinds) {
      if (!(Number(actualCounts[kind]) > 0)) {
        errors.push(`player ${playerId} is missing required ${kind}`);
      }
    }
  }
  if (facts?.entityCount !== descriptor.projectedEntityCount) {
    errors.push(`projected entity count is ${facts?.entityCount}; expected ${descriptor.projectedEntityCount}`);
  }
  if (facts?.snapshotCodec !== "messagepack-compact" || facts?.snapshotFrameKind !== "binary") {
    errors.push(`snapshot codec is ${facts?.snapshotCodec || "missing"}/${facts?.snapshotFrameKind || "missing"}`);
  }
  if (!(facts?.snapshotMessageCount >= 1)) {
    errors.push("no MessagePack snapshot arrived before setup");
  }
  return errors;
}

export function validateLabHellholeSample({ setupResult, monitor, finalFrameCount }, descriptor) {
  const errors = [];
  const renderedFrames = Number(finalFrameCount) || 0;
  if (renderedFrames < 2) errors.push(`only ${Math.max(0, renderedFrames)} rendered frames arrived after setup`);
  if (!(monitor?.snapshotCount >= 2)) errors.push("fewer than two authoritative snapshots arrived during sampling");
  if (monitor?.minEntityCount !== descriptor.projectedEntityCount ||
      monitor?.maxEntityCount !== descriptor.projectedEntityCount) {
    errors.push(`entity count changed during sampling (${monitor?.minEntityCount ?? "n/a"}-${monitor?.maxEntityCount ?? "n/a"})`);
  }
  if (!(monitor?.combatSnapshotCount > 0) || !(monitor?.attackEventCount > 0)) {
    errors.push("room produced no combat snapshots during sampling");
  }
  if (!(monitor?.lastCombatTick >= monitor?.lastSnapshotTick - 90)) {
    errors.push("combat went quiet before the end of the sample window");
  }
  return errors;
}

export async function applyLabHellholeSetup(page, setup, result) {
  const descriptor = setup?.labHellhole;
  if (!descriptor) return;
  try {
    await page.waitForFunction(
      (expectedPlayers) => {
        const match = window.__rts?.match;
        return !!match?.labMetadata &&
          match?.state?._curById?.size > 0 &&
          match.state.playerResources?.length === expectedPlayers;
      },
      { timeout: Number(setup.labHellholeWaitTimeoutMs) || 20000 },
      descriptor.playerIds.length,
    );
    const action = await page.evaluate((expected) => {
      const match = window.__rts?.match;
      const state = match?.state;
      const countsByOwner = {};
      for (const entity of state?._curById?.values?.() || []) {
        if (!expected.playerIds.includes(entity.owner) ||
            !expected.requiredUnitKinds.includes(entity.kind)) continue;
        const counts = countsByOwner[entity.owner] ||= {};
        counts[entity.kind] = (counts[entity.kind] || 0) + 1;
      }
      const supplyByOwner = Object.fromEntries(
        (state?.playerResources || []).map((player) => [player.id, player.supplyUsed]),
      );
      const stats = match?.net?.snapshotReportStats || {};
      const initialRenderedFrames = Number(window.__rtsPerf?.summary?.()?.frameCount) || 0;
      const monitor = {
        snapshotCount: 0,
        combatSnapshotCount: 0,
        attackEventCount: 0,
        projectileEventCount: 0,
        minEntityCount: Number.POSITIVE_INFINITY,
        maxEntityCount: 0,
        lastSnapshotTick: 0,
        lastCombatTick: 0,
      };
      match.net.on("snapshot", (snapshot) => {
        const entityCount = Array.isArray(snapshot?.entities) ? snapshot.entities.length : 0;
        const events = Array.isArray(snapshot?.events) ? snapshot.events : [];
        const attackEvents = events.filter((event) => event?.e === "attack").length;
        const projectileEvents = events.filter((event) => [
          "mortarLaunch", "artilleryTarget", "panzerfaustLaunch",
        ].includes(event?.e)).length;
        monitor.snapshotCount += 1;
        monitor.attackEventCount += attackEvents;
        monitor.projectileEventCount += projectileEvents;
        monitor.minEntityCount = Math.min(monitor.minEntityCount, entityCount);
        monitor.maxEntityCount = Math.max(monitor.maxEntityCount, entityCount);
        monitor.lastSnapshotTick = Number(snapshot?.tick) || monitor.lastSnapshotTick;
        if (attackEvents > 0 || projectileEvents > 0) {
          monitor.combatSnapshotCount += 1;
          monitor.lastCombatTick = Number(snapshot?.tick) || monitor.lastCombatTick;
        }
      });
      window.__rtsLabHellholeMonitor = monitor;
      const params = new URL(window.location.href).searchParams;
      return {
        action: "verifyLabHellhole",
        initialRenderedFrames,
        facts: {
          scenarioId: params.get("scenario") || "",
          map: params.get("map") || "",
          labMode: !!match?.labMetadata,
          visionMode: match?.labMetadata?.vision?.mode || "",
          playerIds: (match?.predictionStartInfo?.players || []).map((player) => player.id),
          godModePlayers: match?.labMetadata?.godModePlayers || [],
          supplyByOwner,
          countsByOwner,
          entityCount: state?._curById?.size || 0,
          snapshotCodec: stats.snapshotCodec || "",
          snapshotFrameKind: stats.snapshotFrameKind || "",
          snapshotMessageCount: stats.messageCount || 0,
        },
      };
    }, descriptor);
    const errors = validateLabHellholeFacts(action.facts, descriptor);
    if (errors.length > 0) action.error = errors.join("; ");
    result.actions.push(action);
    result.labHellhole = action;
    if (action.error) result.error = action.error;
  } catch (err) {
    const message = `timed out waiting for Lab Hellhole setup: ${err.message}`;
    result.actions.push({ action: "verifyLabHellhole", error: message });
    result.error = message;
  }
}
