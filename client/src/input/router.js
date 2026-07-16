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
    this.hoverZone = null;
    this.hoverSource = null;
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
    if (this.hoverZone === zone) this._setHoverZone(null);
  }

  activePreviewSurface() { return this.captureZone?.previewSurface || this.hoverZone?.previewSurface || null; }

  pointerDown(event) {
    const e = this._normalize(event);
    for (const zone of this.zones) {
      if (!zone.contains(e)) continue;
      if (typeof zone.pointerDown === "function" && zone.pointerDown(e)) {
        this._setHoverZone(zone, e);
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
    let hoverZone = null;
    for (const candidate of this.zones) {
      if (!candidate.contains(e)) continue;
      hoverZone = candidate;
      break;
    }
    this._setHoverZone(hoverZone, e);
    if (typeof hoverZone?.pointerMove === "function") return !!hoverZone.pointerMove(e);
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

  wheel(event) {
    const e = this._normalize(event);
    for (const zone of this.zones) {
      if (!zone.contains(e)) continue;
      if (typeof zone.wheel === "function" && zone.wheel(e)) return true;
    }
    return false;
  }

  /** Cancel interaction state owned by an event source that has stopped producing events. */
  releaseSource(source) {
    let released = false;
    if (this.captureZone && this.captureSource === source) {
      const zone = this.captureZone;
      this._clearCapture();
      zone.pointerCancel?.({ source });
      released = true;
    }
    if (this.hoverZone && (!this.hoverSource || this.hoverSource === source)) {
      this._setHoverZone(null, { source });
      released = true;
    }
    return released;
  }

  _clearCapture() {
    this.captureZone = null;
    this.captureSource = null;
  }

  _setHoverZone(zone, event) {
    if (zone === this.hoverZone) {
      if (zone) this.hoverSource = event?.source || null;
      return;
    }
    const previous = this.hoverZone;
    this.hoverZone = zone;
    this.hoverSource = zone ? event?.source || null : null;
    previous?.pointerLeave?.(event);
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

const DOM_FOCUS_SELECTOR = [
  "button",
  "input",
  "select",
  "textarea",
  "a[href]",
  "[tabindex]",
].join(",");

const DEFAULT_POINTER_ID = 1;

/**
 * Forwards pointer-lock synthetic cursor events back into DOM surfaces.
 *
 * Native DOM click targeting does not work while the browser pointer is locked:
 * the real mouse stays captured by the canvas, while Input maintains a virtual
 * cursor. This zone bridges routed locked-pointer events to the DOM element
 * beneath that virtual cursor, while ignored roots such as the viewport continue
 * to fall through to gameplay input.
 */
export class DomClickInputZone {
  constructor(roots, { priority = 120, documentRef = globalThis.document, ignoreRoots = [] } = {}) {
    this.document = documentRef;
    const defaultRoot = this.document?.body || globalThis.document?.body || null;
    const configuredRoots = Array.isArray(roots) ? roots : [roots || defaultRoot];
    this.roots = configuredRoots.filter(Boolean);
    this.ignoreRoots = (Array.isArray(ignoreRoots) ? ignoreRoots : [ignoreRoots]).filter(Boolean);
    this.priority = priority;
    this.activeRoot = null;
    this.activeTarget = null;
    this.activeClickTarget = null;
    this.activeRange = null;
    this.activeButton = 0;
  }

  contains(ev) {
    if (!this._syntheticSource(ev)) return false;
    return !!this._hitAt(ev.clientX, ev.clientY);
  }

  pointerDown(ev) {
    if (!this._syntheticSource(ev)) return false;
    const hit = this._hitAt(ev.clientX, ev.clientY);
    if (!hit) return false;
    this.activeRoot = hit.root;
    this.activeTarget = hit.target;
    this.activeClickTarget = ev.button === 0 ? this._clickTarget(hit.target, hit.root) : null;
    this.activeButton = ev.button;
    const interactionTarget = this.activeClickTarget || this.activeTarget;
    this.activeRange = this._rangeInput(interactionTarget);
    if (this.activeRange) this._setRangeFromPoint(this.activeRange, ev.clientX);
    this._focusTarget(interactionTarget);
    this._dispatchPointerMouseEvent(this.activeTarget, "down", ev);
    ev.originalEvent?.preventDefault();
    ev.preventDefault?.();
    return true;
  }

  pointerMove(ev) {
    if (!this._syntheticSource(ev)) return false;
    const target = this.activeTarget || this._hitAt(ev.clientX, ev.clientY)?.target;
    if (!target) return false;
    if (this.activeRange) this._setRangeFromPoint(this.activeRange, ev.clientX);
    this._dispatchPointerMouseEvent(target, "move", ev);
    ev.originalEvent?.preventDefault();
    ev.preventDefault?.();
    return true;
  }

  pointerUp(ev) {
    if (!this._syntheticSource(ev)) return false;
    const root = this.activeRoot || this._hitAt(ev.clientX, ev.clientY)?.root;
    const target = this.activeTarget;
    const clickTarget = this.activeClickTarget;
    const range = this.activeRange;
    this.activeRoot = null;
    this.activeTarget = null;
    this.activeClickTarget = null;
    this.activeRange = null;
    const upHit = this._hitAt(ev.clientX, ev.clientY);
    const upTarget = target || upHit?.target;
    if (!root || !upTarget) return false;
    ev.originalEvent?.preventDefault();
    ev.preventDefault?.();
    this._dispatchPointerMouseEvent(upTarget, "up", ev);
    if (range) {
      this._dispatchMouseEvent(range, "change", ev);
      return true;
    }
    const upClickTarget = upHit ? this._clickTarget(upHit.target, upHit.root) : null;
    if (ev.button === 0 && clickTarget && upClickTarget === clickTarget) {
      this._dispatchMouseEvent(clickTarget, "click", ev);
      this._showPicker(clickTarget);
    }
    return true;
  }

  pointerCancel() {
    const active = !!this.activeRoot;
    this.activeRoot = null;
    this.activeTarget = null;
    this.activeClickTarget = null;
    this.activeRange = null;
    return active;
  }

  wheel(ev) {
    if (!this._syntheticSource(ev)) return false;
    const hit = this._hitAt(ev.clientX, ev.clientY);
    if (!hit) return false;
    const dispatched = this._dispatchWheelEvent(hit.target, ev);
    if (dispatched) this._scrollFromWheel(hit.target, ev);
    ev.originalEvent?.preventDefault();
    ev.preventDefault?.();
    return true;
  }

  _syntheticSource(ev) {
    return ev?.source === "locked";
  }

  _hitAt(clientX, clientY) {
    const target = this.document?.elementFromPoint?.(clientX, clientY);
    if (!target || this._insideIgnoredRoot(target)) return null;
    const root = this._rootForTarget(target, clientX, clientY);
    return root ? { root, target } : null;
  }

  _rootForTarget(target, clientX, clientY) {
    for (const root of this.roots) {
      if (!this._containsClientPoint(root, clientX, clientY)) continue;
      if (root === target || root.contains?.(target)) return root;
    }
    return null;
  }

  _insideIgnoredRoot(target) {
    return this.ignoreRoots.some((root) => root === target || root.contains?.(target));
  }

  _containsClientPoint(el, clientX, clientY) {
    if (!el || el.hidden) return false;
    const rect = el.getBoundingClientRect?.();
    if (!rect) return false;
    return clientX >= rect.left && clientX <= rect.right && clientY >= rect.top && clientY <= rect.bottom;
  }

  _clickTarget(target, root) {
    const closest = target?.closest?.(DOM_CLICK_SELECTOR);
    if (closest && (closest === root || root.contains?.(closest)) && !this._disabled(closest)) return closest;
    if (this._disabled(target)) return null;
    return target || null;
  }

  _focusTarget(target) {
    const focusTarget = target?.closest?.(DOM_FOCUS_SELECTOR) || target;
    if (!focusTarget || this._disabled(focusTarget) || typeof focusTarget.focus !== "function") return;
    try {
      focusTarget.focus({ preventScroll: true });
    } catch {
      try {
        focusTarget.focus();
      } catch {}
    }
  }

  _disabled(target) {
    return !!target?.disabled || target?.getAttribute?.("aria-disabled") === "true";
  }

  _rangeInput(target) {
    const range = target?.closest?.("input[type='range']") || target;
    if (!range || String(range.type || "").toLowerCase() !== "range") return null;
    if (this._disabled(range)) return null;
    const HTMLInputCtor = globalThis.HTMLInputElement;
    if (typeof HTMLInputCtor === "function") return range instanceof HTMLInputCtor ? range : null;
    return range;
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

  _dispatchPointerMouseEvent(target, phase, ev) {
    const pointerType = phase === "down" ? "pointerdown" : phase === "up" ? "pointerup" : "pointermove";
    const mouseType = phase === "down" ? "mousedown" : phase === "up" ? "mouseup" : "mousemove";
    this._dispatchPointerEvent(target, pointerType, ev, phase);
    this._dispatchMouseEvent(target, mouseType, ev, phase);
  }

  _dispatchPointerEvent(target, type, ev, phase = "move") {
    if (!target || typeof target.dispatchEvent !== "function") return true;
    const PointerEventCtor = globalThis.PointerEvent;
    const options = this._pointerMouseOptions(type, ev, phase);
    if (typeof PointerEventCtor === "function") {
      return target.dispatchEvent(new PointerEventCtor(type, {
        ...options,
        pointerId: DEFAULT_POINTER_ID,
        pointerType: "mouse",
        isPrimary: true,
      }));
    }
    return this._dispatchEvent(target, type, options);
  }

  _dispatchMouseEvent(target, type, ev, phase = null) {
    if (!target) return true;
    const MouseEventCtor = globalThis.MouseEvent;
    const options = this._pointerMouseOptions(type, ev, phase);
    if (typeof MouseEventCtor !== "function") {
      if (type === "click" && typeof target.click === "function") {
        target.click();
        return true;
      }
      return this._dispatchEvent(target, type, options);
    }
    if (typeof target.dispatchEvent !== "function") return true;
    return target.dispatchEvent(new MouseEventCtor(type, options));
  }

  _dispatchWheelEvent(target, ev) {
    if (!target || typeof target.dispatchEvent !== "function") return true;
    const options = {
      ...this._pointerMouseOptions("wheel", ev),
      deltaX: Number.isFinite(ev.deltaX) ? ev.deltaX : 0,
      deltaY: Number.isFinite(ev.deltaY) ? ev.deltaY : 0,
      deltaMode: Number.isFinite(ev.deltaMode) ? ev.deltaMode : 0,
    };
    const WheelEventCtor = globalThis.WheelEvent;
    if (typeof WheelEventCtor === "function") {
      return target.dispatchEvent(new WheelEventCtor("wheel", options));
    }
    return this._dispatchEvent(target, "wheel", options);
  }

  _pointerMouseOptions(type, ev, phase = null) {
    return {
      bubbles: true,
      cancelable: true,
      view: globalThis.window || null,
      clientX: ev.clientX,
      clientY: ev.clientY,
      button: ev.button,
      buttons: this._buttonsFor(type, ev, phase),
      shiftKey: ev.shiftKey,
      ctrlKey: ev.ctrlKey,
      metaKey: ev.metaKey,
      altKey: ev.altKey,
    };
  }

  _buttonsFor(type, ev, phase) {
    if (type === "click" || type === "change" || type === "wheel") return 0;
    if (type === "pointerup" || type === "mouseup" || phase === "up") return 0;
    if (Number.isFinite(ev.buttons)) return ev.buttons;
    if ((type === "pointermove" || type === "mousemove" || phase === "move") && !this.activeRoot) return 0;
    const button = Number.isFinite(ev.button) ? ev.button : this.activeButton;
    if (button === 0) return 1;
    if (button === 1) return 4;
    if (button === 2) return 2;
    return 0;
  }

  _dispatchEvent(target, type, options = {}) {
    if (!target || typeof target.dispatchEvent !== "function") return true;
    const EventCtor = globalThis.Event;
    if (typeof EventCtor !== "function") return true;
    const event = new EventCtor(type, {
      bubbles: options.bubbles !== false,
      cancelable: options.cancelable !== false,
    });
    for (const [key, value] of Object.entries(options)) {
      if (key === "bubbles" || key === "cancelable") continue;
      try {
        Object.defineProperty(event, key, { configurable: true, value });
      } catch {}
    }
    return target.dispatchEvent(event);
  }

  _showPicker(target) {
    const tag = String(target?.tagName || "").toUpperCase();
    const pickerInputTypes = new Set(["color", "date", "datetime-local", "month", "time", "week"]);
    const canShowPicker = tag === "SELECT" ||
      (tag === "INPUT" && pickerInputTypes.has(String(target.type || "").toLowerCase()));
    if (!canShowPicker || typeof target.showPicker !== "function") return;
    try {
      target.showPicker();
    } catch {}
  }

  _scrollFromWheel(target, ev) {
    const scrollTarget = this._scrollableAncestor(target);
    if (!scrollTarget) return;
    const dx = Number.isFinite(ev.deltaX) ? ev.deltaX : 0;
    const dy = Number.isFinite(ev.deltaY) ? ev.deltaY : 0;
    scrollTarget.scrollLeft = Number(scrollTarget.scrollLeft || 0) + dx;
    scrollTarget.scrollTop = Number(scrollTarget.scrollTop || 0) + dy;
  }

  _scrollableAncestor(target) {
    for (let el = target; el; el = el.parentElement) {
      if (this._isScrollable(el)) return el;
    }
    return null;
  }

  _isScrollable(el) {
    const canScrollY = Number(el.scrollHeight || 0) > Number(el.clientHeight || 0);
    const canScrollX = Number(el.scrollWidth || 0) > Number(el.clientWidth || 0);
    if (!canScrollY && !canScrollX) return false;
    const style = globalThis.getComputedStyle?.(el);
    const overflowY = style?.overflowY || style?.overflow || "";
    const overflowX = style?.overflowX || style?.overflow || "";
    const yEnabled = canScrollY && !["hidden", "clip", "visible"].includes(overflowY);
    const xEnabled = canScrollX && !["hidden", "clip", "visible"].includes(overflowX);
    return yEnabled || xEnabled || !style;
  }
}
