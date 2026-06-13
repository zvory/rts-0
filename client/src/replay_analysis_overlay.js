import { STATS } from "./config.js";
import { isUnit } from "./protocol.js";

const STORAGE_KEY = "rts.replayAnalysisOverlay";
const ARMY_VALUE_TAB_ID = "army-value";

export const REPLAY_ANALYSIS_TABS = Object.freeze([
  { id: ARMY_VALUE_TAB_ID, label: "Army value" },
  { id: "production", label: "Production" },
  { id: "units", label: "Units" },
  { id: "units-lost", label: "Units lost" },
  { id: "resources-lost", label: "Resources lost" },
]);

export function createReplayAnalysisOverlayPreferences(storage = safeLocalStorage()) {
  const fallback = {
    selectedTab: REPLAY_ANALYSIS_TABS[0].id,
    visible: true,
    collapsed: false,
  };
  const state = { ...fallback, ...readStoredPreferences(storage) };
  normalizePreferences(state, fallback);

  return {
    get selectedTab() {
      return state.selectedTab;
    },
    set selectedTab(value) {
      state.selectedTab = validTabId(value) ? value : fallback.selectedTab;
      writeStoredPreferences(storage, state);
    },
    get visible() {
      return state.visible;
    },
    set visible(value) {
      state.visible = value !== false;
      writeStoredPreferences(storage, state);
    },
    get collapsed() {
      return state.collapsed;
    },
    set collapsed(value) {
      state.collapsed = value === true;
      writeStoredPreferences(storage, state);
    },
    snapshot() {
      return { ...state };
    },
  };
}

export class ReplayAnalysisOverlay {
  constructor({
    root,
    preferences = createReplayAnalysisOverlayPreferences(),
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
    this.onClick = (ev) => this.handleClick(ev);
    this.mount();
  }

  mount() {
    if (!this.root || this.el) return;

    this.el = document.createElement("aside");
    this.el.className = "replay-analysis-overlay";
    this.el.setAttribute("aria-label", "Replay analysis");
    this.el.addEventListener("click", this.onClick);

    this.panel = document.createElement("section");
    this.panel.className = "replay-analysis-panel hud-panel";

    const header = document.createElement("div");
    header.className = "replay-analysis-header";

    const title = document.createElement("h2");
    title.textContent = "Analysis";
    header.appendChild(title);

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
    this.tabsEl.setAttribute("aria-label", "Replay analysis metrics");

    for (const tab of REPLAY_ANALYSIS_TABS) {
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

    this.showButton = this.buildIconButton("Show replay analysis", "replay-analysis-show", "▣", { show: "1" });
    this.el.appendChild(this.showButton);
    this.root.appendChild(this.el);
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

  render() {
    if (!this.el || !this.panel || !this.tabsEl || !this.bodyEl || !this.showButton) return;
    const selectedTab = validTabId(this.preferences.selectedTab)
      ? this.preferences.selectedTab
      : REPLAY_ANALYSIS_TABS[0].id;
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

    const tab = REPLAY_ANALYSIS_TABS.find((item) => item.id === selectedTab) || REPLAY_ANALYSIS_TABS[0];
    this.bodyEl.setAttribute("aria-labelledby", `replay-analysis-tab-${tab.id}`);
    this.renderBody(tab);
  }

  update() {
    if (!this.bodyEl || this.bodyEl.hidden || this.preferences.selectedTab !== ARMY_VALUE_TAB_ID) return;
    this.renderBody(REPLAY_ANALYSIS_TABS[0]);
  }

  renderBody(tab) {
    if (!this.bodyEl) return;
    if (tab.id === ARMY_VALUE_TAB_ID) {
      const rows = calculateViewportArmyValue({
        entities: this.getEntities(),
        cameraBounds: this.getCameraBounds(),
        players: this.getPlayers(),
        stats: this.stats,
      });
      this.bodyEl.replaceChildren(this.renderArmyValue(rows));
      return;
    }
    this.bodyEl.replaceChildren(this.renderPlaceholder(tab));
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

      const steel = document.createElement("span");
      steel.className = "replay-army-value-steel";
      steel.textContent = formatValue(row.steel);
      steel.title = "Steel value";

      const oil = document.createElement("span");
      oil.className = "replay-army-value-oil";
      oil.textContent = formatValue(row.oil);
      oil.title = "Oil value";

      item.append(swatch, name, steel, oil);
      wrap.appendChild(item);
    }
    return wrap;
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
    if (this.el) {
      this.el.removeEventListener("click", this.onClick);
      this.el.remove();
    }
    this.el = null;
    this.panel = null;
    this.tabsEl = null;
    this.bodyEl = null;
    this.showButton = null;
  }
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

function formatValue(value) {
  return String(Math.max(0, Math.round(Number(value) || 0)));
}

function safeCssColor(color) {
  return typeof color === "string" && /^#[0-9a-fA-F]{3,8}$/.test(color) ? color : "#e7dfc5";
}

function validTabId(id) {
  return REPLAY_ANALYSIS_TABS.some((tab) => tab.id === id);
}

function normalizePreferences(state, fallback) {
  if (!validTabId(state.selectedTab)) state.selectedTab = fallback.selectedTab;
  state.visible = state.visible !== false;
  state.collapsed = state.collapsed === true;
}

function safeLocalStorage() {
  try {
    return typeof window !== "undefined" ? window.localStorage : null;
  } catch {
    return null;
  }
}

function readStoredPreferences(storage) {
  if (!storage) return {};
  try {
    const raw = storage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function writeStoredPreferences(storage, state) {
  if (!storage) return;
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // Storage failures should not break replay viewing.
  }
}
