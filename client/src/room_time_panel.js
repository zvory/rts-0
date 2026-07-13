import { createImmediateTouchButtonActivation } from "./panel_touch_activation.js";
import {
  FloatingPanelPositioner,
  isMobileDebugPanelViewport,
} from "./floating_panel_positioner.js";

export { isMobileDebugPanelViewport } from "./floating_panel_positioner.js";

const ROOM_TIME_PANEL_STORAGE_KEY = "rts.roomTimeControls.panel.v1";
const ROOM_TIME_PANEL_DEFAULT_LEFT = 12;
const ROOM_TIME_PANEL_DEFAULT_TOP = 70;
const ROOM_TIME_PANEL_DEFAULT_WIDTH = 420;
const ROOM_TIME_PANEL_DEFAULT_HEIGHT = 120;
const ROOM_TIME_PANEL_STORAGE_SCHEMA_VERSION = 1;

export class FloatingRoomTimePanel {
  constructor({ root, label }) {
    this.root = root;
    this.label = label || "Room time";
    this.contentEl = null;
    this.renderListeners = [];
    this.collapsed = false;
    this.collapseButton = null;
    this.collapseActivation = createImmediateTouchButtonActivation(() => this.toggleCollapsed());
    this.positioner = new FloatingPanelPositioner({
      root: this.root,
      defaultPosition: { left: ROOM_TIME_PANEL_DEFAULT_LEFT, top: ROOM_TIME_PANEL_DEFAULT_TOP },
      defaultSize: { width: ROOM_TIME_PANEL_DEFAULT_WIDTH, height: ROOM_TIME_PANEL_DEFAULT_HEIGHT },
      readPosition: () => storedPanelPosition(this.readPosition()),
      savePosition: (position) => this.savePosition(position),
      clearPosition: () => this.removeStoredPosition(),
      isMobileViewport: isMobileDebugPanelViewport,
      canConstrain: () => !this.root?.hidden,
      onReset: () => this.setCollapsed(false, { save: false }),
    });
  }

  mount() {
    if (!this.root || !globalThis.document?.createElement) return null;
    this.root.classList.add("room-time-floating-panel");

    let body = this.root.querySelector(".room-time-panel-body");
    let dragHandle = this.root.querySelector(".room-time-panel-drag-handle");
    let collapse = this.root.querySelector(".room-time-panel-collapse");

    if (!body || !dragHandle || !collapse) {
      const existing = Array.from(this.root.children || []);
      const header = document.createElement("div");
      header.className = "room-time-panel-titlebar";

      dragHandle = document.createElement("div");
      dragHandle.className = "room-time-panel-drag-handle";
      dragHandle.tabIndex = 0;
      dragHandle.setAttribute("role", "button");
      dragHandle.setAttribute("aria-keyshortcuts", "ArrowUp ArrowDown ArrowLeft ArrowRight Home");
      dragHandle.title = "Drag to move. Arrow keys nudge. Home resets.";

      const grip = document.createElement("span");
      grip.className = "room-time-panel-grip";
      grip.setAttribute("aria-hidden", "true");
      grip.textContent = "::::";

      const title = document.createElement("strong");
      title.className = "room-time-panel-title";
      dragHandle.appendChild(grip);
      dragHandle.appendChild(title);

      collapse = document.createElement("button");
      collapse.type = "button";
      collapse.className = "room-time-panel-collapse";

      body = document.createElement("div");
      body.className = "room-time-panel-body";
      for (const child of existing) body.appendChild(child);

      const actions = document.createElement("div");
      actions.className = "room-time-panel-actions";
      actions.appendChild(collapse);

      header.appendChild(dragHandle);
      header.appendChild(actions);
      this.root.replaceChildren(header, body);
    }

    this.collapseButton = collapse;
    this.syncLabels();
    this.contentEl = body;
    this.listenRender(collapse, "pointerdown", this.collapseActivation.pointerdown);
    this.listenRender(collapse, "pointerup", this.collapseActivation.pointerup);
    this.listenRender(collapse, "pointercancel", this.collapseActivation.pointercancel);
    this.listenRender(collapse, "pointerleave", this.collapseActivation.pointerleave);
    this.listenRender(collapse, "click", this.collapseActivation.click);
    this.restorePosition();
    this.positioner.mount(dragHandle, { restore: false });
    return this.contentEl;
  }

  syncLabels() {
    const dragHandle = this.root?.querySelector(".room-time-panel-drag-handle");
    const collapse = this.root?.querySelector(".room-time-panel-collapse");
    const title = this.root?.querySelector(".room-time-panel-title");
    if (title) title.textContent = this.label;
    dragHandle?.setAttribute("aria-label", `Move ${this.label.toLowerCase()} controls`);
    if (collapse) {
      const action = this.collapsed ? "Expand" : "Collapse";
      collapse.textContent = action;
      collapse.title = `${action} ${this.label.toLowerCase()} controls`;
      collapse.setAttribute("aria-label", `${action} ${this.label.toLowerCase()} controls`);
      collapse.setAttribute("aria-expanded", this.collapsed ? "false" : "true");
    }
    if (this.contentEl) this.contentEl.hidden = this.collapsed;
    if (this.root?.dataset) this.root.dataset.collapsed = this.collapsed ? "true" : "false";
  }

  destroy() {
    this.positioner.destroy();
    this.collapseActivation.reset();
    this.clearRenderListeners();
    const body = this.root?.querySelector(".room-time-panel-body");
    if (this.root && body) {
      const controls = Array.from(body.children || []);
      this.root.replaceChildren(...controls);
    }
    this.root?.classList?.remove("room-time-floating-panel");
    if (this.root?.dataset) delete this.root.dataset.panelDragging;
    if (this.root?.dataset) delete this.root.dataset.collapsed;
    this.contentEl = null;
    this.collapseButton = null;
  }

  listenRender(target, type, handler) {
    target?.addEventListener?.(type, handler);
    this.renderListeners.push([target, type, handler]);
  }

  clearRenderListeners() {
    for (const [target, type, handler] of this.renderListeners) {
      target?.removeEventListener?.(type, handler);
    }
    this.renderListeners = [];
  }

  restorePosition() {
    const saved = this.readPosition();
    this.setCollapsed(saved?.collapsed === true, { save: false });
    this.positioner.restorePosition();
  }

  resetPosition() {
    this.positioner.resetPosition();
  }

  constrainToViewport() {
    this.positioner.constrainToViewport();
  }

  currentRect() {
    return this.positioner.currentRect();
  }

  constrainPosition(position) {
    return this.positioner.constrainPosition(position);
  }

  applyPosition(position) {
    this.positioner.applyPosition(position);
  }

  clearPositionStyles() {
    this.positioner.clearPositionStyles();
  }

  hasPositionStyles() {
    return this.positioner.hasPositionStyles();
  }

  toggleCollapsed() {
    this.setCollapsed(!this.collapsed);
  }

  setCollapsed(collapsed, options = {}) {
    this.collapsed = !!collapsed;
    this.syncLabels();
    if (options.save !== false) this.savePosition(this.currentRect());
  }

  readPosition() {
    try {
      const raw = globalThis.window?.localStorage?.getItem?.(ROOM_TIME_PANEL_STORAGE_KEY);
      if (!raw) return null;
      const parsed = JSON.parse(raw);
      if (parsed?.schemaVersion !== ROOM_TIME_PANEL_STORAGE_SCHEMA_VERSION) return null;
      const left = Number(parsed.left);
      const top = Number(parsed.top);
      const position = Number.isFinite(left) && Number.isFinite(top) ? { left, top } : null;
      return {
        ...(position || {}),
        collapsed: parsed.collapsed === true,
      };
    } catch {
      return null;
    }
  }

  savePosition(position) {
    try {
      if (isMobileDebugPanelViewport()) {
        this.saveCollapsedState();
        return;
      }
      const next = this.constrainPosition(position);
      globalThis.window?.localStorage?.setItem?.(ROOM_TIME_PANEL_STORAGE_KEY, JSON.stringify({
        schemaVersion: ROOM_TIME_PANEL_STORAGE_SCHEMA_VERSION,
        left: next.left,
        top: next.top,
        collapsed: this.collapsed,
      }));
    } catch {
      // Room-time panel position is a convenience only.
    }
  }

  saveCollapsedState() {
    try {
      const storedPosition = storedPanelPosition(this.readPosition());
      globalThis.window?.localStorage?.setItem?.(ROOM_TIME_PANEL_STORAGE_KEY, JSON.stringify({
        schemaVersion: ROOM_TIME_PANEL_STORAGE_SCHEMA_VERSION,
        ...(storedPosition || {}),
        collapsed: this.collapsed,
      }));
    } catch {
      // Ignore unavailable storage.
    }
  }

  removeStoredPosition() {
    try {
      globalThis.window?.localStorage?.removeItem?.(ROOM_TIME_PANEL_STORAGE_KEY);
    } catch {
      // Ignore unavailable storage.
    }
  }
}

function storedPanelPosition(saved) {
  return Number.isFinite(saved?.left) && Number.isFinite(saved?.top)
    ? { left: saved.left, top: saved.top }
    : null;
}
