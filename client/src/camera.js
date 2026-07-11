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
import {
  boundsForGroundPolygon,
  classifyProjectedPoint,
  clipGroundPolygonToBounds,
  createCameraSnapshot,
} from "./camera_projection.js";

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

function finiteNumber(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function requireFinite(value, name) {
  const number = finiteNumber(value);
  if (number == null) throw new TypeError(`${name} must be finite`);
  return number;
}

function requireNonNegative(value, name) {
  const number = requireFinite(value, name);
  if (number < 0) throw new RangeError(`${name} must be non-negative`);
  return number;
}

function sameView(camera, before) {
  return camera.x === before.x
    && camera.y === before.y
    && camera.zoom === before.zoom
    && camera.worldW === before.worldW
    && camera.worldH === before.worldH
    && camera.viewW === before.viewW
    && camera.viewH === before.viewH;
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
    this.viewW = requireNonNegative(viewW, "viewport width");
    this.viewH = requireNonNegative(viewH, "viewport height");

    /** @private Last valid world-pixel audio reference distance. */
    this._lastAudioReferenceDistancePx = 1920;
    /** @private Semantic camera snapshot listeners. */
    this._listeners = new Set();
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
    const next = {
      worldW: requireNonNegative(worldW, "map width"),
      worldH: requireNonNegative(worldH, "map height"),
      viewW: requireNonNegative(viewW, "viewport width"),
      viewH: requireNonNegative(viewH, "viewport height"),
    };
    const before = this._rawView();
    this.worldW = next.worldW;
    this.worldH = next.worldH;
    this.viewW = next.viewW;
    this.viewH = next.viewH;
    this._clamp();
    this._emitIfChanged(before);
  }

  /**
   * Restore a previously captured viewport.
   * @param {{x?:number,y?:number,zoom?:number}|null} view
   */
  setView(view) {
    if (!view || typeof view !== "object") return false;
    if (Object.hasOwn(view, "version")) return this.restore(view);
    const before = this._rawView();
    const zoom = finiteNumber(view.zoom);
    if (zoom != null) {
      this.zoom = clampZoom(zoom, this.minZoom, this.maxZoom);
    }
    const centerX = finiteNumber(view.centerX);
    const centerY = finiteNumber(view.centerY);
    if (centerX != null && centerY != null) {
      this._centerOn(centerX, centerY);
      this._emitIfChanged(before);
      return !sameView(this, before);
    }
    const x = finiteNumber(view.x);
    const y = finiteNumber(view.y);
    if (x != null) this.x = x;
    if (y != null) this.y = y;
    this._clamp();
    this._emitIfChanged(before);
    return !sameView(this, before);
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

    const elapsed = finiteNumber(dt);
    if (elapsed == null || elapsed < 0) return;

    // Pan speed is defined at zoom 1; zooming in should pan slower in world space
    // so the on-screen pan rate feels constant.
    const speed = (CAMERA.panSpeed * elapsed) / this.zoom;
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
      const before = this._rawView();
      this.x += dx * speed;
      this.y += dy * speed;
      this._clamp();
      this._emitIfChanged(before);
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
   * Project a renderer-neutral presentation point into viewport-local CSS pixels.
   * The orthographic Pixi adapter intentionally ignores presentation-only height.
   * @param {{x:number,y:number,heightPx:number}} point
   * @returns {{x:number,y:number,depth:number,clip:string,visible:boolean}}
   */
  project(point) {
    const x = requireFinite(point?.x, "presented point x");
    const y = requireFinite(point?.y, "presented point y");
    requireFinite(point?.heightPx, "presented point heightPx");
    const screen = this.worldToScreen(x, y);
    return classifyProjectedPoint(
      { ...screen, depth: 1 },
      { widthCssPx: this.viewW, heightCssPx: this.viewH },
    );
  }

  /**
   * Intersect a viewport-local CSS point with the authoritative ground plane.
   * Orthographic projection always has a hit; future perspective adapters may return null.
   * @param {{x:number,y:number}} screen
   * @returns {{x:number,y:number}|null}
   */
  groundAtScreen(screen) {
    const x = finiteNumber(screen?.x);
    const y = finiteNumber(screen?.y);
    if (x == null || y == null) return null;
    const ground = this.screenToWorld(x, y);
    return Object.freeze({ x: ground.x, y: ground.y });
  }

  /**
   * Project a centered semantic extent at a presentation point.
   * @param {{x:number,y:number,heightPx:number}} point
   * @param {number} worldWidthPx
   * @param {number} worldHeightPx
   */
  projectedExtent(point, worldWidthPx, worldHeightPx) {
    const projected = this.project(point);
    const width = requireNonNegative(worldWidthPx, "world extent width") * this.zoom;
    const height = requireNonNegative(worldHeightPx, "world extent height") * this.zoom;
    const depthVisible = projected.depth > 0 && projected.clip !== "outsideDepth";
    const visible = this.viewW > 0
      && this.viewH > 0
      && depthVisible
      && projected.x + width / 2 >= 0
      && projected.x - width / 2 <= this.viewW
      && projected.y + height / 2 >= 0
      && projected.y - height / 2 <= this.viewH;
    return Object.freeze({
      width,
      height,
      scaleX: this.zoom,
      scaleY: this.zoom,
      visible,
    });
  }

  /** Return the bounded visible ground polygon in stable clockwise world winding. */
  viewportGroundPolygon() {
    if (this.viewW <= 0 || this.viewH <= 0 || this.worldW <= 0 || this.worldH <= 0) {
      return Object.freeze([]);
    }
    const corners = [
      this.screenToWorld(0, 0),
      this.screenToWorld(this.viewW, 0),
      this.screenToWorld(this.viewW, this.viewH),
      this.screenToWorld(0, this.viewH),
    ];
    return clipGroundPolygonToBounds(corners, {
      minX: 0,
      minY: 0,
      maxX: this.worldW,
      maxY: this.worldH,
    });
  }

  /** Return the conservative AABB of the visible ground polygon, or null when it is empty. */
  viewportGroundBounds() {
    return boundsForGroundPolygon(this.viewportGroundPolygon());
  }

  /**
   * Test a presented point against the projected viewport with an optional CSS-pixel margin.
   * @param {{x:number,y:number,heightPx:number}} point
   * @param {number} [marginCssPx]
   */
  containsProjected(point, marginCssPx = 0) {
    const margin = requireNonNegative(marginCssPx, "projection margin");
    const projected = this.project(point);
    return this.viewW > 0
      && this.viewH > 0
      && projected.depth > 0
      && projected.clip !== "outsideDepth"
      && projected.x >= -margin
      && projected.x <= this.viewW + margin
      && projected.y >= -margin
      && projected.y <= this.viewH + margin;
  }

  /**
   * Center the viewport on a world point (then clamp to bounds).
   * @param {number} wx
   * @param {number} wy
   */
  centerOn(wx, wy) {
    const x = requireFinite(wx, "camera center x");
    const y = requireFinite(wy, "camera center y");
    const before = this._rawView();
    this._centerOn(x, y);
    this._emitIfChanged(before);
  }

  /** Focus the semantic camera on a world point. */
  focusAt(point) {
    this.centerOn(point?.x, point?.y);
  }

  /**
   * Fit finite world points within viewport-local CSS padding.
   * Invalid points are ignored; no finite points leave the view unchanged.
   */
  fitWorldPoints(points, { paddingCssPx = 0 } = {}) {
    if (!Array.isArray(points)) throw new TypeError("fit points must be an array");
    const padding = requireNonNegative(paddingCssPx, "fit padding");
    const finite = points
      .map((point) => ({ x: finiteNumber(point?.x), y: finiteNumber(point?.y) }))
      .filter((point) => point.x != null && point.y != null);
    if (finite.length === 0) return false;
    const availableWidth = this.viewW - padding * 2;
    const availableHeight = this.viewH - padding * 2;
    if (availableWidth <= 0 || availableHeight <= 0) return false;

    const xs = finite.map((point) => point.x);
    const ys = finite.map((point) => point.y);
    const minX = Math.min(...xs);
    const maxX = Math.max(...xs);
    const minY = Math.min(...ys);
    const maxY = Math.max(...ys);
    const width = maxX - minX;
    const height = maxY - minY;
    const widthScale = width > 0 ? availableWidth / width : Number.POSITIVE_INFINITY;
    const heightScale = height > 0 ? availableHeight / height : Number.POSITIVE_INFINITY;
    const targetScale = Number.isFinite(Math.min(widthScale, heightScale))
      ? Math.min(widthScale, heightScale)
      : this.maxZoom;
    const before = this._rawView();
    this.zoom = clampZoom(targetScale, this.minZoom, this.maxZoom);
    this._centerOn((minX + maxX) / 2, (minY + maxY) / 2);
    this._emitIfChanged(before);
    return true;
  }

  /**
   * Pan by a screen-space drag delta. Positive dx/dy means the pointer moved
   * right/down, so the viewed world is pulled with it.
   * @param {number} dx screen-space x delta in pixels
   * @param {number} dy screen-space y delta in pixels
   */
  panByScreenDelta(dx, dy) {
    const deltaX = typeof dx === "object" && dx !== null ? dx.x : dx;
    const deltaY = typeof dx === "object" && dx !== null ? dx.y : dy;
    const x = requireFinite(deltaX, "pan delta x");
    const y = requireFinite(deltaY, "pan delta y");
    const before = this._rawView();
    this.x -= x / this.zoom;
    this.y -= y / this.zoom;
    this._clamp();
    this._emitIfChanged(before);
  }

  /**
   * Set the zoom factor (clamped to this camera's min/max zoom) keeping a screen anchor
   * fixed in world space. With no anchor the viewport center is held.
   * @param {number} zoom target zoom
   * @param {number} [anchorSx] screen-space anchor x (defaults to viewport center)
   * @param {number} [anchorSy] screen-space anchor y (defaults to viewport center)
   */
  setZoom(zoom, anchorSx, anchorSy) {
    const target = requireFinite(zoom, "camera zoom");
    const ax = anchorSx == null
      ? this.viewW / 2
      : requireFinite(anchorSx, "zoom anchor x");
    const ay = anchorSy == null
      ? this.viewH / 2
      : requireFinite(anchorSy, "zoom anchor y");
    const before = this._rawView();
    this._setZoom(target, ax, ay);
    this._emitIfChanged(before);
  }

  /** Multiply framing scale while preserving the ground point under a valid CSS anchor. */
  dollyBy(factor, anchorScreen) {
    const multiplier = requireFinite(factor, "dolly factor");
    if (multiplier <= 0) throw new RangeError("dolly factor must be positive");
    const anchorX = anchorScreen == null
      ? this.viewW / 2
      : requireFinite(anchorScreen.x, "dolly anchor x");
    const anchorY = anchorScreen == null
      ? this.viewH / 2
      : requireFinite(anchorScreen.y, "dolly anchor y");
    const before = this._rawView();
    this._setZoom(this.zoom * multiplier, anchorX, anchorY);
    this._emitIfChanged(before);
  }

  /** Resize the viewport in CSS pixels without exposing canvas backing dimensions or DPR. */
  resize(viewportWidthCssPx, viewportHeightCssPx) {
    const width = requireNonNegative(viewportWidthCssPx, "viewport width");
    const height = requireNonNegative(viewportHeightCssPx, "viewport height");
    const before = this._rawView();
    this.viewW = width;
    this.viewH = height;
    this._clamp();
    this._emitIfChanged(before);
  }

  /** Set map dimensions in authoritative world pixels. */
  setMapBounds(worldWidthPx, worldHeightPx) {
    const width = requireNonNegative(worldWidthPx, "map width");
    const height = requireNonNegative(worldHeightPx, "map height");
    const before = this._rawView();
    this.worldW = width;
    this.worldH = height;
    this._clamp();
    this._emitIfChanged(before);
  }

  /** Return detached player-intent camera state, never raw adapter coordinates. */
  snapshot() {
    return createCameraSnapshot(
      this.x + this.viewW / (2 * this.zoom),
      this.y + this.viewH / (2 * this.zoom),
      this.zoom,
    );
  }

  /**
   * Freeze the current orthographic coefficients for selection/capture queries.
   * The result contains no live Camera, Pixi, DOM, or mutable matrix reference.
   */
  projectionSnapshot() {
    const originX = this.x;
    const originY = this.y;
    const scale = this.zoom;
    const width = this.viewW;
    const height = this.viewH;
    const worldWidth = this.worldW;
    const worldHeight = this.worldH;
    const camera = this.snapshot();
    const viewport = Object.freeze({ widthCssPx: width, heightCssPx: height });
    const mapBounds = worldWidth > 0 && worldHeight > 0
      ? Object.freeze({ minX: 0, minY: 0, maxX: worldWidth, maxY: worldHeight })
      : null;
    const project = (point) => {
      const x = requireFinite(point?.x, "presented point x");
      const y = requireFinite(point?.y, "presented point y");
      requireFinite(point?.heightPx, "presented point heightPx");
      return classifyProjectedPoint(
        { x: (x - originX) * scale, y: (y - originY) * scale, depth: 1 },
        { widthCssPx: width, heightCssPx: height },
      );
    };
    const groundAtScreen = (screen) => {
      const x = finiteNumber(screen?.x);
      const y = finiteNumber(screen?.y);
      if (x == null || y == null) return null;
      return Object.freeze({ x: originX + x / scale, y: originY + y / scale });
    };
    const viewportGroundPolygon = () => {
      if (!mapBounds || width <= 0 || height <= 0) return Object.freeze([]);
      return clipGroundPolygonToBounds([
        { x: originX, y: originY },
        { x: originX + width / scale, y: originY },
        { x: originX + width / scale, y: originY + height / scale },
        { x: originX, y: originY + height / scale },
      ], mapBounds);
    };
    const projectedExtent = (point, worldWidthPx, worldHeightPx) => {
      const projected = project(point);
      const projectedWidth = requireNonNegative(worldWidthPx, "world extent width") * scale;
      const projectedHeight = requireNonNegative(worldHeightPx, "world extent height") * scale;
      return Object.freeze({
        width: projectedWidth,
        height: projectedHeight,
        scaleX: scale,
        scaleY: scale,
        visible: width > 0
          && height > 0
          && projected.depth > 0
          && projected.clip !== "outsideDepth"
          && projected.x + projectedWidth / 2 >= 0
          && projected.x - projectedWidth / 2 <= width
          && projected.y + projectedHeight / 2 >= 0
          && projected.y - projectedHeight / 2 <= height,
      });
    };
    const containsProjected = (point, marginCssPx = 0) => {
      const margin = requireNonNegative(marginCssPx, "projection margin");
      const projected = project(point);
      return width > 0
        && height > 0
        && projected.depth > 0
        && projected.clip !== "outsideDepth"
        && projected.x >= -margin
        && projected.x <= width + margin
        && projected.y >= -margin
        && projected.y <= height + margin;
    };
    const referenceDistancePx = Number.isFinite(width / scale) && width / scale > 0
      ? width / scale
      : this._lastAudioReferenceDistancePx;
    return Object.freeze({
      version: 1,
      camera,
      viewport,
      mapBounds,
      project,
      groundAtScreen,
      projectedExtent,
      viewportGroundPolygon,
      viewportGroundBounds: () => boundsForGroundPolygon(viewportGroundPolygon()),
      containsProjected,
      snapshot: () => camera,
      audioListener: () => Object.freeze({
        x: camera.focus.x,
        y: camera.focus.y,
        referenceDistancePx,
      }),
    });
  }

  /**
   * Restore CameraSnapshotV1 or the named legacy `{x,y,zoom}` read edge.
   * Unknown versions and malformed values fail without mutating the view.
   */
  restore(snapshotOrLegacy) {
    if (!snapshotOrLegacy || typeof snapshotOrLegacy !== "object") return false;
    let focusX;
    let focusY;
    let framingScale;
    if (snapshotOrLegacy.version === 1) {
      if (snapshotOrLegacy.boundsPolicy !== "mapOverscroll") return false;
      focusX = finiteNumber(snapshotOrLegacy.focus?.x);
      focusY = finiteNumber(snapshotOrLegacy.focus?.y);
      framingScale = finiteNumber(snapshotOrLegacy.framingScale);
    } else if (!Object.hasOwn(snapshotOrLegacy, "version")) {
      const legacyX = finiteNumber(snapshotOrLegacy.x);
      const legacyY = finiteNumber(snapshotOrLegacy.y);
      framingScale = finiteNumber(snapshotOrLegacy.zoom);
      if (legacyX == null || legacyY == null || framingScale == null) return false;
      framingScale = clampZoom(framingScale, this.minZoom, this.maxZoom);
      focusX = legacyX + this.viewW / (2 * framingScale);
      focusY = legacyY + this.viewH / (2 * framingScale);
    } else {
      return false;
    }
    if (focusX == null || focusY == null || framingScale == null || framingScale <= 0) return false;

    const before = this._rawView();
    this.zoom = clampZoom(framingScale, this.minZoom, this.maxZoom);
    this._centerOn(focusX, focusY);
    this._emitIfChanged(before);
    return true;
  }

  /** Return semantic spatial-audio listener data in world pixels. */
  audioListener() {
    const view = this.snapshot();
    const scale = this.projectedExtent(
      { ...view.focus, heightPx: 0 },
      1,
      1,
    ).scaleX;
    const candidate = this.viewW / scale;
    if (Number.isFinite(candidate) && candidate > 0) {
      this._lastAudioReferenceDistancePx = candidate;
    }
    return Object.freeze({
      x: view.focus.x,
      y: view.focus.y,
      referenceDistancePx: this._lastAudioReferenceDistancePx,
    });
  }

  /** Subscribe to detached CameraSnapshotV1 values after successful view mutations. */
  subscribe(listener) {
    if (typeof listener !== "function") throw new TypeError("camera listener must be a function");
    this._listeners.add(listener);
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      this._listeners.delete(listener);
    };
  }

  /** @private */
  _rawView() {
    return {
      x: this.x,
      y: this.y,
      zoom: this.zoom,
      worldW: this.worldW,
      worldH: this.worldH,
      viewW: this.viewW,
      viewH: this.viewH,
    };
  }

  /** @private */
  _centerOn(x, y) {
    this.x = x - this.viewW / (2 * this.zoom);
    this.y = y - this.viewH / (2 * this.zoom);
    this._clamp();
  }

  /** @private */
  _setZoom(zoom, anchorX, anchorY) {
    const before = this.screenToWorld(anchorX, anchorY);
    this.zoom = clampZoom(zoom, this.minZoom, this.maxZoom);
    const after = this.screenToWorld(anchorX, anchorY);
    this.x += before.x - after.x;
    this.y += before.y - after.y;
    this._clamp();
  }

  /** @private */
  _emitIfChanged(before) {
    if (sameView(this, before) || this._listeners.size === 0) return;
    const snapshot = this.snapshot();
    for (const listener of [...this._listeners]) {
      try {
        listener(snapshot);
      } catch (error) {
        console.error("Camera semantic listener failed", error);
      }
    }
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
