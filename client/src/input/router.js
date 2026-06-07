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
