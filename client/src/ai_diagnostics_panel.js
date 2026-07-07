const STORAGE_KEY = "rts.aiDiagnosticsPanel";

export function shouldMountAiDiagnosticsPanel({ capabilities } = {}) {
  return capabilities?.diagnostics?.observerAnalysis === true;
}

export function createAiDiagnosticsPanelPreferences(storage = safeLocalStorage()) {
  const fallback = {
    visible: true,
    collapsed: false,
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
    snapshot() {
      return { ...state };
    },
  };
}

export class AiDiagnosticsPanel {
  constructor({
    root,
    preferences = createAiDiagnosticsPanelPreferences(),
    getPlayers = () => [],
  }) {
    this.root = root;
    this.preferences = preferences;
    this.getPlayers = getPlayers;
    this.analysis = null;
    this.el = null;
    this.panel = null;
    this.bodyEl = null;
    this.showButton = null;
    this.bodySignature = "";
    this.onClick = (ev) => this.handleClick(ev);
    this.mount();
  }

  mount() {
    if (!this.root || this.el) return;

    this.el = document.createElement("aside");
    this.el.className = "ai-diagnostics-panel-host";
    this.el.setAttribute("aria-label", "AI diagnostics");
    this.el.addEventListener("click", this.onClick);

    this.panel = document.createElement("section");
    this.panel.className = "ai-diagnostics-panel hud-panel";

    const header = document.createElement("div");
    header.className = "ai-diagnostics-header";

    const title = document.createElement("h2");
    title.textContent = "AI Diagnostics";
    header.appendChild(title);

    const actions = document.createElement("div");
    actions.className = "ai-diagnostics-actions";
    actions.append(
      this.buildIconButton("Collapse AI diagnostics", "ai-diagnostics-collapse", "-", { collapse: "1" }),
      this.buildIconButton("Hide AI diagnostics", "ai-diagnostics-hide", "x", { hide: "1" }),
    );
    header.appendChild(actions);

    this.bodyEl = document.createElement("div");
    this.bodyEl.className = "ai-diagnostics-body";

    this.panel.append(header, this.bodyEl);
    this.el.appendChild(this.panel);

    this.showButton = this.buildIconButton("Show AI diagnostics", "ai-diagnostics-show", "AI", { show: "1" });
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

    if (btn.dataset.collapse) {
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

  applyObserverAnalysis(payload) {
    this.analysis = normalizeAiDiagnosticsPanelPayload(payload, this.getPlayers());
    this.render();
  }

  render() {
    if (!this.el || !this.panel || !this.bodyEl || !this.showButton) return;
    const visible = this.preferences.visible !== false;
    const collapsed = this.preferences.collapsed === true;

    this.el.classList.toggle("is-hidden", !visible);
    this.el.classList.toggle("is-collapsed", visible && collapsed);
    this.panel.hidden = !visible;
    this.showButton.hidden = visible;
    this.bodyEl.hidden = !visible || collapsed;

    const collapse = this.panel.querySelector(".ai-diagnostics-collapse");
    if (collapse) {
      collapse.textContent = collapsed ? "+" : "-";
      collapse.title = collapsed ? "Expand AI diagnostics" : "Collapse AI diagnostics";
      collapse.setAttribute("aria-label", collapse.title);
      collapse.setAttribute("aria-expanded", String(!collapsed));
    }

    if (visible && !collapsed) this.renderBody();
  }

  renderBody() {
    if (!this.bodyEl) return;
    const signature = aiDiagnosticsBodySignature(this.analysis);
    if (signature === this.bodySignature) return;
    this.bodySignature = signature;

    const body = [this.renderStatus(this.analysis)];

    if (!this.analysis) {
      body.push(renderEmptyState("Waiting for observer analysis"));
    } else if (!this.analysis.rows.length) {
      body.push(renderEmptyState("No AI diagnostics"));
    } else {
      const list = document.createElement("div");
      list.className = "ai-diagnostics-player-list";
      for (const row of this.analysis.rows) {
        list.appendChild(renderPlayerSection(row));
      }
      body.push(list);
    }

    this.bodyEl.replaceChildren(...body);
  }

  renderStatus(analysis) {
    const status = document.createElement("div");
    status.className = "ai-diagnostics-status";
    if (!analysis) {
      status.append(
        renderStatusItem("Observer", "Waiting"),
        renderStatusItem("Traces", "0"),
        renderStatusItem("Latest trace", "-"),
      );
      return status;
    }

    status.append(
      renderStatusItem("Observer tick", formatValue(analysis.tick)),
      renderStatusItem("Traces", formatValue(analysis.rows.length)),
      renderStatusItem("Latest trace", analysis.latestTraceTick == null ? "-" : formatValue(analysis.latestTraceTick)),
    );
    return status;
  }

  destroy() {
    if (this.el) {
      this.el.removeEventListener("click", this.onClick);
      this.el.remove();
    }
    this.el = null;
    this.panel = null;
    this.bodyEl = null;
    this.showButton = null;
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
    tick: Math.max(0, Math.trunc(Number(payload.tick) || 0)),
    rows,
    latestTraceTick,
  };
}

export function normalizeAiDiagnostics(diagnostics) {
  if (!diagnostics || typeof diagnostics !== "object") return null;
  const profileId = String(diagnostics.profileId || "").trim();
  const lines = Array.isArray(diagnostics.lines)
    ? diagnostics.lines.map((line) => String(line || "").trim()).filter(Boolean)
    : [];
  if (!profileId || lines.length === 0) return null;
  return {
    profileId,
    traceTick: Math.max(0, Math.trunc(Number(diagnostics.traceTick) || 0)),
    lines,
  };
}

function normalizeAiDiagnosticsPlayer(player, metadata) {
  const id = Number(player?.id);
  if (!Number.isFinite(id) || id <= 0) return null;
  const aiDiagnostics = normalizeAiDiagnostics(player.aiDiagnostics);
  if (!aiDiagnostics) return null;
  const meta = metadata.get(id) || {};
  return {
    id,
    name: meta.name || `Player ${id}`,
    color: safeCssColor(meta.color || "#e7dfc5"),
    aiDiagnostics,
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

function renderEmptyState(text) {
  const empty = document.createElement("div");
  empty.className = "ai-diagnostics-empty";
  empty.textContent = text;
  return empty;
}

function renderPlayerSection(row) {
  const section = document.createElement("section");
  section.className = "ai-diagnostics-player";

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
  row.aiDiagnostics.lines.forEach((line, index) => {
    trace.appendChild(renderTraceLine(line, index));
  });
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

function aiDiagnosticsBodySignature(analysis) {
  if (!analysis) return "waiting";
  return [
    analysis.tick,
    analysis.latestTraceTick ?? "",
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

function formatValue(value) {
  return String(Math.max(0, Math.round(Number(value) || 0)));
}

function safeCssColor(color) {
  return typeof color === "string" && /^#[0-9a-fA-F]{3,8}$/.test(color) ? color : "#e7dfc5";
}

function normalizePreferences(state) {
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
    // Storage failures should not break observer diagnostics.
  }
}
