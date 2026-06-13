const STORAGE_KEY = "rts.replayAnalysisOverlay";

export const REPLAY_ANALYSIS_TABS = Object.freeze([
  { id: "army-value", label: "Army value" },
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
  constructor({ root, preferences = createReplayAnalysisOverlayPreferences() }) {
    this.root = root;
    this.preferences = preferences;
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
    this.bodyEl.replaceChildren(this.renderPlaceholder(tab));
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
