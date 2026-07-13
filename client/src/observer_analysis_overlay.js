import { STATS, UPGRADES } from "./config.js";
import { isUnit } from "./protocol.js";
import {
  createObserverAnalysisOverlayPreferences,
  isObserverAnalysisTabId,
  OBSERVER_ANALYSIS_TABS,
} from "./observer_analysis_preferences.js";
import {
  FloatingPanelPositioner,
  isMobileDebugPanelViewport,
} from "./floating_panel_positioner.js";
import { playerAnalysisRows } from "./observer_analysis_rows.js";
import { normalizeResourceWindows, renderResourcesMetric } from "./observer_analysis_resources.js";
import { renderObserverAnalysisBody } from "./observer_analysis_signatures.js";
import { resourceValueElement } from "./resource_icons.js";

const ARMY_VALUE_TAB_ID = "army-value";
const PRODUCTION_TAB_ID = "production";
const UNITS_TAB_ID = "units";
const RESOURCES_TAB_ID = "resources";
const UNITS_LOST_TAB_ID = "units-lost";
const RESOURCES_LOST_TAB_ID = "resources-lost";

export { createObserverAnalysisOverlayPreferences, OBSERVER_ANALYSIS_TABS };

export function shouldMountObserverAnalysisOverlay({ capabilities } = {}) {
  return capabilities?.diagnostics?.observerAnalysis === true;
}

export class ObserverAnalysisOverlay {
  constructor({
    root,
    preferences = createObserverAnalysisOverlayPreferences(),
    getEntities = () => [],
    getCameraBounds = () => null,
    getPlayers = () => [],
    stats = STATS,
  }) {
    this.root = root;
    this.preferences = preferences;
    this.getEntities = getEntities;
    this.getCameraBounds = getCameraBounds;
    this.getPlayers = getPlayers;
    this.stats = stats;
    this.el = null;
    this.panel = null;
    this.tabsEl = null;
    this.bodyEl = null;
    this.showButton = null;
    this.positioner = null;
    this.analysis = null;
    this.onClick = (ev) => this.handleClick(ev);
    this.onKeyDown = (ev) => this.handleKeyDown(ev);
    this.mount();
  }

  mount() {
    if (!this.root || this.el) return;

    this.el = document.createElement("aside");
    this.el.className = "replay-analysis-overlay";
    this.el.setAttribute("aria-label", "Observer analysis");
    this.el.addEventListener("click", this.onClick);
    this.el.addEventListener("keydown", this.onKeyDown);

    this.panel = document.createElement("section");
    this.panel.className = "replay-analysis-panel hud-panel";

    const header = document.createElement("div");
    header.className = "replay-analysis-header";

    const dragHandle = document.createElement("div");
    dragHandle.className = "replay-analysis-drag-handle";
    dragHandle.tabIndex = 0;
    dragHandle.setAttribute("role", "button");
    dragHandle.setAttribute("aria-label", "Move observer analysis");
    dragHandle.setAttribute("aria-keyshortcuts", "ArrowUp ArrowDown ArrowLeft ArrowRight Home");
    dragHandle.title = "Drag to move. Arrow keys nudge. Home resets.";

    const grip = document.createElement("span");
    grip.className = "replay-analysis-grip";
    grip.setAttribute("aria-hidden", "true");
    grip.textContent = "::::";

    const title = document.createElement("h2");
    title.textContent = "Analysis";
    dragHandle.append(grip, title);
    header.appendChild(dragHandle);

    const actions = document.createElement("div");
    actions.className = "replay-analysis-actions";
    actions.append(
      this.buildIconButton("Collapse analysis", "replay-analysis-collapse", "▾", { collapse: "1" }),
      this.buildIconButton("Hide analysis", "replay-analysis-hide", "×", { hide: "1" }),
    );
    header.appendChild(actions);

    this.tabsEl = document.createElement("div");
    this.tabsEl.className = "replay-analysis-tabs";
    this.tabsEl.setAttribute("role", "tablist");
    this.tabsEl.setAttribute("aria-label", "Observer analysis metrics");

    for (const tab of OBSERVER_ANALYSIS_TABS) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "replay-analysis-tab";
      btn.id = `replay-analysis-tab-${tab.id}`;
      btn.dataset.tabId = tab.id;
      btn.setAttribute("role", "tab");
      btn.setAttribute("aria-controls", "replay-analysis-body");
      btn.textContent = tab.label;
      this.tabsEl.appendChild(btn);
    }

    this.bodyEl = document.createElement("div");
    this.bodyEl.id = "replay-analysis-body";
    this.bodyEl.className = "replay-analysis-body";
    this.bodyEl.setAttribute("role", "tabpanel");

    this.panel.append(header, this.tabsEl, this.bodyEl);
    this.el.appendChild(this.panel);

    this.showButton = this.buildIconButton("Show observer analysis", "replay-analysis-show", "▣", { show: "1" });
    this.el.appendChild(this.showButton);
    this.root.appendChild(this.el);
    this.positioner = new FloatingPanelPositioner({
      root: this.el,
      defaultPosition: { left: 12, top: 58 },
      defaultSize: { width: 316, height: 260 },
      readPosition: () => this.preferences?.position,
      savePosition: (position) => {
        if (this.preferences) this.preferences.position = position;
      },
      clearPosition: () => this.preferences?.clearPosition?.(),
      isMobileViewport: isMobileDebugPanelViewport,
    });
    this.positioner.mount(dragHandle);
    this.render();
  }

  buildIconButton(label, className, text, dataset = {}) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = className;
    btn.textContent = text;
    btn.title = label;
    btn.setAttribute("aria-label", label);
    Object.assign(btn.dataset, dataset);
    return btn;
  }

  handleClick(ev) {
    const target = ev.target instanceof Element ? ev.target : null;
    const btn = target?.closest("button");
    if (!btn || !this.el?.contains(btn)) return;
    ev.preventDefault();
    ev.stopPropagation();

    if (btn.dataset.tabId) {
      this.preferences.selectedTab = btn.dataset.tabId;
    } else if (btn.dataset.collapse) {
      this.preferences.collapsed = !this.preferences.collapsed;
      if (!this.preferences.visible) this.preferences.visible = true;
    } else if (btn.dataset.hide) {
      this.preferences.visible = false;
    } else if (btn.dataset.show) {
      this.preferences.visible = true;
      this.preferences.collapsed = false;
    }
    this.render();
  }

  handleKeyDown(ev) {
    const target = ev.target instanceof Element ? ev.target : null;
    const tab = target?.closest(".replay-analysis-tab");
    if (!tab || !this.tabsEl?.contains(tab)) return;

    const tabs = [...this.tabsEl.querySelectorAll(".replay-analysis-tab")];
    const currentIndex = tabs.indexOf(tab);
    if (currentIndex < 0) return;

    let nextIndex = currentIndex;
    if (ev.key === "ArrowRight" || ev.key === "ArrowDown") {
      nextIndex = (currentIndex + 1) % tabs.length;
    } else if (ev.key === "ArrowLeft" || ev.key === "ArrowUp") {
      nextIndex = (currentIndex - 1 + tabs.length) % tabs.length;
    } else if (ev.key === "Home") {
      nextIndex = 0;
    } else if (ev.key === "End") {
      nextIndex = tabs.length - 1;
    } else {
      return;
    }

    ev.preventDefault();
    ev.stopPropagation();
    const nextTab = tabs[nextIndex];
    this.preferences.selectedTab = nextTab.dataset.tabId;
    this.render();
    nextTab.focus?.();
  }

  render() {
    if (!this.el || !this.panel || !this.tabsEl || !this.bodyEl || !this.showButton) return;
    const selectedTab = isObserverAnalysisTabId(this.preferences.selectedTab)
      ? this.preferences.selectedTab
      : OBSERVER_ANALYSIS_TABS[0].id;
    const visible = this.preferences.visible !== false;
    const collapsed = this.preferences.collapsed === true;

    this.el.classList.toggle("is-hidden", !visible);
    this.el.classList.toggle("is-collapsed", visible && collapsed);
    this.panel.hidden = !visible;
    this.showButton.hidden = visible;
    this.tabsEl.hidden = collapsed;
    this.bodyEl.hidden = collapsed;

    const collapse = this.panel.querySelector(".replay-analysis-collapse");
    if (collapse) {
      collapse.textContent = collapsed ? "▸" : "▾";
      collapse.title = collapsed ? "Expand analysis" : "Collapse analysis";
      collapse.setAttribute("aria-label", collapse.title);
      collapse.setAttribute("aria-expanded", String(!collapsed));
    }

    for (const btn of this.tabsEl.querySelectorAll(".replay-analysis-tab")) {
      const selected = btn.dataset.tabId === selectedTab;
      btn.classList.toggle("active", selected);
      btn.setAttribute("aria-selected", String(selected));
      btn.tabIndex = selected ? 0 : -1;
    }

    const tab = OBSERVER_ANALYSIS_TABS.find((item) => item.id === selectedTab) || OBSERVER_ANALYSIS_TABS[0];
    this.bodyEl.setAttribute("aria-labelledby", `replay-analysis-tab-${tab.id}`);
    this.renderBody(tab);
  }

  update(frameViews = null, { profiler = null } = {}) {
    if (!this.bodyEl || this.bodyEl.hidden || this.preferences.selectedTab !== ARMY_VALUE_TAB_ID) return;
    this.renderBody(OBSERVER_ANALYSIS_TABS[0], frameViews, { profiler });
  }

  applyObserverAnalysis(payload) {
    this.analysis = normalizeObserverAnalysisPayload(payload);
    if (!this.bodyEl || this.bodyEl.hidden) return;
    const selected = isObserverAnalysisTabId(this.preferences.selectedTab)
      ? this.preferences.selectedTab
      : OBSERVER_ANALYSIS_TABS[0].id;
    if (
      selected === PRODUCTION_TAB_ID
      || selected === UNITS_TAB_ID
      || selected === RESOURCES_TAB_ID
      || selected === UNITS_LOST_TAB_ID
      || selected === RESOURCES_LOST_TAB_ID
    ) {
      const tab = OBSERVER_ANALYSIS_TABS.find((item) => item.id === selected);
      this.renderBody(tab);
    }
  }

  renderBody(tab, frameViews = null, { profiler = null } = {}) {
    renderObserverAnalysisBody(this, tab, frameViews, {
      armyValue: ARMY_VALUE_TAB_ID,
      production: PRODUCTION_TAB_ID,
      units: UNITS_TAB_ID,
      resources: RESOURCES_TAB_ID,
      unitsLost: UNITS_LOST_TAB_ID,
      resourcesLost: RESOURCES_LOST_TAB_ID,
      calculateViewportArmyValue,
    }, { profiler });
    profiler?.recordDiagnosticCounter?.(
      Array.isArray(frameViews?.authoritativeEntities)
        ? "entityViews.cache.hit.observer.authoritative"
        : "entityViews.uncached.observer.authoritative",
    );
  }

  renderArmyValue(rows) {
    const wrap = document.createElement("div");
    wrap.className = "replay-army-value";

    const header = document.createElement("div");
    header.className = "replay-army-value-heading";
    header.textContent = "Visible in viewport";
    wrap.appendChild(header);

    if (!rows.length) {
      const empty = document.createElement("div");
      empty.className = "replay-army-value-empty";
      empty.textContent = "No players";
      wrap.appendChild(empty);
      return wrap;
    }

    for (const row of rows) {
      const item = document.createElement("div");
      item.className = "replay-army-value-row";

      const swatch = document.createElement("span");
      swatch.className = "replay-army-value-swatch";
      swatch.setAttribute("style", `background:${safeCssColor(row.color)};`);
      swatch.setAttribute("aria-hidden", "true");

      const name = document.createElement("span");
      name.className = "replay-army-value-name";
      name.textContent = row.name;

      const steel = resourceValueElement("steel", row.steel, "replay-army-value-steel");
      const oil = resourceValueElement("oil", row.oil, "replay-army-value-oil");

      item.append(swatch, name, steel, oil);
      wrap.appendChild(item);
    }
    return wrap;
  }

  renderProduction(analysis) {
    const wrap = this.renderAnalysisMetric("replay-production", "Current queues");
    const rows = playerAnalysisRows({ analysis, players: this.getPlayers() });
    if (!analysis) {
      wrap.appendChild(renderEmptyMetric("Waiting for observer analysis"));
      return wrap;
    }
    if (!rows.some((row) => row.production.length > 0)) {
      wrap.appendChild(renderEmptyMetric("No active production"));
      return wrap;
    }

    for (const player of rows) {
      if (!player.production.length) continue;
      wrap.appendChild(this.renderPlayerHeading(player));
      for (const item of player.production) {
        const row = document.createElement("div");
        row.className = "replay-production-row";

        const icon = document.createElement("span");
        icon.className = "replay-analysis-kind-icon";
        icon.textContent = itemIcon(item.itemKind, item.itemType, this.stats);

        const main = document.createElement("span");
        main.className = "replay-production-main";
        const itemLabel = itemLabelFor(item.itemKind, item.itemType, this.stats);
        const buildingLabel = kindLabel(item.buildingKind, this.stats);
        main.textContent = `${itemLabel} at ${buildingLabel}`;

        const progress = document.createElement("span");
        progress.className = "replay-production-progress";
        progress.textContent = `${formatPercent(item.progress)}%`;
        progress.title = "Production progress";

        const queue = document.createElement("span");
        queue.className = "replay-production-queue";
        queue.textContent = `Q ${formatValue(item.queueDepth)}`;
        queue.title = "Queue depth";

        row.append(icon, main, progress, queue);
        wrap.appendChild(row);
      }
    }
    return wrap;
  }

  renderUnits(analysis) {
    const wrap = this.renderAnalysisMetric("replay-units", "Current army");
    const rows = playerAnalysisRows({ analysis, players: this.getPlayers() });
    if (!analysis) {
      wrap.appendChild(renderEmptyMetric("Waiting for observer analysis"));
      return wrap;
    }
    if (!rows.some((row) => row.units.length > 0)) {
      wrap.appendChild(renderEmptyMetric("No units"));
      return wrap;
    }

    for (const player of rows) {
      const units = [...player.units].sort(compareKindRows(this.stats));
      if (!units.length) continue;
      wrap.appendChild(this.renderPlayerHeading(player));

      const total = units.reduce((acc, unit) => {
        acc.count += unit.count;
        acc.steel += unit.steelValue;
        acc.oil += unit.oilValue;
        return acc;
      }, { count: 0, steel: 0, oil: 0 });
      wrap.appendChild(renderUnitRow({
        className: "replay-units-row is-total",
        label: "Total",
        icon: "#",
        count: total.count,
        steel: total.steel,
        oil: total.oil,
      }));

      for (const unit of units) {
        wrap.appendChild(renderUnitRow({
          className: "replay-units-row",
          label: kindLabel(unit.kind, this.stats),
          icon: itemIcon(unit.kind, "unit", this.stats),
          count: unit.count,
          steel: unit.steelValue,
          oil: unit.oilValue,
        }));
      }
    }
    return wrap;
  }

  renderResources(analysis) {
    return renderResourcesMetric({ analysis, players: this.getPlayers() });
  }

  renderUnitsLost(analysis) {
    const wrap = this.renderAnalysisMetric("replay-units-lost", "Destroyed units");
    const rows = playerAnalysisRows({ analysis, players: this.getPlayers() });
    if (!analysis) {
      wrap.appendChild(renderEmptyMetric("Waiting for observer analysis"));
      return wrap;
    }
    if (!rows.some((row) => row.unitsLost.length > 0)) {
      wrap.appendChild(renderEmptyMetric("No units lost"));
      return wrap;
    }

    for (const player of rows) {
      const unitsLost = [...player.unitsLost].sort(compareKindRows(this.stats));
      if (!unitsLost.length) continue;
      wrap.appendChild(this.renderPlayerHeading(player));

      const total = unitsLost.reduce((acc, unit) => {
        acc.count += unit.count;
        acc.steel += unit.steelValue;
        acc.oil += unit.oilValue;
        return acc;
      }, { count: 0, steel: 0, oil: 0 });
      wrap.appendChild(renderUnitRow({
        className: "replay-units-row replay-units-lost-row is-total",
        label: "Total lost",
        icon: "#",
        count: total.count,
        steel: total.steel,
        oil: total.oil,
      }));

      for (const unit of unitsLost) {
        wrap.appendChild(renderUnitRow({
          className: "replay-units-row replay-units-lost-row",
          label: kindLabel(unit.kind, this.stats),
          icon: itemIcon(unit.kind, "unit", this.stats),
          count: unit.count,
          steel: unit.steelValue,
          oil: unit.oilValue,
        }));
      }
    }
    return wrap;
  }

  renderResourcesLost(analysis) {
    const wrap = this.renderAnalysisMetric("replay-resources-lost", "Dead unit value");
    const note = document.createElement("div");
    note.className = "replay-analysis-note";
    note.textContent = "Spent steel and oil value of units that died. Buildings, cancelled queues, refunds, harvesting, and stockpile changes are excluded.";
    wrap.appendChild(note);

    const rows = playerAnalysisRows({ analysis, players: this.getPlayers() });
    if (!analysis) {
      wrap.appendChild(renderEmptyMetric("Waiting for observer analysis"));
      return wrap;
    }
    if (!rows.length) {
      wrap.appendChild(renderEmptyMetric("No players"));
      return wrap;
    }

    const total = rows.reduce((acc, player) => {
      acc.steel += player.resourcesLost.steel;
      acc.oil += player.resourcesLost.oil;
      return acc;
    }, { steel: 0, oil: 0 });
    wrap.appendChild(renderResourceLostRow({
      className: "replay-resources-lost-row is-total",
      name: "Total",
      color: "#e7dfc5",
      steel: total.steel,
      oil: total.oil,
    }));

    for (const player of rows) {
      wrap.appendChild(renderResourceLostRow({
        className: "replay-resources-lost-row",
        name: player.name,
        color: player.color,
        steel: player.resourcesLost.steel,
        oil: player.resourcesLost.oil,
      }));
    }
    return wrap;
  }

  renderAnalysisMetric(className, headingText) {
    const wrap = document.createElement("div");
    wrap.className = `replay-analysis-metric ${className}`;
    const heading = document.createElement("div");
    heading.className = "replay-analysis-metric-heading";
    heading.textContent = headingText;
    wrap.appendChild(heading);
    return wrap;
  }

  renderPlayerHeading(player) {
    const heading = document.createElement("div");
    heading.className = "replay-analysis-player-heading";

    const swatch = document.createElement("span");
    swatch.className = "replay-analysis-player-swatch";
    swatch.setAttribute("style", `background:${safeCssColor(player.color)};`);
    swatch.setAttribute("aria-hidden", "true");

    const name = document.createElement("span");
    name.className = "replay-analysis-player-name";
    name.textContent = player.name;

    heading.append(swatch, name);
    return heading;
  }

  renderPlaceholder(tab) {
    const wrap = document.createElement("div");
    wrap.className = "replay-analysis-placeholder";

    const label = document.createElement("strong");
    label.textContent = tab.label;
    const text = document.createElement("span");
    text.textContent = "Placeholder metric shell";
    wrap.append(label, text);
    return wrap;
  }

  destroy() {
    this.positioner?.destroy();
    if (this.el) {
      this.el.removeEventListener("click", this.onClick);
      this.el.removeEventListener("keydown", this.onKeyDown);
      this.el.remove();
    }
    this.el = null;
    this.panel = null;
    this.tabsEl = null;
    this.bodyEl = null;
    this.showButton = null;
    this.positioner = null;
  }
}

function normalizeObserverAnalysisPayload(payload) {
  if (!payload || typeof payload !== "object") return null;
  return {
    tick: Math.max(0, Math.trunc(Number(payload.tick) || 0)),
    players: Array.isArray(payload.players)
      ? payload.players.map(normalizeAnalysisPlayer).filter(Boolean)
      : [],
  };
}

function normalizeAnalysisPlayer(player) {
  const id = Number(player?.id);
  if (!Number.isFinite(id) || id <= 0) return null;
  return {
    id,
    units: normalizeKindRows(player.units),
    production: normalizeProductionRows(player.production),
    unitsLost: normalizeKindRows(player.unitsLost),
    resourcesLost: {
      steel: Math.max(0, Math.trunc(Number(player.resourcesLost?.steel) || 0)),
      oil: Math.max(0, Math.trunc(Number(player.resourcesLost?.oil) || 0)),
    },
    resources: normalizeResourceWindows(player.resources),
  };
}

function normalizeKindRows(rows) {
  if (!Array.isArray(rows)) return [];
  return rows.map((row) => ({
    kind: String(row?.kind || ""),
    count: Math.max(0, Math.trunc(Number(row?.count) || 0)),
    steelValue: Math.max(0, Math.trunc(Number(row?.steelValue) || 0)),
    oilValue: Math.max(0, Math.trunc(Number(row?.oilValue) || 0)),
  })).filter((row) => row.kind && row.count > 0);
}

function normalizeProductionRows(rows) {
  if (!Array.isArray(rows)) return [];
  return rows.map((row) => ({
    buildingId: Math.max(0, Math.trunc(Number(row?.buildingId) || 0)),
    buildingKind: String(row?.buildingKind || ""),
    itemKind: String(row?.itemKind || ""),
    itemType: row?.itemType === "upgrade" ? "upgrade" : "unit",
    progress: clamp01(Number(row?.progress) || 0),
    queueDepth: Math.max(0, Math.trunc(Number(row?.queueDepth) || 0)),
  })).filter((row) => row.buildingKind && row.itemKind);
}

function renderEmptyMetric(text) {
  const empty = document.createElement("div");
  empty.className = "replay-analysis-empty";
  empty.textContent = text;
  return empty;
}

function renderUnitRow({ className, icon, label, count, steel, oil }) {
  const row = document.createElement("div");
  row.className = className;

  const iconEl = document.createElement("span");
  iconEl.className = "replay-analysis-kind-icon";
  iconEl.textContent = icon;

  const labelEl = document.createElement("span");
  labelEl.className = "replay-units-label";
  labelEl.textContent = label;

  const countEl = document.createElement("span");
  countEl.className = "replay-units-count";
  countEl.textContent = formatValue(count);

  const steelEl = resourceValueElement("steel", steel, "replay-units-steel");
  const oilEl = resourceValueElement("oil", oil, "replay-units-oil");

  row.append(iconEl, labelEl, countEl, steelEl, oilEl);
  return row;
}

function renderResourceLostRow({ className, name, color, steel, oil }) {
  const row = document.createElement("div");
  row.className = className;

  const swatch = document.createElement("span");
  swatch.className = "replay-analysis-player-swatch";
  swatch.setAttribute("style", `background:${safeCssColor(color)};`);
  swatch.setAttribute("aria-hidden", "true");

  const nameEl = document.createElement("span");
  nameEl.className = "replay-resources-lost-name";
  nameEl.textContent = name;

  const steelEl = resourceValueElement("steel", steel, "replay-resources-lost-steel");
  const oilEl = resourceValueElement("oil", oil, "replay-resources-lost-oil");

  row.append(swatch, nameEl, steelEl, oilEl);
  return row;
}

export function calculateViewportArmyValue({
  entities = [],
  cameraBounds = null,
  players = [],
  stats = STATS,
} = {}) {
  const rowsByOwner = new Map();
  for (const player of players || []) {
    const id = Number(player?.id);
    if (!Number.isFinite(id) || id === 0) continue;
    rowsByOwner.set(id, {
      owner: id,
      name: player?.name || `Player ${id}`,
      color: player?.color || "#e7dfc5",
      steel: 0,
      oil: 0,
    });
  }

  if (!cameraBounds || !Array.isArray(entities)) return [...rowsByOwner.values()];
  const bounds = normalizeBounds(cameraBounds);
  if (!bounds) return [...rowsByOwner.values()];

  for (const entity of entities) {
    if (!entity || entity.shotReveal || !isUnit(entity.kind)) continue;
    const owner = Number(entity.owner);
    if (!Number.isFinite(owner) || owner === 0) continue;
    const x = Number(entity.x);
    const y = Number(entity.y);
    if (!Number.isFinite(x) || !Number.isFinite(y)) continue;

    const stat = stats?.[entity.kind] || {};
    const radius = Math.max(0, Number(stat.size) || 0);
    if (!circleIntersectsBounds(x, y, radius, bounds)) continue;

    const row = rowsByOwner.get(owner) || {
      owner,
      name: `Player ${owner}`,
      color: "#e7dfc5",
      steel: 0,
      oil: 0,
    };
    const cost = stat.cost || {};
    row.steel += Math.max(0, Number(cost.steel) || 0);
    row.oil += Math.max(0, Number(cost.oil) || 0);
    rowsByOwner.set(owner, row);
  }

  return [...rowsByOwner.values()].sort((a, b) => a.owner - b.owner);
}

function normalizeBounds(bounds) {
  const left = Number(bounds.left ?? bounds.x);
  const top = Number(bounds.top ?? bounds.y);
  const width = Number(bounds.width ?? bounds.w);
  const height = Number(bounds.height ?? bounds.h);
  if (![left, top, width, height].every(Number.isFinite) || width <= 0 || height <= 0) return null;
  return {
    left,
    top,
    right: left + width,
    bottom: top + height,
  };
}

function circleIntersectsBounds(x, y, radius, bounds) {
  return (
    x + radius >= bounds.left
    && x - radius <= bounds.right
    && y + radius >= bounds.top
    && y - radius <= bounds.bottom
  );
}

function compareKindRows(stats) {
  return (a, b) => {
    const labelCmp = kindLabel(a.kind, stats).localeCompare(kindLabel(b.kind, stats));
    if (labelCmp !== 0) return labelCmp;
    return a.kind.localeCompare(b.kind);
  };
}

function kindLabel(kind, stats = STATS) {
  return stats?.[kind]?.label || kindToTitle(kind);
}

function itemLabelFor(kind, itemType, stats = STATS) {
  if (itemType === "upgrade") return UPGRADES?.[kind]?.label || kindToTitle(kind);
  return kindLabel(kind, stats);
}

function itemIcon(kind, itemType, stats = STATS) {
  const icon = itemType === "upgrade" ? UPGRADES?.[kind]?.icon : stats?.[kind]?.icon;
  return icon || kindToIcon(kind);
}

function kindToTitle(kind) {
  return String(kind || "Unknown")
    .split("_")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function kindToIcon(kind) {
  const parts = String(kind || "?").split("_").filter(Boolean);
  const text = parts.length > 1
    ? parts.map((part) => part.charAt(0)).join("")
    : String(kind || "?").slice(0, 3);
  return text.toUpperCase() || "?";
}

function formatPercent(value) {
  return String(Math.round(clamp01(value) * 100));
}

function formatValue(value) {
  return String(Math.max(0, Math.round(Number(value) || 0)));
}

function clamp01(value) {
  if (!Number.isFinite(value)) return 0;
  return Math.min(1, Math.max(0, value));
}

function safeCssColor(color) {
  return typeof color === "string" && /^#[0-9a-fA-F]{3,8}$/.test(color) ? color : "#e7dfc5";
}
