const ROOM_TIME_PANEL_STORAGE_KEY = "rts.roomTimeControls.panel.v1";
const ROOM_TIME_PANEL_MARGIN = 12;
const ROOM_TIME_PANEL_DEFAULT_LEFT = 12;
const ROOM_TIME_PANEL_DEFAULT_TOP = 70;
const ROOM_TIME_PANEL_KEY_STEP = 24;
const ROOM_TIME_PANEL_KEY_STEP_LARGE = 72;
const ROOM_TIME_PANEL_DEFAULT_WIDTH = 420;
const ROOM_TIME_PANEL_DEFAULT_HEIGHT = 120;

export class FloatingRoomTimePanel {
  constructor({ root, label }) {
    this.root = root;
    this.label = label || "Room time";
    this.contentEl = null;
    this.renderListeners = [];
    this.windowListeners = [];
    this.activeListeners = [];
    this.drag = null;
  }

  mount() {
    if (!this.root || !globalThis.document?.createElement) return null;
    this.root.classList.add("room-time-floating-panel");

    let body = this.root.querySelector(".room-time-panel-body");
    let dragHandle = this.root.querySelector(".room-time-panel-drag-handle");
    let reset = this.root.querySelector(".room-time-panel-reset");

    if (!body || !dragHandle || !reset) {
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

      reset = document.createElement("button");
      reset.type = "button";
      reset.className = "room-time-panel-reset";
      reset.textContent = "Reset";

      body = document.createElement("div");
      body.className = "room-time-panel-body";
      for (const child of existing) body.appendChild(child);

      header.appendChild(dragHandle);
      header.appendChild(reset);
      this.root.replaceChildren(header, body);
    }

    this.syncLabels();
    this.contentEl = body;
    this.listenRender(dragHandle, "pointerdown", (event) => this.beginDrag(event));
    this.listenRender(dragHandle, "keydown", (event) => this.handleKeyDown(event));
    this.listenRender(reset, "click", () => this.resetPosition());
    this.listenWindow("resize", () => this.constrainToViewport());
    this.restorePosition();
    return this.contentEl;
  }

  syncLabels() {
    const dragHandle = this.root?.querySelector(".room-time-panel-drag-handle");
    const reset = this.root?.querySelector(".room-time-panel-reset");
    const title = this.root?.querySelector(".room-time-panel-title");
    if (title) title.textContent = this.label;
    dragHandle?.setAttribute("aria-label", `Move ${this.label.toLowerCase()} controls`);
    if (reset) {
      reset.title = `Reset ${this.label.toLowerCase()} controls position`;
      reset.setAttribute("aria-label", `Reset ${this.label.toLowerCase()} controls position`);
    }
  }

  destroy() {
    this.finishDrag(false);
    this.clearRenderListeners();
    this.clearWindowListeners();
    const body = this.root?.querySelector(".room-time-panel-body");
    if (this.root && body) {
      const controls = Array.from(body.children || []);
      this.root.replaceChildren(...controls);
    }
    this.root?.classList?.remove("room-time-floating-panel");
    if (this.root?.dataset) delete this.root.dataset.panelDragging;
    this.contentEl = null;
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

  listenWindow(type, handler) {
    const windowObj = globalThis.window;
    if (!windowObj?.addEventListener) return;
    windowObj.addEventListener(type, handler);
    this.windowListeners.push([windowObj, type, handler]);
  }

  clearWindowListeners() {
    for (const [target, type, handler] of this.windowListeners) {
      target?.removeEventListener?.(type, handler);
    }
    this.windowListeners = [];
  }

  listenActive(type, handler) {
    const windowObj = globalThis.window;
    if (!windowObj?.addEventListener) return;
    windowObj.addEventListener(type, handler);
    this.activeListeners.push([windowObj, type, handler]);
  }

  clearActiveListeners() {
    for (const [target, type, handler] of this.activeListeners) {
      target?.removeEventListener?.(type, handler);
    }
    this.activeListeners = [];
  }

  beginDrag(event) {
    if (!isPrimaryPointer(event)) return;
    const point = eventPoint(event);
    if (!point) return;

    this.finishDrag(false);
    event.preventDefault?.();
    event.stopPropagation?.();
    event.currentTarget?.setPointerCapture?.(event.pointerId);

    const rect = this.currentRect();
    const start = this.constrainPosition({ left: rect.left, top: rect.top, width: rect.width, height: rect.height });
    this.applyPosition(start);
    this.drag = {
      pointerId: event.pointerId,
      startX: point.x,
      startY: point.y,
      rect: start,
    };
    if (this.root?.dataset) this.root.dataset.panelDragging = "true";

    this.listenActive("pointermove", (moveEvent) => this.updateDrag(moveEvent));
    this.listenActive("pointerup", (upEvent) => this.finishPointerDrag(upEvent));
    this.listenActive("pointercancel", (cancelEvent) => this.finishPointerDrag(cancelEvent));
    this.listenActive("blur", () => this.finishDrag(true));
  }

  updateDrag(event) {
    if (!this.drag || !samePointer(this.drag, event)) return;
    const point = eventPoint(event);
    if (!point) return;
    event.preventDefault?.();
    this.applyPosition({
      ...this.drag.rect,
      left: this.drag.rect.left + point.x - this.drag.startX,
      top: this.drag.rect.top + point.y - this.drag.startY,
    });
  }

  finishPointerDrag(event) {
    if (this.drag && !samePointer(this.drag, event)) return;
    this.finishDrag(true);
  }

  finishDrag(save) {
    this.clearActiveListeners();
    if (save && this.drag) this.savePosition(this.currentRect());
    this.drag = null;
    if (this.root?.dataset) delete this.root.dataset.panelDragging;
  }

  handleKeyDown(event) {
    if (event?.key === "Home") {
      event.preventDefault?.();
      this.resetPosition();
      return;
    }
    const delta = arrowDelta(event);
    if (!delta) return;
    event.preventDefault?.();
    const rect = this.currentRect();
    this.applyPosition({
      ...rect,
      left: rect.left + delta.x,
      top: rect.top + delta.y,
    });
    this.savePosition(this.currentRect());
  }

  restorePosition() {
    const saved = this.readPosition();
    if (saved) this.applyPosition(saved);
  }

  resetPosition() {
    this.removeStoredPosition();
    this.clearPositionStyles();
  }

  constrainToViewport() {
    if (!this.root || this.root.hidden) return;
    this.applyPosition(this.currentRect());
    this.savePosition(this.currentRect());
  }

  currentRect() {
    const rect = this.root?.getBoundingClientRect?.();
    const width = finitePositive(rect?.width) || parsePixels(this.root?.style?.width) || ROOM_TIME_PANEL_DEFAULT_WIDTH;
    const height = finitePositive(rect?.height) || parsePixels(this.root?.style?.height) || ROOM_TIME_PANEL_DEFAULT_HEIGHT;
    const left = finiteNumber(rect?.left) ?? parsePixels(this.root?.style?.left) ?? ROOM_TIME_PANEL_DEFAULT_LEFT;
    const top = finiteNumber(rect?.top) ?? parsePixels(this.root?.style?.top) ?? ROOM_TIME_PANEL_DEFAULT_TOP;
    return this.constrainPosition({ left, top, width, height });
  }

  constrainPosition(position) {
    const viewport = panelViewport();
    const width = finitePositive(position.width) || ROOM_TIME_PANEL_DEFAULT_WIDTH;
    const height = finitePositive(position.height) || ROOM_TIME_PANEL_DEFAULT_HEIGHT;
    const maxLeft = Math.max(ROOM_TIME_PANEL_MARGIN, viewport.width - width - ROOM_TIME_PANEL_MARGIN);
    const maxTop = Math.max(ROOM_TIME_PANEL_MARGIN, viewport.height - height - ROOM_TIME_PANEL_MARGIN);
    return {
      left: Math.round(clamp(finiteNumber(position.left) ?? ROOM_TIME_PANEL_DEFAULT_LEFT, ROOM_TIME_PANEL_MARGIN, maxLeft)),
      top: Math.round(clamp(finiteNumber(position.top) ?? ROOM_TIME_PANEL_DEFAULT_TOP, ROOM_TIME_PANEL_MARGIN, maxTop)),
      width,
      height,
    };
  }

  applyPosition(position) {
    if (!this.root) return;
    const next = this.constrainPosition(position);
    setStyle(this.root, "left", `${next.left}px`);
    setStyle(this.root, "top", `${next.top}px`);
    setStyle(this.root, "right", "auto");
    setStyle(this.root, "bottom", "auto");
    setStyle(this.root, "transform", "none");
  }

  clearPositionStyles() {
    if (!this.root) return;
    clearStyle(this.root, "left");
    clearStyle(this.root, "top");
    clearStyle(this.root, "right");
    clearStyle(this.root, "bottom");
    clearStyle(this.root, "transform");
  }

  readPosition() {
    try {
      const raw = globalThis.window?.localStorage?.getItem?.(ROOM_TIME_PANEL_STORAGE_KEY);
      if (!raw) return null;
      const parsed = JSON.parse(raw);
      if (parsed?.schemaVersion !== 1) return null;
      const left = Number(parsed.left);
      const top = Number(parsed.top);
      return Number.isFinite(left) && Number.isFinite(top) ? { left, top } : null;
    } catch {
      return null;
    }
  }

  savePosition(position) {
    try {
      const next = this.constrainPosition(position);
      globalThis.window?.localStorage?.setItem?.(ROOM_TIME_PANEL_STORAGE_KEY, JSON.stringify({
        schemaVersion: 1,
        left: next.left,
        top: next.top,
      }));
    } catch {
      // Room-time panel position is a convenience only.
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

function panelViewport() {
  const documentElement = globalThis.document?.documentElement;
  return {
    width: finitePositive(globalThis.window?.innerWidth) ||
      finitePositive(documentElement?.clientWidth) ||
      1440,
    height: finitePositive(globalThis.window?.innerHeight) ||
      finitePositive(documentElement?.clientHeight) ||
      900,
  };
}

function arrowDelta(event) {
  const step = event?.shiftKey ? ROOM_TIME_PANEL_KEY_STEP_LARGE : ROOM_TIME_PANEL_KEY_STEP;
  if (event?.key === "ArrowLeft") return { x: -step, y: 0 };
  if (event?.key === "ArrowRight") return { x: step, y: 0 };
  if (event?.key === "ArrowUp") return { x: 0, y: -step };
  if (event?.key === "ArrowDown") return { x: 0, y: step };
  return null;
}

function eventPoint(event) {
  const x = Number(event?.clientX);
  const y = Number(event?.clientY);
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null;
}

function isPrimaryPointer(event) {
  if (event?.button != null && event.button !== 0) return false;
  if (event?.isPrimary === false) return false;
  return true;
}

function samePointer(active, event) {
  return active.pointerId == null || event?.pointerId == null || active.pointerId === event.pointerId;
}

function parsePixels(value) {
  if (typeof value !== "string" || !value.endsWith("px")) return null;
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function finiteNumber(value) {
  return Number.isFinite(value) ? value : null;
}

function finitePositive(value) {
  return Number.isFinite(value) && value > 0 ? value : null;
}

function clamp(value, min, max) {
  if (max < min) return min;
  return Math.min(max, Math.max(min, value));
}

function setStyle(el, property, value) {
  el.style?.setProperty?.(property, value);
  if (el.style) el.style[toCamelCase(property)] = value;
}

function clearStyle(el, property) {
  el.style?.removeProperty?.(property);
  if (el.style) el.style[toCamelCase(property)] = "";
}

function toCamelCase(property) {
  return property.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
}
