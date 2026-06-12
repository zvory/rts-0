export class SettingsContainer {
  constructor({ button, menu, title = "Settings" } = {}) {
    this.button = button || null;
    this.menu = menu || null;
    this.title = title;
    this.context = {};
    this.tabs = [];
    this.activeTabId = "";
    this.activePanelDestroy = null;
    this._returnFocus = null;

    this.onButtonClick = () => this.toggle();
    this.onKeyDown = (ev) => this.handleKeyDown(ev);

    this.button?.addEventListener("click", this.onButtonClick);
    window.addEventListener("keydown", this.onKeyDown, true);
    this.close();
  }

  setContext(context = {}) {
    this.context = context;
    const kind = context.kind || "lobby";
    this.button?.parentElement?.setAttribute("data-settings-context", kind);
    this.setTabs(context.tabs || []);
  }

  setTabs(tabs = []) {
    this.tabs = tabs.filter((tab) => tab && tab.id && tab.visible !== false);
    if (!this.tabs.some((tab) => tab.id === this.activeTabId)) {
      this.activeTabId = this.tabs[0]?.id || "";
    }
    this.render();
  }

  open({ focus = true } = {}) {
    if (!this.menu || !this.tabs.length) return;
    this._returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : this.button;
    this.render();
    this.menu.hidden = false;
    this.button?.setAttribute("aria-expanded", "true");
    if (focus) this.firstFocusable()?.focus();
  }

  close({ restoreFocus = false } = {}) {
    if (!this.menu) return;
    this.menu.hidden = true;
    this.button?.setAttribute("aria-expanded", "false");
    if (restoreFocus && this._returnFocus instanceof HTMLElement) this._returnFocus.focus();
    this._returnFocus = null;
  }

  toggle() {
    if (!this.menu) return;
    if (this.menu.hidden) this.open();
    else this.close({ restoreFocus: true });
  }

  isOpen() {
    return !!this.menu && !this.menu.hidden;
  }

  activateTab(id) {
    if (!this.tabs.some((tab) => tab.id === id)) return;
    this.activeTabId = id;
    this.render();
    this.menu?.querySelector(`[data-settings-tab="${CSS.escape(id)}"]`)?.focus();
  }

  render() {
    if (!this.menu) return;
    this.destroyActivePanel();
    this.menu.replaceChildren();
    if (!this.tabs.length) return;

    const header = document.createElement("div");
    header.className = "settings-header";
    const title = document.createElement("h2");
    title.id = "settings-title";
    title.textContent = this.title;
    const actions = document.createElement("div");
    actions.className = "settings-actions";
    for (const action of this.context.actions || []) {
      const el = action?.render?.(this.context);
      if (el) actions.appendChild(el);
    }
    header.append(title, actions);

    const tabList = document.createElement("div");
    tabList.className = "settings-tabs";
    tabList.setAttribute("role", "tablist");
    for (const tab of this.tabs) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "settings-tab";
      button.dataset.settingsTab = tab.id;
      button.setAttribute("role", "tab");
      button.setAttribute("aria-selected", String(tab.id === this.activeTabId));
      button.textContent = tab.label || tab.id;
      button.addEventListener("click", () => this.activateTab(tab.id));
      tabList.appendChild(button);
    }

    const panel = document.createElement("div");
    panel.className = "settings-panel";
    panel.setAttribute("role", "tabpanel");
    const active = this.tabs.find((tab) => tab.id === this.activeTabId) || this.tabs[0];
    this.activeTabId = active?.id || "";
    const cleanup = active?.render?.(panel, this.context);
    this.activePanelDestroy = typeof cleanup === "function" ? cleanup : null;

    this.menu.append(header, tabList, panel);
  }

  firstFocusable() {
    return this.menu?.querySelector("button, input, select, textarea, [tabindex]:not([tabindex='-1'])") || null;
  }

  handleKeyDown(ev) {
    if (ev.code !== "Escape" || ev.repeat || !this.isOpen()) return;
    ev.preventDefault();
    ev.stopPropagation();
    this.close({ restoreFocus: true });
  }

  destroyActivePanel() {
    if (!this.activePanelDestroy) return;
    try {
      this.activePanelDestroy();
    } finally {
      this.activePanelDestroy = null;
    }
  }

  destroy() {
    this.destroyActivePanel();
    this.button?.removeEventListener("click", this.onButtonClick);
    window.removeEventListener("keydown", this.onKeyDown, true);
    this.menu?.replaceChildren();
  }
}
