/**
 * Ordered input router for match UI zones.
 *
 * Events use both viewport-local coordinates for gameplay code and client
 * coordinates for DOM-backed UI zones. Zones return true when they consume an
 * event. A zone that consumes pointerDown captures subsequent moves/up until
 * release, which keeps drags stable even if the cursor leaves the zone.
 */
export class MatchInputRouter {
  constructor(viewportEl) {
    this.viewportEl = viewportEl;
    this.zones = [];
    this.captureZone = null;
    this.captureSource = null;
  }

  registerZone(zone) {
    if (!zone || typeof zone.contains !== "function") {
      throw new Error("input router zone must implement contains(event)");
    }
    this.zones.push(zone);
    this.zones.sort((a, b) => (b.priority || 0) - (a.priority || 0));
    return () => this.unregisterZone(zone);
  }

  unregisterZone(zone) {
    this.zones = this.zones.filter((z) => z !== zone);
    if (this.captureZone === zone) this._clearCapture();
  }

  pointerDown(event) {
    const e = this._normalize(event);
    for (const zone of this.zones) {
      if (!zone.contains(e)) continue;
      if (typeof zone.pointerDown === "function" && zone.pointerDown(e)) {
        this.captureZone = zone;
        this.captureSource = e.source || null;
        return true;
      }
    }
    return false;
  }

  pointerMove(event) {
    const e = this._normalize(event);
    const zone = this.captureZone;
    if (zone && this.captureSource && e.source && e.source !== this.captureSource) return false;
    if (zone && typeof zone.pointerMove === "function") {
      return !!zone.pointerMove(e);
    }
    for (const candidate of this.zones) {
      if (!candidate.contains(e)) continue;
      if (typeof candidate.pointerMove === "function") return !!candidate.pointerMove(e);
      return false;
    }
    return false;
  }

  pointerUp(event) {
    const e = this._normalize(event);
    const zone = this.captureZone;
    if (zone && this.captureSource && e.source && e.source !== this.captureSource) return false;
    this._clearCapture();
    if (zone && typeof zone.pointerUp === "function") return !!zone.pointerUp(e);
    return false;
  }

  _clearCapture() {
    this.captureZone = null;
    this.captureSource = null;
  }

  _normalize(event) {
    const rect = this.viewportEl.getBoundingClientRect();
    const viewportX = Number.isFinite(event.viewportX)
      ? event.viewportX
      : event.clientX - rect.left;
    const viewportY = Number.isFinite(event.viewportY)
      ? event.viewportY
      : event.clientY - rect.top;
    const clientX = Number.isFinite(event.clientX)
      ? event.clientX
      : rect.left + viewportX;
    const clientY = Number.isFinite(event.clientY)
      ? event.clientY
      : rect.top + viewportY;
    return {
      ...event,
      viewportX,
      viewportY,
      clientX,
      clientY,
      button: Number.isFinite(event.button) ? event.button : 0,
      shiftKey: !!event.shiftKey,
      ctrlKey: !!event.ctrlKey,
      metaKey: !!event.metaKey,
      altKey: !!event.altKey,
    };
  }
}

const DOM_CLICK_SELECTOR = [
  "button",
  "input",
  "select",
  "textarea",
  "a[href]",
  "[role='button']",
  "[role='menuitem']",
  "[role='menuitemcheckbox']",
  "[data-hotkey]",
].join(",");

/**
 * Forwards pointer-lock synthetic cursor clicks back into DOM HUD controls.
 *
 * Native DOM click targeting does not work while the browser pointer is locked:
 * the real mouse stays captured by the canvas, while Input maintains a virtual
 * cursor. This zone bridges routed locked-pointer events to the HUD element
 * beneath that virtual cursor.
 */
export class DomClickInputZone {
  constructor(roots, { priority = 120, documentRef = globalThis.document } = {}) {
    this.roots = Array.isArray(roots) ? roots.filter(Boolean) : [roots].filter(Boolean);
    this.priority = priority;
    this.document = documentRef;
    this.activeRoot = null;
    this.activeTarget = null;
    this.activeRange = null;
  }

  contains(ev) {
    return !!this._rootAt(ev.clientX, ev.clientY);
  }

  pointerDown(ev) {
    if (ev.button !== 0) return false;
    const root = this._rootAt(ev.clientX, ev.clientY);
    if (!root) return false;
    this.activeRoot = root;
    this.activeTarget = this._interactiveTargetAt(ev.clientX, ev.clientY, root);
    this.activeRange = this._rangeInput(this.activeTarget);
    if (this.activeRange) this._setRangeFromPoint(this.activeRange, ev.clientX);
    ev.originalEvent?.preventDefault();
    return true;
  }

  pointerMove(ev) {
    if (!this.activeRoot) return false;
    if (this.activeRange) this._setRangeFromPoint(this.activeRange, ev.clientX);
    ev.originalEvent?.preventDefault();
    return true;
  }

  pointerUp(ev) {
    const root = this.activeRoot;
    const target = this.activeTarget;
    const range = this.activeRange;
    this.activeRoot = null;
    this.activeTarget = null;
    this.activeRange = null;
    if (!root) return false;
    ev.originalEvent?.preventDefault();
    if (range) {
      this._dispatchMouseEvent(range, "change", ev);
      return true;
    }
    const upTarget = this._interactiveTargetAt(ev.clientX, ev.clientY, root);
    if (target && upTarget === target) this._dispatchMouseEvent(target, "click", ev);
    return true;
  }

  _rootAt(clientX, clientY) {
    for (const root of this.roots) {
      if (this._containsClientPoint(root, clientX, clientY)) return root;
    }
    return null;
  }

  _containsClientPoint(el, clientX, clientY) {
    if (!el || el.hidden) return false;
    const rect = el.getBoundingClientRect?.();
    if (!rect) return false;
    return clientX >= rect.left && clientX <= rect.right && clientY >= rect.top && clientY <= rect.bottom;
  }

  _interactiveTargetAt(clientX, clientY, root) {
    const el = this.document?.elementFromPoint?.(clientX, clientY);
    if (!el || !root.contains(el)) return null;
    const target = el.closest?.(DOM_CLICK_SELECTOR);
    if (!target || !root.contains(target)) return null;
    if (target.disabled || target.getAttribute?.("aria-disabled") === "true") return null;
    return target;
  }

  _rangeInput(target) {
    const HTMLInputCtor = globalThis.HTMLInputElement;
    return typeof HTMLInputCtor === "function" && target instanceof HTMLInputCtor && target.type === "range"
      ? target
      : null;
  }

  _setRangeFromPoint(input, clientX) {
    const rect = input.getBoundingClientRect();
    const min = Number.parseFloat(input.min || "0");
    const max = Number.parseFloat(input.max || "100");
    const span = max - min;
    if (!Number.isFinite(span) || span <= 0 || rect.width <= 0) return;
    const t = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    input.value = String(min + span * t);
    this._dispatchEvent(input, "input");
  }

  _dispatchMouseEvent(target, type, ev) {
    const MouseEventCtor = globalThis.MouseEvent;
    if (typeof MouseEventCtor !== "function") {
      target.click?.();
      return;
    }
    target.dispatchEvent(new MouseEventCtor(type, {
      bubbles: true,
      cancelable: true,
      view: globalThis.window || null,
      clientX: ev.clientX,
      clientY: ev.clientY,
      button: ev.button,
      shiftKey: ev.shiftKey,
      ctrlKey: ev.ctrlKey,
      metaKey: ev.metaKey,
      altKey: ev.altKey,
    }));
  }

  _dispatchEvent(target, type) {
    const EventCtor = globalThis.Event;
    if (typeof EventCtor !== "function") return;
    target.dispatchEvent(new EventCtor(type, { bubbles: true }));
  }
}
