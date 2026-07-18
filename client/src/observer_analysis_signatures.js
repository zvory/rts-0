const BODY_SIGNATURES = new WeakMap();

export function renderObserverAnalysisBody(overlay, tab, frameViews, ids, { profiler = null } = {}) {
  if (!overlay?.bodyEl || !tab) return;
  const replace = (signature, buildNode) => {
    const changed = replaceObserverAnalysisBody(overlay.bodyEl, `${tab.id}:${signature}`, buildNode);
    profiler?.recordDiagnosticCounter?.(
      `observer.dirty.${observerDiagnosticTab(tab.id)}.${changed ? "miss" : "hit"}`,
    );
  };
  if (tab.id === ids.armyValue) {
    const rows = ids.calculateViewportArmyValue({
      entities: Array.isArray(frameViews?.authoritativeEntities)
        ? frameViews.authoritativeEntities
        : overlay.getEntities(),
      cameraBounds: overlay.getCameraBounds(),
      players: overlay.getPlayers(),
      stats: overlay.stats,
    });
    replace(armyValueBodySignature(rows), () => overlay.renderArmyValue(rows));
    return;
  }
  const analysisSig = observerAnalysisBodySignature(overlay.analysis, overlay.getPlayers());
  if (tab.id === ids.production) {
    replace(analysisSig, () => overlay.renderProduction(overlay.analysis));
  } else if (tab.id === ids.research) {
    replace(analysisSig, () => overlay.renderResearch(overlay.analysis));
  } else if (tab.id === ids.units) {
    replace(analysisSig, () => overlay.renderUnits(overlay.analysis));
  } else if (tab.id === ids.resources) {
    replace(analysisSig, () => overlay.renderResources(overlay.analysis));
  } else if (tab.id === ids.unitsLost) {
    replace(analysisSig, () => overlay.renderUnitsLost(overlay.analysis));
  } else if (tab.id === ids.resourcesLost) {
    replace(analysisSig, () => overlay.renderResourcesLost(overlay.analysis));
  } else {
    replace("placeholder", () => overlay.renderPlaceholder(tab));
  }
}

function replaceObserverAnalysisBody(bodyEl, signature, buildNode) {
  if (!bodyEl) return false;
  if (BODY_SIGNATURES.get(bodyEl) === signature) return false;
  BODY_SIGNATURES.set(bodyEl, signature);
  bodyEl.replaceChildren(buildNode());
  return true;
}

function armyValueBodySignature(rows) {
  return (rows || []).map((row) => [
    row.owner,
    row.name,
    safeCssColor(row.color),
    formatValue(row.steel),
    formatValue(row.oil),
  ].join(":")).join("|");
}

function observerAnalysisBodySignature(analysis, players) {
  return `${JSON.stringify(analysis || null)}|players:${observerPlayersSignature(players)}`;
}

function observerPlayersSignature(players) {
  return (players || []).map((player) => [
    player?.id,
    player?.name || "",
    safeCssColor(player?.color),
  ].join(":")).join("|");
}

function formatValue(value) {
  return String(Math.max(0, Math.round(Number(value) || 0)));
}

function safeCssColor(color) {
  return typeof color === "string" && /^#[0-9a-fA-F]{3,8}$/.test(color) ? color : "#e7dfc5";
}

function observerDiagnosticTab(tabId) {
  return String(tabId || "unknown").replace(/[^A-Za-z0-9_-]/g, "_").slice(0, 32) || "unknown";
}
