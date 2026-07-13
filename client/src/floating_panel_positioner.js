const PANEL_MARGIN = 12;
const KEY_STEP = 24;
const KEY_STEP_LARGE = 72;
const MOBILE_DEBUG_MAX_VIEWPORT_WIDTH = 1024;
const MOBILE_DEBUG_MAX_VIEWPORT_HEIGHT = 1024;

// Shared move-only window behavior for app-shell panels. Callers retain ownership
// of their content, visibility, and persistence format.
export class FloatingPanelPositioner {
  constructor({
    root,
    defaultPosition,
    defaultSize,
    readPosition = () => null,
    savePosition = () => {},
    clearPosition = () => {},
    isMobileViewport = () => false,
    canConstrain = () => true,
    onReset = () => {},
  }) {
    this.root = root;
    this.defaultPosition = {
      left: finiteNumber(defaultPosition?.left) ?? PANEL_MARGIN,
      top: finiteNumber(defaultPosition?.top) ?? PANEL_MARGIN,
    };
    this.defaultSize = {
      width: finitePositive(defaultSize?.width) || 320,
      height: finitePositive(defaultSize?.height) || 240,
    };
    this.readPosition = readPosition;
    this.saveStoredPosition = savePosition;
    this.clearStoredPosition = clearPosition;
    this.isMobileViewport = isMobileViewport;
    this.canConstrain = canConstrain;
    this.onReset = onReset;
    this.renderListeners = [];
    this.windowListeners = [];
    this.activeListeners = [];
    this.drag = null;
  }

  mount(dragHandle, { restore = true } = {}) {
    if (!this.root || !dragHandle) return;
    this.listenRender(dragHandle, "pointerdown", (event) => this.beginDrag(event));
    this.listenRender(dragHandle, "keydown", (event) => this.handleKeyDown(event));
    this.listenWindow("resize", () => this.constrainToViewport());
    if (restore) this.restorePosition();
  }

  destroy() {
    this.finishDrag(false);
    this.clearRenderListeners();
    this.clearWindowListeners();
  }

  beginDrag(event) {
    if (this.isMobileViewport() || !isPrimaryPointer(event)) return;
    const point = eventPoint(event);
    if (!point) return;

    this.finishDrag(false);
    event.preventDefault?.();
    event.stopPropagation?.();
    try {
      event.currentTarget?.setPointerCapture?.(event.pointerId);
    } catch {}

    this.drag = {
      pointerId: event.pointerId,
      startX: point.x,
      startY: point.y,
      rect: this.currentRect(),
      moved: false,
    };
    if (this.root.dataset) this.root.dataset.panelDragging = "true";

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
    const deltaX = point.x - this.drag.startX;
    const deltaY = point.y - this.drag.startY;
    if (deltaX === 0 && deltaY === 0) return;
    this.drag.moved = true;
    this.applyPosition({
      ...this.drag.rect,
      left: this.drag.rect.left + deltaX,
      top: this.drag.rect.top + deltaY,
    });
  }

  finishPointerDrag(event) {
    if (this.drag && !samePointer(this.drag, event)) return;
    this.finishDrag(true);
  }

  finishDrag(save) {
    this.clearActiveListeners();
    if (save && this.drag?.moved) this.savePosition(this.currentRect());
    this.drag = null;
    if (this.root?.dataset) delete this.root.dataset.panelDragging;
  }

  handleKeyDown(event) {
    if (this.isMobileViewport()) return;
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
    if (this.isMobileViewport()) {
      this.clearPositionStyles();
      return;
    }
    const position = this.readPosition();
    if (position) this.applyPosition(position);
  }

  resetPosition() {
    this.clearStoredPosition();
    this.clearPositionStyles();
    this.onReset();
  }

  constrainToViewport() {
    if (!this.root || !this.canConstrain()) return;
    if (this.isMobileViewport()) {
      this.finishDrag(false);
      this.clearPositionStyles();
      return;
    }
    const position = this.hasPositionStyles() ? this.currentRect() : this.readPosition();
    if (!position) return;
    this.applyPosition(position);
    this.savePosition(this.currentRect());
  }

  currentRect() {
    const rect = this.root?.getBoundingClientRect?.();
    const width = finitePositive(rect?.width) || parsePixels(this.root?.style?.width) || this.defaultSize.width;
    const height = finitePositive(rect?.height) || parsePixels(this.root?.style?.height) || this.defaultSize.height;
    const left = parsePixels(this.root?.style?.left) ?? finiteNumber(rect?.left) ?? this.defaultPosition.left;
    const top = parsePixels(this.root?.style?.top) ?? finiteNumber(rect?.top) ?? this.defaultPosition.top;
    return this.constrainPosition({ left, top, width, height });
  }

  constrainPosition(position) {
    const viewport = panelViewport();
    const width = finitePositive(position?.width) || this.defaultSize.width;
    const height = finitePositive(position?.height) || this.defaultSize.height;
    const maxLeft = Math.max(PANEL_MARGIN, viewport.width - width - PANEL_MARGIN);
    const maxTop = Math.max(PANEL_MARGIN, viewport.height - height - PANEL_MARGIN);
    return {
      left: Math.round(clamp(finiteNumber(position?.left) ?? this.defaultPosition.left, PANEL_MARGIN, maxLeft)),
      top: Math.round(clamp(finiteNumber(position?.top) ?? this.defaultPosition.top, PANEL_MARGIN, maxTop)),
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

  hasPositionStyles() {
    return parsePixels(this.root?.style?.left) !== null || parsePixels(this.root?.style?.top) !== null;
  }

  savePosition(position) {
    if (this.isMobileViewport()) return;
    const next = this.constrainPosition(position);
    this.saveStoredPosition({ left: next.left, top: next.top });
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
}

export function panelViewport() {
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

export function isMobileDebugPanelViewport() {
  const viewport = panelViewport();
  return hasCoarsePrimaryPointer()
    && viewport.width <= MOBILE_DEBUG_MAX_VIEWPORT_WIDTH
    && viewport.height <= MOBILE_DEBUG_MAX_VIEWPORT_HEIGHT;
}

function hasCoarsePrimaryPointer() {
  try {
    return globalThis.window?.matchMedia?.("(pointer: coarse)")?.matches === true;
  } catch {
    return false;
  }
}

function arrowDelta(event) {
  const step = event?.shiftKey ? KEY_STEP_LARGE : KEY_STEP;
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
