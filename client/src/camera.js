// Camera — the player's view into the world. See docs/design/client-ui.md §4.1 / §4.2.
//
// The camera holds the world-space coordinate of the viewport's top-left corner
// (`x`, `y`) and a `zoom` factor. World units are pixels at zoom 1; on screen a
// world distance `d` covers `d * zoom` device-independent pixels.
//
// Panning comes from three sources:
//   - keyboard arrows, applied in `update(dt, input)`,
//   - screen-edge scrolling (real cursor, or the pointer-lock virtual cursor,
//     within `CAMERA.edgeScrollPx` of a viewport edge),
//   - direct drag panning through `panByScreenDelta`,
// and the result is always clamped so the visible rectangle stays inside the map.
//
// The renderer drives the Pixi world container from `x`, `y`, `zoom`; the input
// layer uses `screenToWorld` for picking and `worldToScreen` for overlays.

import { CAMERA } from "./config.js";

function resolveMinZoom(value) {
  const zoom = Number(value);
  if (!Number.isFinite(zoom) || zoom <= 0) return CAMERA.minZoom;
  return zoom;
}

function resolveMaxZoom(value, minZoom) {
  const zoom = Number(value);
  const resolved = Number.isFinite(zoom) && zoom > 0 ? zoom : CAMERA.maxZoom;
  return Math.max(minZoom, resolved);
}

function clampZoom(value, minZoom, maxZoom) {
  return Math.max(minZoom, Math.min(maxZoom, value));
}

export class Camera {
  /**
   * @param {number} [viewW] initial viewport width in screen px
   * @param {number} [viewH] initial viewport height in screen px
   * @param {{minZoom?:number,maxZoom?:number}} [options] optional per-session zoom limits
   */
  constructor(viewW = 0, viewH = 0, options = {}) {
    /** World x of the viewport's top-left corner. */
    this.x = 0;
    /** World y of the viewport's top-left corner. */
    this.y = 0;
    /** Zoom factor: screen px per world px. */
    this.zoom = 1;
    /** Zoom limits for this session. Defaults to the live-match range. */
    this.minZoom = resolveMinZoom(options?.minZoom);
    this.maxZoom = resolveMaxZoom(options?.maxZoom, this.minZoom);

    /** Map extent in world px. Set via {@link Camera#setBounds}. */
    this.worldW = 0;
    this.worldH = 0;
    /** Viewport extent in screen px. */
    this.viewW = viewW;
    this.viewH = viewH;
  }

  /**
   * Record the map size (world px) and the viewport size (screen px). Used to
   * clamp panning and to keep the zoom within a range that can still fill the
   * viewport. Safe to call on every resize.
   * @param {number} worldW map width in world px
   * @param {number} worldH map height in world px
   * @param {number} viewW viewport width in screen px
   * @param {number} viewH viewport height in screen px
   */
  setBounds(worldW, worldH, viewW, viewH) {
    this.worldW = worldW;
    this.worldH = worldH;
    this.viewW = viewW;
    this.viewH = viewH;
    this._clamp();
  }

  /**
   * Restore a previously captured viewport.
   * @param {{x?:number,y?:number,zoom?:number}|null} view
   */
  setView(view) {
    if (!view) return;
    const zoom = Number(view.zoom);
    if (Number.isFinite(zoom)) {
      this.zoom = clampZoom(zoom, this.minZoom, this.maxZoom);
    }
    const centerX = Number(view.centerX);
    const centerY = Number(view.centerY);
    if (Number.isFinite(centerX) && Number.isFinite(centerY)) {
      this.centerOn(centerX, centerY);
      return;
    }
    const x = Number(view.x);
    const y = Number(view.y);
    if (Number.isFinite(x)) this.x = x;
    if (Number.isFinite(y)) this.y = y;
    this._clamp();
  }

  /**
   * Advance the camera one frame: apply keyboard + screen-edge panning, then clamp.
   * @param {number} dt seconds since the previous frame
   * @param {object} [input] read-only view of current input state
   * @param {{up:boolean,down:boolean,left:boolean,right:boolean}} [input.keys] pan flags
   *   owned by Input (arrow keys feed these; reset on blur)
   * @param {{x:number,y:number}|null} [input.mouse] cursor position in screen px, or null if outside.
   *   While pointer lock is active this is Input's clamped virtual cursor.
   */
  update(dt, input) {
    if (!input) return;

    // Pan speed is defined at zoom 1; zooming in should pan slower in world space
    // so the on-screen pan rate feels constant.
    const speed = (CAMERA.panSpeed * dt) / this.zoom;
    let dx = 0;
    let dy = 0;

    // Input owns the pan state as semantic direction flags (see input/index.js `this.keys`).
    const keys = input.keys;
    if (keys) {
      if (keys.left) dx -= 1;
      if (keys.right) dx += 1;
      if (keys.up) dy -= 1;
      if (keys.down) dy += 1;
    }

    // Screen-edge scrolling: nudge when the cursor hugs a viewport edge.
    const m = input.mouse;
    const band = CAMERA.edgeScrollPx;
    if (m && this.viewW > 0 && this.viewH > 0) {
      if (m.x <= band) dx -= 1;
      else if (m.x >= this.viewW - band) dx += 1;
      if (m.y <= band) dy -= 1;
      else if (m.y >= this.viewH - band) dy += 1;
    }

    if (dx !== 0 || dy !== 0) {
      this.x += dx * speed;
      this.y += dy * speed;
      this._clamp();
    }
  }

  /**
   * Convert a world point to its on-screen position (device-independent px).
   * @param {number} wx
   * @param {number} wy
   * @returns {{x:number, y:number}}
   */
  worldToScreen(wx, wy) {
    return {
      x: (wx - this.x) * this.zoom,
      y: (wy - this.y) * this.zoom,
    };
  }

  /**
   * Convert an on-screen point (device-independent px) to a world point.
   * @param {number} sx
   * @param {number} sy
   * @returns {{x:number, y:number}}
   */
  screenToWorld(sx, sy) {
    return {
      x: this.x + sx / this.zoom,
      y: this.y + sy / this.zoom,
    };
  }

  /**
   * Center the viewport on a world point (then clamp to bounds).
   * @param {number} wx
   * @param {number} wy
   */
  centerOn(wx, wy) {
    this.x = wx - this.viewW / (2 * this.zoom);
    this.y = wy - this.viewH / (2 * this.zoom);
    this._clamp();
  }

  /**
   * Pan by a screen-space drag delta. Positive dx/dy means the pointer moved
   * right/down, so the viewed world is pulled with it.
   * @param {number} dx screen-space x delta in pixels
   * @param {number} dy screen-space y delta in pixels
   */
  panByScreenDelta(dx, dy) {
    this.x -= dx / this.zoom;
    this.y -= dy / this.zoom;
    this._clamp();
  }

  /**
   * Set the zoom factor (clamped to this camera's min/max zoom) keeping a screen anchor
   * fixed in world space. With no anchor the viewport center is held.
   * @param {number} zoom target zoom
   * @param {number} [anchorSx] screen-space anchor x (defaults to viewport center)
   * @param {number} [anchorSy] screen-space anchor y (defaults to viewport center)
   */
  setZoom(zoom, anchorSx, anchorSy) {
    const ax = anchorSx == null ? this.viewW / 2 : anchorSx;
    const ay = anchorSy == null ? this.viewH / 2 : anchorSy;
    // World point currently under the anchor; we keep it pinned after zooming.
    const before = this.screenToWorld(ax, ay);
    this.zoom = clampZoom(zoom, this.minZoom, this.maxZoom);
    const after = this.screenToWorld(ax, ay);
    this.x += before.x - after.x;
    this.y += before.y - after.y;
    this._clamp();
  }

  /**
   * Clamp `x`/`y` so the visible world rectangle stays within the map. When the
   * map is smaller than the viewport along an axis it is centered on that axis.
   * @private
   */
  _clamp() {
    if (this.worldW <= 0 || this.worldH <= 0) return;
    const visW = this.viewW / this.zoom;
    const visH = this.viewH / this.zoom;

    // Allow up to one viewport of overscroll past each edge so UI chrome
    // (command card, top HUD) can be scrolled clear of map content.
    this.x = Math.max(-visW / 4, Math.min(this.worldW - visW * 3 / 4, this.x));
    this.y = Math.max(-visH / 4, Math.min(this.worldH - visH * 3 / 4, this.y));
  }
}
