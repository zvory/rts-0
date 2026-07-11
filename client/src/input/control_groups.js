const CONTROL_GROUP_DOUBLE_TAP_MS = 400;

export function _controlGroupSlotFromKey(ev) {
  const match = /^(?:Digit|Numpad)([0-9])$/.exec(ev.code || "");
  if (!match) return null;
  const n = Number(match[1]);
  return n === 0 ? 9 : n - 1;
}

export function _controlGroupIsWindowsPlatform() {
  const nav = globalThis.navigator;
  const platform = `${nav?.userAgentData?.platform || nav?.platform || ""}`;
  if (/Windows/i.test(platform)) return true;
  return /\bWindows\b|\bWin(?:32|64|CE)\b/i.test(`${nav?.userAgent || ""}`);
}

export function _controlGroupSaveModifierActive(ev, runtime = {}) {
  const isWindows = runtime.isWindows ?? _controlGroupIsWindowsPlatform();
  const isInstalledApp = !!runtime.isInstalledApp;
  if (isInstalledApp) return ev.altKey || ev.ctrlKey || ev.metaKey;
  if (isWindows) {
    return ev.altKey && !ev.ctrlKey && !ev.metaKey;
  }
  return ev.altKey || ev.ctrlKey || ev.metaKey;
}

export function _handleControlGroupHotkey(ev) {
  const slot = this._controlGroupSlotFromKey(ev);
  if (slot == null || !controlGroupsEnabled(this.state)) return false;

  const save = _controlGroupSaveModifierActive(ev, {
    isInstalledApp: typeof this.installedAppRuntime === "function" && this.installedAppRuntime(),
  }) && !ev.shiftKey;
  const add = ev.shiftKey && !ev.altKey && !ev.ctrlKey && !ev.metaKey;
  const recall = !ev.altKey && !ev.ctrlKey && !ev.metaKey && !ev.shiftKey;
  if (!save && !add && !recall) return false;

  ev.preventDefault();
  if (typeof ev.stopPropagation === "function") ev.stopPropagation();

  if (save) {
    this.state.setControlGroup(slot, this.state.selection);
    this._lastControlGroupTap = null;
    return true;
  }
  if (add) {
    this.state.addToControlGroup(slot, this.state.selection);
    this._lastControlGroupTap = null;
    return true;
  }

  const now = performance.now();
  const last = this._lastControlGroupTap;
  const doubleTap = last && last.slot === slot && now - last.t <= CONTROL_GROUP_DOUBLE_TAP_MS;
  const ids = this.state.selectControlGroup(slot);
  if (ids.length === 0) {
    this._lastControlGroupTap = null;
    return true;
  }
  if (doubleTap) {
    this._jumpToControlGroupCluster(slot);
    this._lastControlGroupTap = null;
  } else {
    this._lastControlGroupTap = { slot, t: now };
  }
  return true;
}

function controlGroupsEnabled(state) {
  if (state?.controlPolicy?.kind === "lab") {
    return !!state.controlPolicy.canUseCommandSurface?.(state);
  }
  return !state?.spectator;
}

export function _jumpToControlGroupCluster(slot) {
  const entities = this.state.controlGroupEntities(slot);
  if (entities.length === 0) return false;
  const viewportBounds = this.camera.viewportGroundBounds?.();
  const cameraSnapshot = this.camera.snapshot?.();
  const viewW = viewportBounds ? viewportBounds.maxX - viewportBounds.minX : 0;
  const viewH = viewportBounds ? viewportBounds.maxY - viewportBounds.minY : 0;
  if (!Number.isFinite(viewW) || !Number.isFinite(viewH) || viewW <= 0 || viewH <= 0) {
    return false;
  }
  const currentX = cameraSnapshot?.focus?.x;
  const currentY = cameraSnapshot?.focus?.y;
  if (!Number.isFinite(currentX) || !Number.isFinite(currentY)) return false;

  const lefts = [];
  const tops = [];
  for (const e of entities) {
    lefts.push(e.x - viewW / 2, e.x, e.x - viewW);
    tops.push(e.y - viewH / 2, e.y, e.y - viewH);
  }

  let best = null;
  for (const left of lefts) {
    for (const top of tops) {
      const right = left + viewW;
      const bottom = top + viewH;
      const contained = entities.filter((e) =>
        e.x >= left && e.x <= right && e.y >= top && e.y <= bottom,
      );
      if (contained.length === 0) continue;
      const score = _controlGroupClusterScore(
        contained,
        left + viewW / 2,
        top + viewH / 2,
        currentX,
        currentY,
      );
      if (!best || _controlGroupScoreBeats(score, best.score)) {
        best = { left, top, score };
      }
    }
  }

  if (!best) return false;
  this.camera.focusAt({ x: best.left + viewW / 2, y: best.top + viewH / 2 });
  return true;
}

function _controlGroupClusterScore(entities, centerX, centerY, currentX, currentY) {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const e of entities) {
    minX = Math.min(minX, e.x);
    minY = Math.min(minY, e.y);
    maxX = Math.max(maxX, e.x);
    maxY = Math.max(maxY, e.y);
  }
  const boxCenterX = (minX + maxX) / 2;
  const boxCenterY = (minY + maxY) / 2;
  return {
    count: entities.length,
    spread: Math.max(1, maxX - minX) * Math.max(1, maxY - minY),
    centerMiss: Math.hypot(centerX - boxCenterX, centerY - boxCenterY),
    currentMiss: Math.hypot(centerX - currentX, centerY - currentY),
  };
}

function _controlGroupScoreBeats(a, b) {
  if (a.count !== b.count) return a.count > b.count;
  if (a.spread !== b.spread) return a.spread < b.spread;
  if (a.centerMiss !== b.centerMiss) return a.centerMiss < b.centerMiss;
  return a.currentMiss < b.currentMiss;
}
