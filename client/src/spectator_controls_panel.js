import {
  FloatingPanelPositioner,
  isMobileDebugPanelViewport,
  panelViewport,
} from "./floating_panel_positioner.js";
import { createImmediateTouchButtonActivation } from "./panel_touch_activation.js";

const STORAGE_KEY = "rts.spectatorControls.panel.v1";
const STORAGE_SCHEMA_VERSION = 1;
const PANEL_WIDTH = 292;
const PANEL_HEIGHT = 92;
const PANEL_MARGIN = 12;
const PANEL_TOP = 58;

export class SpectatorControlsPanel {
  constructor({ root, state, onToggle } = {}) {
    this.host = root || null;
    this.state = typeof state === "function" ? state : () => ({});
    this.onToggle = onToggle;
    this.root = null;
    this.toggle = null;
    this.toggleActivation = createImmediateTouchButtonActivation(() => this.toggleEnabled());
    this.toggleListeners = [];
    this.positioner = null;
    this.mount();
  }

  mount() {
    if (!this.host || !globalThis.document?.createElement) return;

    const root = document.createElement("section");
    root.className = "spectator-controls-panel hud-panel";
    root.setAttribute("aria-label", "Spectator controls");

    const dragHandle = document.createElement("div");
    dragHandle.className = "spectator-controls-drag-handle";
    dragHandle.tabIndex = 0;
    dragHandle.setAttribute("role", "button");
    dragHandle.setAttribute("aria-label", "Move spectator controls");
    dragHandle.setAttribute("aria-keyshortcuts", "ArrowUp ArrowDown ArrowLeft ArrowRight Home");
    dragHandle.title = "Drag to move. Arrow keys nudge. Home resets.";

    const grip = document.createElement("span");
    grip.className = "spectator-controls-grip";
    grip.setAttribute("aria-hidden", "true");
    grip.textContent = "::::";

    const title = document.createElement("strong");
    title.className = "spectator-controls-title";
    title.textContent = "Spectator Controls";
    dragHandle.append(grip, title);

    const row = document.createElement("div");
    row.className = "spectator-controls-row";

    const label = document.createElement("span");
    label.id = "spectator-controls-fight-label";
    label.className = "spectator-controls-label";
    label.textContent = "Follow active fights";

    const toggle = document.createElement("button");
    toggle.id = "auto-spectator-toggle";
    toggle.type = "button";
    toggle.className = "spectator-controls-toggle";
    toggle.setAttribute("role", "switch");
    toggle.setAttribute("aria-labelledby", label.id);
    toggle.title = "Automatically frame active battles and widen the view when combat is quiet.";

    row.append(label, toggle);
    root.append(dragHandle, row);
    this.host.appendChild(root);
    this.root = root;
    this.toggle = toggle;

    this.bindToggle(toggle);
    this.positioner = new FloatingPanelPositioner({
      root,
      defaultPosition: defaultPanelPosition(),
      defaultSize: { width: PANEL_WIDTH, height: PANEL_HEIGHT },
      readPosition: () => this.readPosition(),
      savePosition: (position) => this.savePosition(position),
      clearPosition: () => this.clearPosition(),
      isMobileViewport: isMobileDebugPanelViewport,
      canConstrain: () => !!this.root?.isConnected,
    });
    this.positioner.mount(dragHandle);
    this.sync();
  }

  bindToggle(toggle) {
    const listeners = [
      ["pointerdown", this.toggleActivation.pointerdown],
      ["pointerup", this.toggleActivation.pointerup],
      ["pointercancel", this.toggleActivation.pointercancel],
      ["pointerleave", this.toggleActivation.pointerleave],
      ["click", this.toggleActivation.click],
    ];
    for (const [type, handler] of listeners) toggle.addEventListener(type, handler);
    this.toggleListeners = listeners;
  }

  toggleEnabled() {
    const current = this.state() || {};
    if (current.available === false) return;
    this.onToggle?.(!current.enabled);
    this.sync();
  }

  sync() {
    if (!this.toggle) return;
    const current = this.state() || {};
    const enabled = !!current.enabled;
    this.toggle.disabled = current.available === false;
    this.toggle.setAttribute("aria-checked", String(enabled));
    this.toggle.textContent = enabled ? "On" : "Off";
  }

  handleViewportChange() {
    this.positioner?.constrainToViewport();
  }

  destroy() {
    this.positioner?.destroy();
    this.toggleActivation.reset();
    for (const [type, handler] of this.toggleListeners) {
      this.toggle?.removeEventListener?.(type, handler);
    }
    this.toggleListeners = [];
    this.root?.remove?.();
    this.positioner = null;
    this.toggle = null;
    this.root = null;
  }

  readPosition() {
    try {
      const raw = globalThis.window?.localStorage?.getItem?.(STORAGE_KEY);
      if (!raw) return null;
      const parsed = JSON.parse(raw);
      if (parsed?.schemaVersion !== STORAGE_SCHEMA_VERSION) return null;
      const left = Number(parsed.left);
      const top = Number(parsed.top);
      return Number.isFinite(left) && Number.isFinite(top) ? { left, top } : null;
    } catch {
      return null;
    }
  }

  savePosition(position) {
    try {
      globalThis.window?.localStorage?.setItem?.(STORAGE_KEY, JSON.stringify({
        schemaVersion: STORAGE_SCHEMA_VERSION,
        left: position.left,
        top: position.top,
      }));
    } catch {
      // Panel placement is a convenience only.
    }
  }

  clearPosition() {
    try {
      globalThis.window?.localStorage?.removeItem?.(STORAGE_KEY);
    } catch {
      // Ignore unavailable storage.
    }
  }
}

function defaultPanelPosition() {
  const viewport = panelViewport();
  return {
    left: Math.max(PANEL_MARGIN, viewport.width - PANEL_WIDTH - 340),
    top: PANEL_TOP,
  };
}
