import { LabPanelWindowChrome } from "./lab_panel_window.js";

const STORAGE_KEY = "rts.aiDiagnosticsPanel";
const WINDOW_STORAGE_KEY = "rts.aiDiagnosticsPanel.window.v1";
const MAP_LABELS_LAYER_ID = "labels";
const MAP_LAYER_ID_RE = /^[A-Za-z0-9:_-]{1,64}$/;
const RETIRED_MAP_LAYER_IDS = new Set(["regions", "voronoi"]);
const DEFAULT_MAP_LAYER_VISIBILITY = Object.freeze({
  chokes: true,
  bases: true,
  resources: true,
  [MAP_LABELS_LAYER_ID]: true,
});

export function shouldMountAiDiagnosticsPanel({ capabilities, players = [] } = {}) {
  return capabilities?.diagnostics?.observerAnalysis === true
    && players.some((player) => player?.isAi === true || player?.is_ai === true);
}

export function createAiDiagnosticsPanelPreferences(storage = safeLocalStorage()) {
  const fallback = {
    visible: true,
    collapsed: false,
    selectedPlayerId: null,
    mapLayers: { ...DEFAULT_MAP_LAYER_VISIBILITY },
  };
  const state = { ...fallback, ...readStoredPreferences(storage) };
  normalizePreferences(state);

  return {
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
    get selectedPlayerId() {
      return state.selectedPlayerId;
    },
    set selectedPlayerId(value) {
      state.selectedPlayerId = normalizeSelectedPlayerId(value);
      writeStoredPreferences(storage, state);
    },
    mapLayerVisibility(layerIds = []) {
      ensureKnownMapLayers(state, layerIds);
      return { ...state.mapLayers };
    },
    setMapLayerVisible(layerId, visible) {
      const id = normalizeMapLayerId(layerId);
      if (!id || isRetiredMapLayerId(id)) return;
      state.mapLayers = normalizeMapLayerVisibility(state.mapLayers);
      state.mapLayers[id] = visible === true;
      writeStoredPreferences(storage, state);
    },
    snapshot() {
      return { ...state, mapLayers: { ...state.mapLayers } };
    },
  };
}

export class AiDiagnosticsPanel {
  constructor({
    root,
    preferences = createAiDiagnosticsPanelPreferences(),
    getPlayers = () => [],
    onMapLayerVisibilityChange = null,
  }) {
    this.root = root;
    this.preferences = preferences;
    this.getPlayers = getPlayers;
    this.onMapLayerVisibilityChange = onMapLayerVisibilityChange;
    this.analysis = null;
    this.el = null;
    this.bodyEl = null;
    this.showButton = null;
    this.windowChrome = null;
    this.bodySignature = "";
    this.renderedPlayerId = null;
    this.onPanelClick = (ev) => this.handlePanelClick(ev);
    this.onTabKeydown = (ev) => this.handleTabKeydown(ev);
    this.onShowClick = (ev) => this.show(ev);
    this.mount();
    this.publishMapLayerVisibility();
  }

  mount() {
    if (!this.root || this.el) return;

    this.el = document.createElement("aside");
    this.el.className = "ai-diagnostics-panel-host ai-diagnostics-panel lab-panel";
    this.el.setAttribute("aria-label", "AI diagnostics");
    this.el.addEventListener("click", this.onPanelClick);
    this.el.addEventListener("keydown", this.onTabKeydown);

    this.windowChrome = new LabPanelWindowChrome(this.el, {
      storageKey: WINDOW_STORAGE_KEY,
    });
    const storedWindowState = this.windowChrome.readStoredState?.();
    if (!storedWindowState && this.preferences.collapsed === true) {
      this.windowChrome.setCollapsed(true, { save: false });
    }
    this.windowChrome.onCollapsedChange = (collapsed) => {
      this.preferences.collapsed = collapsed;
    };
    this.preferences.collapsed = this.windowChrome.collapsed;

    const header = this.windowChrome.renderHeader({
      kicker: "AI",
      title: "Diagnostics",
      collapseLabel: "AI diagnostics panel",
    });
    const actions = header.querySelector(".lab-panel-titlebar-actions");
    actions?.appendChild(this.buildIconButton("Hide AI diagnostics", "ai-diagnostics-hide lab-btn", "Hide", { hide: "1" }));

    this.bodyEl = document.createElement("div");
    this.bodyEl.className = "ai-diagnostics-body lab-panel-body";

    this.el.append(header, this.bodyEl, this.windowChrome.renderResizeHandle());

    this.showButton = this.buildIconButton("Show AI diagnostics", "ai-diagnostics-show", "AI", { show: "1" });
    this.showButton.addEventListener("click", this.onShowClick);

    this.root.append(this.el, this.showButton);
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

  handlePanelClick(ev) {
    const target = ev.target instanceof Element ? ev.target : null;
    const btn = target?.closest("button");
    if (!btn || !this.el?.contains(btn)) return;

    if (btn.dataset.hide) {
      ev.preventDefault();
      ev.stopPropagation();
      this.preferences.visible = false;
      this.render();
      return;
    }

    if (btn.dataset.aiDiagnosticsTab) {
      ev.preventDefault();
      ev.stopPropagation();
      this.selectPlayerTab(btn.dataset.aiDiagnosticsTab);
      return;
    }

    if (btn.dataset.aiMapLayer) {
      ev.preventDefault();
      ev.stopPropagation();
      this.toggleMapLayer(btn.dataset.aiMapLayer);
    }
  }

  handleTabKeydown(ev) {
    const target = ev.target instanceof Element ? ev.target : null;
    const tab = target?.closest(".ai-diagnostics-tab");
    if (!tab || !this.el?.contains(tab)) return;
    const key = ev.key;
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(key)) return;

    const tabs = Array.from(this.el.querySelectorAll(".ai-diagnostics-tab"));
    const index = tabs.indexOf(tab);
    if (index < 0 || tabs.length === 0) return;

    ev.preventDefault();
    ev.stopPropagation();
    const nextIndex = key === "Home"
      ? 0
      : key === "End"
        ? tabs.length - 1
        : key === "ArrowLeft"
          ? (index + tabs.length - 1) % tabs.length
          : (index + 1) % tabs.length;
    this.selectPlayerTab(tabs[nextIndex]?.dataset.aiDiagnosticsTab, { focus: true });
  }

  show(ev) {
    ev?.preventDefault?.();
    ev?.stopPropagation?.();
    this.preferences.visible = true;
    this.windowChrome?.setCollapsed(false);
    this.preferences.collapsed = false;
    this.render();
  }

  selectPlayerTab(playerId, { focus = false } = {}) {
    const selectedPlayerId = normalizeSelectedPlayerId(playerId);
    if (selectedPlayerId == null) return;
    this.preferences.selectedPlayerId = selectedPlayerId;
    this.bodySignature = "";
    this.renderBody();
    if (focus) {
      const tab = Array.from(this.el?.querySelectorAll(".ai-diagnostics-tab") || [])
        .find((candidate) => Number(candidate.dataset.aiDiagnosticsTab) === selectedPlayerId);
      tab?.focus?.();
    }
  }

  applyObserverAnalysis(payload) {
    this.analysis = normalizeAiDiagnosticsPanelPayload(payload, this.getPlayers());
    this.publishMapLayerVisibility();
    if (!this.bodyEl || !this.preferences.visible) return;
    this.renderBody();
  }

  render() {
    if (!this.el || !this.bodyEl || !this.showButton) return;
    const visible = this.preferences.visible !== false;

    this.el.classList.toggle("is-hidden", !visible);
    this.el.hidden = !visible;
    this.showButton.hidden = visible;
    this.bodyEl.hidden = !visible;

    if (visible) this.renderBody();
  }

  renderBody() {
    if (!this.bodyEl) return;
    const activeRow = activeAiRow(this.analysis, this.preferences.selectedPlayerId);
    const signature = aiDiagnosticsBodySignature(this.analysis, activeRow?.id);
    if (signature === this.bodySignature) return;
    const previousPlayerId = this.renderedPlayerId;
    const nextPlayerId = activeRow?.id ?? null;
    const scrollState = snapshotAiDiagnosticsScroll(this.bodyEl, previousPlayerId);
    this.bodySignature = signature;

    const body = [
      this.renderStatus(this.analysis),
      this.renderMapSection(this.analysis?.mapAnalysis || null),
    ];

    if (!this.analysis) {
      body.push(renderEmptyState("Waiting for observer analysis"));
    } else if (!this.analysis.rows.length) {
      body.push(renderEmptyState("No AI diagnostics"));
    } else {
      if (activeRow && this.preferences.selectedPlayerId !== activeRow.id) {
        this.preferences.selectedPlayerId = activeRow.id;
      }
      body.push(renderPlayerTabs(this.analysis.rows, activeRow?.id));
      body.push(renderPlayerSection(activeRow));
    }

    this.bodyEl.replaceChildren(...body);
    this.renderedPlayerId = nextPlayerId;
    restoreAiDiagnosticsScroll(this.bodyEl, scrollState, nextPlayerId);
  }

  renderStatus(analysis) {
    const status = document.createElement("div");
    status.className = "ai-diagnostics-status";
    const rows = analysis?.rows || [];
    const lineCount = rows.reduce((total, row) => total + row.aiDiagnostics.lines.length, 0);
    const mapLayerCount = analysis?.mapAnalysis?.layers?.length || 0;
    status.append(
      renderStatusItem("AI players", analysis ? formatValue(rows.length) : "Waiting"),
      renderStatusItem("Trace lines", formatValue(lineCount)),
      renderStatusItem("Latest trace", analysis?.latestTraceTick == null ? "-" : formatValue(analysis.latestTraceTick)),
      renderStatusItem("Map layers", analysis ? formatValue(mapLayerCount) : "Waiting"),
    );
    return status;
  }

  renderMapSection(mapAnalysis) {
    const section = document.createElement("section");
    section.className = "ai-diagnostics-map";

    const header = document.createElement("div");
    header.className = "ai-diagnostics-map-header";
    const title = document.createElement("h3");
    title.textContent = "Map analysis";
    const summary = document.createElement("span");
    summary.textContent = mapAnalysis
      ? `${formatValue(mapAnalysis.primitives)} primitives`
      : "Waiting";
    header.append(title, summary);
    section.appendChild(header);

    const toggles = document.createElement("div");
    toggles.className = "ai-diagnostics-map-toggles";
    const layers = mapAnalysis?.layers || defaultMapLayerRows();
    const visibility = this.preferences.mapLayerVisibility?.(layers.map((layer) => layer.id)) || {};
    for (const layer of layers) {
      toggles.appendChild(renderMapLayerToggle(layer, visibility[layer.id] !== false));
    }
    section.appendChild(toggles);
    return section;
  }

  toggleMapLayer(layerId) {
    const id = normalizeMapLayerId(layerId);
    if (!id) return;
    const layers = this.analysis?.mapAnalysis?.layers?.map((layer) => layer.id) || [];
    const visibility = this.preferences.mapLayerVisibility?.(layers) || {};
    this.preferences.setMapLayerVisible?.(id, visibility[id] === false);
    this.publishMapLayerVisibility();
    this.bodySignature = "";
    this.renderBody();
  }

  mapLayerVisibility() {
    const layers = this.analysis?.mapAnalysis?.layers?.map((layer) => layer.id) || [];
    return this.preferences.mapLayerVisibility?.(layers) || { ...DEFAULT_MAP_LAYER_VISIBILITY };
  }

  publishMapLayerVisibility() {
    this.onMapLayerVisibilityChange?.(this.mapLayerVisibility());
  }

  destroy() {
    this.windowChrome?.destroy();
    if (this.el) {
      this.el.removeEventListener("click", this.onPanelClick);
      this.el.removeEventListener("keydown", this.onTabKeydown);
      this.el.remove();
    }
    if (this.showButton) {
      this.showButton.removeEventListener("click", this.onShowClick);
      this.showButton.remove();
    }
    this.el = null;
    this.bodyEl = null;
    this.showButton = null;
    this.windowChrome = null;
  }
}

export function normalizeAiDiagnosticsPanelPayload(payload, players = []) {
  if (!payload || typeof payload !== "object") return null;
  const metadata = playerMetadata(players);
  const rows = Array.isArray(payload.players)
    ? payload.players.map((player) => normalizeAiDiagnosticsPlayer(player, metadata)).filter(Boolean)
    : [];
  rows.sort((a, b) => a.id - b.id);

  const latestTraceTick = rows.reduce((latest, row) => (
    latest == null ? row.aiDiagnostics.traceTick : Math.max(latest, row.aiDiagnostics.traceTick)
  ), null);

  return {
    rows,
    latestTraceTick,
    mapAnalysis: normalizeMapAnalysisSummary(payload.mapAnalysis),
  };
}

export function normalizeAiDiagnostics(diagnostics) {
  if (!diagnostics || typeof diagnostics !== "object") return null;
  const profileId = String(diagnostics.profileId || "").trim();
  const lines = Array.isArray(diagnostics.lines)
    ? diagnostics.lines.map((line) => String(line || "").trim()).filter(Boolean)
    : [];
  if (!profileId) return null;
  return {
    profileId,
    traceTick: Math.max(0, Math.trunc(Number(diagnostics.traceTick) || 0)),
    lines,
  };
}

export function normalizeMapAnalysisSummary(mapAnalysis) {
  if (!mapAnalysis || typeof mapAnalysis !== "object") return null;
  const layers = Array.isArray(mapAnalysis.layers)
    ? mapAnalysis.layers.map(normalizeMapLayerSummary).filter(Boolean)
    : [];
  const primitives = layers.reduce((total, layer) => total + layer.primitives, 0);
  return {
    mapWidth: Math.max(0, Math.trunc(Number(mapAnalysis.mapWidth) || 0)),
    mapHeight: Math.max(0, Math.trunc(Number(mapAnalysis.mapHeight) || 0)),
    tileSize: Math.max(0, Math.trunc(Number(mapAnalysis.tileSize) || 0)),
    layers: [
      ...layers,
      {
        id: MAP_LABELS_LAYER_ID,
        label: "Labels",
        primitives,
        defaultVisible: true,
      },
    ],
    primitives,
  };
}

function normalizeMapLayerSummary(layer) {
  const id = normalizeMapLayerId(layer?.id);
  if (!id) return null;
  return {
    id,
    label: String(layer?.label || id).slice(0, 24),
    primitives: Array.isArray(layer?.primitives) ? layer.primitives.length : 0,
    defaultVisible: layer?.defaultVisible !== false,
  };
}

function normalizeAiDiagnosticsPlayer(player, metadata) {
  const id = Number(player?.id);
  if (!Number.isFinite(id) || id <= 0) return null;
  const aiDiagnostics = normalizeAiDiagnostics(player.aiDiagnostics);
  const meta = metadata.get(id) || {};
  if (!aiDiagnostics && !meta.isAi) return null;
  return {
    id,
    name: meta.name || `Player ${id}`,
    color: safeCssColor(meta.color || "#e7dfc5"),
    aiDiagnostics: aiDiagnostics || {
      profileId: meta.aiProfileId || "AI",
      traceTick: 0,
      lines: [],
    },
  };
}

function playerMetadata(players) {
  const metadata = new Map();
  for (const player of players || []) {
    const id = Number(player?.id);
    if (!Number.isFinite(id) || id <= 0) continue;
    metadata.set(id, {
      name: player?.name || `Player ${id}`,
      color: player?.color || "#e7dfc5",
      isAi: player?.isAi === true || player?.is_ai === true || Boolean(player?.aiProfileId || player?.ai_profile_id),
      aiProfileId: String(player?.aiProfileId || player?.ai_profile_id || "").trim(),
    });
  }
  return metadata;
}

function renderStatusItem(label, value) {
  const item = document.createElement("div");
  item.className = "ai-diagnostics-status-item";
  const labelEl = document.createElement("span");
  labelEl.className = "ai-diagnostics-status-label";
  labelEl.textContent = label;
  const valueEl = document.createElement("strong");
  valueEl.className = "ai-diagnostics-status-value";
  valueEl.textContent = value;
  item.append(labelEl, valueEl);
  return item;
}

function renderMapLayerToggle(layer, visible) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "ai-diagnostics-map-toggle";
  button.dataset.aiMapLayer = layer.id;
  button.setAttribute("role", "switch");
  button.setAttribute("aria-checked", String(visible));
  button.classList.toggle("active", visible);

  const indicator = document.createElement("span");
  indicator.className = "ai-diagnostics-map-toggle-indicator";
  indicator.setAttribute("aria-hidden", "true");

  const label = document.createElement("span");
  label.className = "ai-diagnostics-map-toggle-label";
  label.textContent = layer.label || layer.id;

  const count = document.createElement("span");
  count.className = "ai-diagnostics-map-toggle-count";
  count.textContent = formatValue(layer.primitives || 0);

  button.append(indicator, label, count);
  return button;
}

function renderEmptyState(text) {
  const empty = document.createElement("div");
  empty.className = "ai-diagnostics-empty";
  empty.textContent = text;
  return empty;
}

function renderPlayerTabs(rows, activePlayerId) {
  const tabs = document.createElement("div");
  tabs.className = "ai-diagnostics-tabs";
  tabs.setAttribute("role", "tablist");
  tabs.setAttribute("aria-label", "AI players");

  for (const row of rows) {
    const selected = row.id === activePlayerId;
    const button = document.createElement("button");
    button.type = "button";
    button.className = "ai-diagnostics-tab";
    button.dataset.aiDiagnosticsTab = String(row.id);
    button.setAttribute("role", "tab");
    button.setAttribute("aria-selected", String(selected));
    button.tabIndex = selected ? 0 : -1;
    if (selected) button.classList.add("active");

    const swatch = document.createElement("span");
    swatch.className = "ai-diagnostics-tab-swatch";
    swatch.setAttribute("style", `background:${safeCssColor(row.color)};`);
    swatch.setAttribute("aria-hidden", "true");

    const label = document.createElement("span");
    label.className = "ai-diagnostics-tab-label";
    label.textContent = row.name;

    const tick = document.createElement("span");
    tick.className = "ai-diagnostics-tab-tick";
    tick.textContent = `t${formatValue(row.aiDiagnostics.traceTick)}`;

    button.append(swatch, label, tick);
    tabs.appendChild(button);
  }

  return tabs;
}

function renderPlayerSection(row) {
  const section = document.createElement("section");
  section.className = "ai-diagnostics-player";
  section.setAttribute("role", "tabpanel");

  const header = document.createElement("div");
  header.className = "ai-diagnostics-player-header";

  const swatch = document.createElement("span");
  swatch.className = "ai-diagnostics-player-swatch";
  swatch.setAttribute("style", `background:${safeCssColor(row.color)};`);
  swatch.setAttribute("aria-hidden", "true");

  const identity = document.createElement("div");
  identity.className = "ai-diagnostics-player-identity";
  const name = document.createElement("h3");
  name.textContent = row.name;
  const profile = document.createElement("span");
  profile.textContent = row.aiDiagnostics.profileId;
  identity.append(name, profile);

  const tick = document.createElement("div");
  tick.className = "ai-diagnostics-player-tick";
  tick.textContent = `tick ${formatValue(row.aiDiagnostics.traceTick)}`;

  header.append(swatch, identity, tick);
  section.appendChild(header);

  const trace = document.createElement("div");
  trace.className = "ai-diagnostics-trace";
  if (row.aiDiagnostics.lines.length > 0) {
    row.aiDiagnostics.lines.forEach((line, index) => {
      trace.appendChild(renderTraceLine(line, index));
    });
  } else {
    trace.appendChild(renderEmptyState("No trace lines for this AI"));
  }
  section.appendChild(trace);

  return section;
}

function renderTraceLine(lineText, index) {
  const row = document.createElement("div");
  row.className = "ai-diagnostics-trace-row";

  const number = document.createElement("span");
  number.className = "ai-diagnostics-trace-index";
  number.textContent = String(index + 1).padStart(2, "0");

  const content = document.createElement("div");
  content.className = "ai-diagnostics-trace-content";
  content.title = lineText;

  const parsed = parseTraceFields(lineText);
  if (parsed.fields.length >= 2) {
    const fields = document.createElement("div");
    fields.className = "ai-diagnostics-trace-fields";
    for (const field of parsed.fields) {
      fields.appendChild(renderTraceField(field));
    }
    content.appendChild(fields);
    if (parsed.rest) {
      const rest = document.createElement("div");
      rest.className = "ai-diagnostics-trace-raw";
      rest.textContent = parsed.rest;
      content.appendChild(rest);
    }
  } else {
    const raw = document.createElement("div");
    raw.className = "ai-diagnostics-trace-raw";
    raw.textContent = lineText;
    content.appendChild(raw);
  }

  row.append(number, content);
  return row;
}

function renderTraceField(field) {
  const wrap = document.createElement("span");
  wrap.className = "ai-diagnostics-field";

  const key = document.createElement("span");
  key.className = "ai-diagnostics-field-key";
  key.textContent = field.key;

  const value = document.createElement("span");
  value.className = "ai-diagnostics-field-value";
  value.textContent = field.value || "-";

  wrap.append(key, value);
  return wrap;
}

function parseTraceFields(lineText) {
  const fields = [];
  const rest = [];
  for (const part of String(lineText || "").split(/\s+/).filter(Boolean)) {
    const index = part.indexOf("=");
    if (index > 0) {
      fields.push({
        key: part.slice(0, index),
        value: part.slice(index + 1),
      });
    } else {
      rest.push(part);
    }
  }
  return { fields, rest: rest.join(" ") };
}

function activeAiRow(analysis, selectedPlayerId) {
  const rows = analysis?.rows || [];
  if (rows.length === 0) return null;
  const selected = normalizeSelectedPlayerId(selectedPlayerId);
  return rows.find((row) => row.id === selected) || rows[0];
}

function aiDiagnosticsBodySignature(analysis, activePlayerId) {
  if (!analysis) return "waiting";
  return [
    analysis.latestTraceTick ?? "",
    activePlayerId ?? "",
    mapAnalysisSignature(analysis.mapAnalysis),
    ...analysis.rows.map((row) => [
      row.id,
      row.name,
      safeCssColor(row.color),
      row.aiDiagnostics.profileId,
      row.aiDiagnostics.traceTick,
      row.aiDiagnostics.lines.join("\n"),
    ].join(":")),
  ].join("|");
}

function snapshotAiDiagnosticsScroll(bodyEl, activePlayerId) {
  if (!bodyEl) return null;
  const playerEl = bodyEl.querySelector?.(".ai-diagnostics-player");
  const tabsEl = bodyEl.querySelector?.(".ai-diagnostics-tabs");
  return {
    activePlayerId: normalizeSelectedPlayerId(activePlayerId),
    bodyTop: scrollNumber(bodyEl.scrollTop),
    bodyLeft: scrollNumber(bodyEl.scrollLeft),
    playerTop: scrollNumber(playerEl?.scrollTop),
    playerLeft: scrollNumber(playerEl?.scrollLeft),
    tabsLeft: scrollNumber(tabsEl?.scrollLeft),
  };
}

function restoreAiDiagnosticsScroll(bodyEl, scrollState, activePlayerId) {
  if (!bodyEl || !scrollState) return;
  if (scrollState.activePlayerId !== normalizeSelectedPlayerId(activePlayerId)) return;
  bodyEl.scrollTop = scrollState.bodyTop;
  bodyEl.scrollLeft = scrollState.bodyLeft;
  const playerEl = bodyEl.querySelector?.(".ai-diagnostics-player");
  if (playerEl) {
    playerEl.scrollTop = scrollState.playerTop;
    playerEl.scrollLeft = scrollState.playerLeft;
  }
  const tabsEl = bodyEl.querySelector?.(".ai-diagnostics-tabs");
  if (tabsEl) tabsEl.scrollLeft = scrollState.tabsLeft;
}

function scrollNumber(value) {
  return Math.max(0, Number(value) || 0);
}

function formatValue(value) {
  return String(Math.max(0, Math.round(Number(value) || 0)));
}

function defaultMapLayerRows() {
  return [
    { id: "chokes", label: "Chokes", primitives: 0, defaultVisible: true },
    { id: "bases", label: "Bases", primitives: 0, defaultVisible: true },
    { id: "resources", label: "Resources", primitives: 0, defaultVisible: true },
    { id: MAP_LABELS_LAYER_ID, label: "Labels", primitives: 0, defaultVisible: true },
  ];
}

function mapAnalysisSignature(mapAnalysis) {
  if (!mapAnalysis) return "map:waiting";
  return [
    mapAnalysis.mapWidth,
    mapAnalysis.mapHeight,
    mapAnalysis.tileSize,
    ...mapAnalysis.layers.map((layer) => `${layer.id}:${layer.label}:${layer.primitives}:${layer.defaultVisible}`),
  ].join("|");
}

function safeCssColor(color) {
  return typeof color === "string" && /^#[0-9a-fA-F]{3,8}$/.test(color) ? color : "#e7dfc5";
}

function normalizePreferences(state) {
  state.visible = state.visible !== false;
  state.collapsed = state.collapsed === true;
  state.selectedPlayerId = normalizeSelectedPlayerId(state.selectedPlayerId);
  state.mapLayers = normalizeMapLayerVisibility(state.mapLayers);
}

function normalizeSelectedPlayerId(value) {
  const playerId = Math.trunc(Number(value));
  return Number.isFinite(playerId) && playerId > 0 ? playerId : null;
}

function normalizeMapLayerVisibility(value, layerIds = []) {
  const knownIds = new Set(Object.keys(DEFAULT_MAP_LAYER_VISIBILITY));
  for (const layerId of layerIds) {
    const id = normalizeMapLayerId(layerId);
    if (id && !isRetiredMapLayerId(id)) knownIds.add(id);
  }

  const normalized = { ...DEFAULT_MAP_LAYER_VISIBILITY };
  for (const id of knownIds) {
    if (!hasOwn(normalized, id)) normalized[id] = true;
  }
  if (value && typeof value === "object") {
    for (const [key, visible] of Object.entries(value)) {
      const id = normalizeMapLayerId(key);
      if (!id || isRetiredMapLayerId(id)) continue;
      normalized[id] = visible === true;
    }
  }
  return normalized;
}

function ensureKnownMapLayers(state, layerIds = []) {
  state.mapLayers = normalizeMapLayerVisibility(state.mapLayers, layerIds);
  if (!hasOwn(state.mapLayers, MAP_LABELS_LAYER_ID)) {
    state.mapLayers[MAP_LABELS_LAYER_ID] = true;
  }
}

function normalizeMapLayerId(value) {
  const id = String(value || "").trim();
  return MAP_LAYER_ID_RE.test(id) ? id : "";
}

function isRetiredMapLayerId(id) {
  return RETIRED_MAP_LAYER_IDS.has(id);
}

function hasOwn(object, key) {
  return Object.prototype.hasOwnProperty.call(object, key);
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
    // Storage failures should not break observer diagnostics.
  }
}
