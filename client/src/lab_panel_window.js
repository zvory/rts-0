import { createImmediateTouchButtonActivation } from "./panel_touch_activation.js";

const DEFAULT_STORAGE_KEY = "rts.labPanel.window.v1";
const DEFAULT_MARGIN = 12;
const DEFAULT_TOP = 58;
const DEFAULT_WIDTH = 320;
const DEFAULT_HEIGHT = 432;
const DEFAULT_MAX_HEIGHT = 560;
const DEFAULT_COMMAND_CARD_CLEARANCE = 368;
const MIN_WIDTH = 260;
const MIN_HEIGHT = 220;
const KEYBOARD_STEP = 24;
const KEYBOARD_LARGE_STEP = 72;
const STORAGE_SCHEMA_VERSION = 1;
const MOBILE_LAYOUT_MAX_WIDTH = 720;

export class LabPanelWindowChrome {
  constructor(el, options = {}) {
    this.el = el;
    this.windowObj = options.windowObj ?? globalThis.window ?? null;
    this.storage = options.storage ?? this.windowObj?.localStorage ?? null;
    this.storageKey = options.storageKey || DEFAULT_STORAGE_KEY;
    this.panelLabel = String(options.panelLabel || "lab controls").trim() || "panel";
    this.renderListeners = [];
    this.windowListeners = [];
    this.activeListeners = [];
    this.activeInteraction = null;
    this.collapsed = false;
    this.collapseButton = null;
    this.collapseLabel = "panel";
    this.onCollapsedChange = typeof options.onCollapsedChange === "function" ? options.onCollapsedChange : null;
    this.collapseActivation = createImmediateTouchButtonActivation(() => this.toggleCollapsed());

    this.restoreGeometry();
    this.listenWindow("resize", () => this.constrainToViewport());
  }

  renderHeader({ kicker = "Lab", title = "", collapseLabel = "panel" } = {}) {
    this.clearRenderListeners();
    this.collapseActivation.reset();
    this.collapseButton = null;
    this.collapseLabel = collapseLabel || "panel";

    const header = document.createElement("header");
    header.className = "lab-panel-titlebar";

    const dragHandle = document.createElement("div");
    dragHandle.className = "lab-panel-drag-handle";
    dragHandle.tabIndex = 0;
    dragHandle.setAttribute("role", "button");
    dragHandle.setAttribute("aria-label", `Move ${this.panelLabel} panel`);
    dragHandle.setAttribute("aria-keyshortcuts", "ArrowUp ArrowDown ArrowLeft ArrowRight Home");
    dragHandle.title = "Drag to move. Arrow keys nudge. Home resets.";
    dragHandle.dataset.labPanelDragHandle = "true";

    const grip = document.createElement("span");
    grip.className = "lab-panel-grip";
    grip.setAttribute("aria-hidden", "true");
    grip.textContent = "::::";

    const titleGroup = document.createElement("span");
    titleGroup.className = "lab-panel-title";
    const kickerNode = document.createElement("span");
    kickerNode.className = "lab-panel-kicker";
    kickerNode.textContent = kicker;
    titleGroup.append(kickerNode);
    const normalizedTitle = String(title || "").trim();
    if (normalizedTitle) {
      const titleNode = document.createElement("h2");
      titleNode.textContent = normalizedTitle;
      titleGroup.append(titleNode);
    }
    dragHandle.append(grip, titleGroup);

    const collapse = document.createElement("button");
    collapse.type = "button";
    collapse.className = "lab-btn lab-panel-collapse";
    collapse.dataset.labPanelCollapse = "true";
    this.collapseButton = collapse;
    this.syncCollapseButton();

    this.listenRender(dragHandle, "pointerdown", (event) => this.beginInteraction("move", event));
    this.listenRender(dragHandle, "keydown", (event) => this.handleMoveKey(event));
    this.listenRender(collapse, "pointerdown", this.collapseActivation.pointerdown);
    this.listenRender(collapse, "pointerup", this.collapseActivation.pointerup);
    this.listenRender(collapse, "pointercancel", this.collapseActivation.pointercancel);
    this.listenRender(collapse, "pointerleave", this.collapseActivation.pointerleave);
    this.listenRender(collapse, "click", this.collapseActivation.click);

    const actions = document.createElement("div");
    actions.className = "lab-panel-titlebar-actions";
    actions.append(collapse);

    header.append(dragHandle, actions);
    return header;
  }

  renderResizeHandle() {
    const handle = document.createElement("button");
    handle.type = "button";
    handle.className = "lab-panel-resize-handle";
    handle.title = "Drag to resize. Arrow keys resize.";
    handle.setAttribute("aria-label", "Resize lab controls panel");
    handle.setAttribute("aria-keyshortcuts", "ArrowUp ArrowDown ArrowLeft ArrowRight");
    this.listenRender(handle, "pointerdown", (event) => this.beginInteraction("resize", event));
    this.listenRender(handle, "keydown", (event) => this.handleResizeKey(event));
    return handle;
  }

  destroy() {
    this.finishInteraction(false);
    this.collapseActivation.reset();
    this.clearRenderListeners();
    for (const [target, type, handler] of this.windowListeners) {
      target.removeEventListener?.(type, handler);
    }
    this.windowListeners = [];
    this.collapseButton = null;
    this.onCollapsedChange = null;
  }

  beginInteraction(mode, event) {
    if (!isPrimaryPointer(event)) return;
    const point = eventPoint(event);
    if (!point) return;

    event.preventDefault?.();
    event.stopPropagation?.();
    try {
      event.currentTarget?.setPointerCapture?.(event.pointerId);
    } catch {}

    const rect = this.currentGeometry();
    this.applyGeometry(rect);
    this.activeInteraction = {
      mode,
      pointerId: event.pointerId,
      startX: point.x,
      startY: point.y,
      rect,
    };
    this.el.dataset.panelInteraction = mode;

    this.listenActive("pointermove", (moveEvent) => this.updateInteraction(moveEvent));
    this.listenActive("pointerup", (upEvent) => this.finishPointerInteraction(upEvent));
    this.listenActive("pointercancel", (cancelEvent) => this.finishPointerInteraction(cancelEvent));
    this.listenActive("blur", () => this.finishInteraction(true));
  }

  updateInteraction(event) {
    if (!this.activeInteraction) return;
    if (!samePointer(this.activeInteraction, event)) return;
    const point = eventPoint(event);
    if (!point) return;
    event.preventDefault?.();

    const dx = point.x - this.activeInteraction.startX;
    const dy = point.y - this.activeInteraction.startY;
    const base = this.activeInteraction.rect;
    const next = this.activeInteraction.mode === "resize"
      ? { ...base, width: base.width + dx, height: base.height + dy }
      : { ...base, left: base.left + dx, top: base.top + dy };
    this.applyGeometry(next);
  }

  finishPointerInteraction(event) {
    if (this.activeInteraction && !samePointer(this.activeInteraction, event)) return;
    this.finishInteraction(true);
  }

  finishInteraction(save) {
    for (const [target, type, handler] of this.activeListeners) {
      target.removeEventListener?.(type, handler);
    }
    this.activeListeners = [];
    if (save && this.activeInteraction) this.saveGeometry(this.currentGeometry());
    this.activeInteraction = null;
    delete this.el.dataset.panelInteraction;
  }

  handleMoveKey(event) {
    const key = event?.key;
    if (key === "Home") {
      event.preventDefault?.();
      this.resetGeometry();
      return;
    }
    const delta = arrowDelta(event);
    if (!delta) return;
    event.preventDefault?.();
    const rect = this.currentGeometry();
    this.applyGeometry({
      ...rect,
      left: rect.left + delta.x,
      top: rect.top + delta.y,
    });
    this.saveGeometry(this.currentGeometry());
  }

  handleResizeKey(event) {
    const delta = arrowDelta(event);
    if (!delta) return;
    event.preventDefault?.();
    const rect = this.currentGeometry();
    this.applyGeometry({
      ...rect,
      width: rect.width + delta.x,
      height: rect.height + delta.y,
    });
    this.saveGeometry(this.currentGeometry());
  }

  constrainToViewport() {
    if (this.isMobileLayout()) {
      this.clearGeometryStyles();
      this.el.dataset.windowed = "false";
      return;
    }
    const geometry = this.el.dataset.windowed === "true" ? this.currentGeometry() : this.readGeometry();
    if (!geometry) {
      this.el.dataset.windowed = "false";
      return;
    }
    this.applyGeometry(geometry);
    this.saveGeometry(this.currentGeometry());
  }

  restoreGeometry() {
    const saved = this.readStoredState();
    this.setCollapsed(saved?.collapsed === true, { save: false });
    if (saved?.geometry && !this.isMobileLayout()) this.applyGeometry(saved.geometry);
    else {
      if (this.isMobileLayout()) this.clearGeometryStyles();
      this.el.dataset.windowed = "false";
    }
  }

  resetGeometry() {
    this.removeStoredGeometry();
    this.clearGeometryStyles();
    this.el.dataset.windowed = "false";
    this.setCollapsed(false, { save: false });
  }

  currentGeometry() {
    const viewport = this.viewport();
    const rect = this.el.getBoundingClientRect?.();
    const width = parsePixels(this.el.style.width) || finitePositive(rect?.width) || defaultWidth(viewport);
    const height = parsePixels(this.el.style.height) || finitePositive(rect?.height) || defaultHeight(viewport);
    const left = finiteNumber(rect?.left) ?? parsePixels(this.el.style.left) ?? defaultLeft(viewport, width);
    const top = finiteNumber(rect?.top) ?? parsePixels(this.el.style.top) ?? DEFAULT_TOP;
    return this.constrainGeometry({ left, top, width, height });
  }

  constrainGeometry(geometry) {
    const viewport = this.viewport();
    const margin = DEFAULT_MARGIN;
    const maxWidth = Math.max(1, viewport.width - margin * 2);
    const maxHeight = Math.max(1, viewport.height - margin * 2);
    const minWidth = Math.min(MIN_WIDTH, maxWidth);
    const minHeight = Math.min(MIN_HEIGHT, maxHeight);
    const width = clamp(finitePositive(geometry.width) || defaultWidth(viewport), minWidth, maxWidth);
    const height = clamp(finitePositive(geometry.height) || defaultHeight(viewport), minHeight, maxHeight);
    const maxLeft = Math.max(margin, viewport.width - width - margin);
    const maxTop = Math.max(margin, viewport.height - height - margin);
    return {
      left: Math.round(clamp(finiteNumber(geometry.left) ?? defaultLeft(viewport, width), margin, maxLeft)),
      top: Math.round(clamp(finiteNumber(geometry.top) ?? DEFAULT_TOP, margin, maxTop)),
      width: Math.round(width),
      height: Math.round(height),
    };
  }

  applyGeometry(geometry) {
    const next = this.constrainGeometry(geometry);
    this.el.dataset.windowed = "true";
    setStyle(this.el, "left", `${next.left}px`);
    setStyle(this.el, "top", `${next.top}px`);
    setStyle(this.el, "width", `${next.width}px`);
    setStyle(this.el, "height", `${next.height}px`);
    setStyle(this.el, "right", "auto");
    setStyle(this.el, "bottom", "auto");
    setStyle(this.el, "max-height", "none");
  }

  clearGeometryStyles() {
    clearStyle(this.el, "left");
    clearStyle(this.el, "top");
    clearStyle(this.el, "width");
    clearStyle(this.el, "height");
    clearStyle(this.el, "right");
    clearStyle(this.el, "bottom");
    clearStyle(this.el, "max-height");
  }

  toggleCollapsed() {
    this.setCollapsed(!this.collapsed);
  }

  setCollapsed(collapsed, options = {}) {
    this.collapsed = !!collapsed;
    this.el.dataset.collapsed = this.collapsed ? "true" : "false";
    this.syncCollapseButton();
    if (options.save !== false) this.saveCollapsedState();
    this.onCollapsedChange?.(this.collapsed);
  }

  syncCollapseButton() {
    if (!this.collapseButton) return;
    const collapsed = this.collapsed;
    const label = this.collapseLabel || "panel";
    const action = collapsed ? "Expand" : "Collapse";
    this.collapseButton.textContent = action;
    this.collapseButton.title = `${action} ${label}`;
    this.collapseButton.setAttribute("aria-label", `${action} ${label}`);
    this.collapseButton.setAttribute("aria-expanded", collapsed ? "false" : "true");
  }

  viewport() {
    const documentElement = globalThis.document?.documentElement;
    return {
      width: finitePositive(this.windowObj?.innerWidth) ||
        finitePositive(documentElement?.clientWidth) ||
        1440,
      height: finitePositive(this.windowObj?.innerHeight) ||
        finitePositive(documentElement?.clientHeight) ||
        900,
    };
  }

  isMobileLayout() {
    return this.viewport().width <= MOBILE_LAYOUT_MAX_WIDTH;
  }

  listenRender(target, type, handler) {
    target.addEventListener(type, handler);
    this.renderListeners.push([target, type, handler]);
  }

  clearRenderListeners() {
    for (const [target, type, handler] of this.renderListeners) {
      target.removeEventListener?.(type, handler);
    }
    this.renderListeners = [];
  }

  listenWindow(type, handler) {
    if (!this.windowObj?.addEventListener) return;
    this.windowObj.addEventListener(type, handler);
    this.windowListeners.push([this.windowObj, type, handler]);
  }

  listenActive(type, handler) {
    if (!this.windowObj?.addEventListener) return;
    this.windowObj.addEventListener(type, handler);
    this.activeListeners.push([this.windowObj, type, handler]);
  }

  readStoredState() {
    try {
      const raw = this.storage?.getItem?.(this.storageKey);
      if (!raw) return null;
      const parsed = JSON.parse(raw);
      if (parsed?.schemaVersion !== STORAGE_SCHEMA_VERSION) return null;
      const geometry = {
        left: Number(parsed.left),
        top: Number(parsed.top),
        width: Number(parsed.width),
        height: Number(parsed.height),
      };
      return {
        collapsed: parsed.collapsed === true,
        geometry: Object.values(geometry).every(Number.isFinite) ? geometry : null,
      };
    } catch {
      return null;
    }
  }

  readGeometry() {
    return this.readStoredState()?.geometry || null;
  }

  saveGeometry(geometry) {
    try {
      if (this.isMobileLayout()) {
        this.saveCollapsedState();
        return;
      }
      const next = this.constrainGeometry(geometry);
      this.storage?.setItem?.(this.storageKey, JSON.stringify({
        schemaVersion: STORAGE_SCHEMA_VERSION,
        collapsed: this.collapsed,
        ...next,
      }));
    } catch {
      // Local storage is an ergonomic hint, not a requirement for lab controls.
    }
  }

  saveCollapsedState() {
    try {
      const stored = this.readStoredState();
      this.storage?.setItem?.(this.storageKey, JSON.stringify({
        schemaVersion: STORAGE_SCHEMA_VERSION,
        collapsed: this.collapsed,
        ...(stored?.geometry || {}),
      }));
    } catch {
      // Ignore unavailable storage.
    }
  }

  removeStoredGeometry() {
    try {
      this.storage?.removeItem?.(this.storageKey);
    } catch {
      // Ignore unavailable storage.
    }
  }
}

function defaultWidth(viewport) {
  return Math.min(DEFAULT_WIDTH, Math.max(1, viewport.width - DEFAULT_MARGIN * 2));
}

function defaultHeight(viewport) {
  return Math.min(
    DEFAULT_MAX_HEIGHT,
    Math.max(MIN_HEIGHT, finitePositive(viewport.height - DEFAULT_COMMAND_CARD_CLEARANCE) || DEFAULT_HEIGHT),
  );
}

function defaultLeft(viewport, width) {
  return Math.max(DEFAULT_MARGIN, viewport.width - width - DEFAULT_MARGIN);
}

function arrowDelta(event) {
  const step = event?.shiftKey ? KEYBOARD_LARGE_STEP : KEYBOARD_STEP;
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
  el.style.setProperty?.(property, value);
  el.style[toCamelCase(property)] = value;
}

function clearStyle(el, property) {
  el.style.removeProperty?.(property);
  el.style[toCamelCase(property)] = "";
}

function toCamelCase(property) {
  return property.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
}
