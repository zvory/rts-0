// Minimap — the bottom-left overview canvas (`#minimap`, 220×220). Draws the terrain,
// the fog overlay, entity blips colored by owner, and the current camera ground
// footprint. Left-click/drag recenters the camera; right-click issues a context-sensitive
// order for the current own-unit selection. See docs/design/client-ui.md §4.1 (Minimap) and §4.2 (look).
//
// The minimap is a plain 2D canvas (not Pixi). World↔canvas conversion is a uniform
// scale derived from the map's pixel size and the (square) canvas size, letterboxed so
// non-square maps stay centered and undistorted.

import { cmd } from "./protocol.js";
import { ABILITY, KIND, ORDER_STAGE, TERRAIN, UPGRADE, isResource, isUnit } from "./protocol.js";
import {
  ABILITIES,
  COLORS,
  FOG_EXPLORED_ALPHA,
  FOG_UNEXPLORED_ALPHA,
  isProducerBuilding,
} from "./config.js";
import {
  buildArtilleryTargetLocks,
  isArtilleryFireAbility,
} from "./input/artillery_targeting.js";

const isImpassableTerrainCode = (code) => code === TERRAIN.ROCK || code === TERRAIN.WATER;

const PING_MS = 900;
const BORDER_PULSE_MS = 700;
const CONTEXT_MENU_EVENT_OPTIONS = { capture: true };
const IMPASSABLE_FOG_SCALE = 0.56;
const ARTILLERY_MINIMAP_MARKER_MS = 2200;
const ARTILLERY_MINIMAP_ICON_W = 30;
const ARTILLERY_MINIMAP_ICON_H = 24;
const MINIMAP_TAP_SLOP_PX = 8;
const MINIMAP_BLIP_SCALE = 1.6;
const MINIMAP_OWNED_ENTITY_BLIP_RADIUS = 1.6 * MINIMAP_BLIP_SCALE;
const MINIMAP_STATIC_ENTITY_BLIP_RADIUS = 2.2;
const MINIMAP_PLAYER_BLIP_OUTLINE_COLOR = "rgba(255,255,255,0.92)";
const MINIMAP_PLAYER_BLIP_OUTLINE_OFFSETS = Object.freeze([
  [0, -1],
  [-1, 0], [1, 0],
  [0, 1],
]);
const TWO_PI = Math.PI * 2;

// Convert one of the 0xRRGGBB palette ints into a CSS color string.
const hex = (n) => "#" + n.toString(16).padStart(6, "0");

const terrainStyleSignature = () => [
  COLORS.rock,
  COLORS.water,
  COLORS.field,
  COLORS.mud,
  COLORS.grass,
  COLORS.grassAlt,
].join(",");

const resourceStyleSignature = () => [
  COLORS.oil,
  COLORS.steel,
].join(",");

const resourceLayoutSignature = (resources) => {
  let signature = `${resources.length}`;
  for (const node of resources) {
    signature += `|${node.id}:${node.kind}:${node.x}:${node.y}:${node.remaining ?? ""}`;
  }
  return signature;
};

const commandTargetsMatch = (left, right) => {
  if (left === right) return true;
  if (!left || !right || typeof left !== "object" || typeof right !== "object") return false;
  return left.kind === right.kind && left.ability === right.ability;
};

const minimapArtillerySvg = (svgText) => {
  if (typeof svgText !== "string") return "";
  const style = [
    "<style>",
    "[id$='.packed'],[id^='part.art.flash'],[id^='anchor.'],[id^='bounds.']{display:none}",
    "</style>",
  ].join("");
  return svgText.replace(/<svg\b([^>]*)>/, `<svg$1>${style}`);
};

const createSvgImage = (svgText, canvas, onReady) => {
  const svg = minimapArtillerySvg(svgText);
  if (!svg) return null;
  const doc = canvas?.ownerDocument || globalThis.document || null;
  const image = doc?.createElement
    ? doc.createElement("img")
    : typeof globalThis.Image === "function"
      ? new globalThis.Image()
      : null;
  if (!image) return null;
  image.onload = () => onReady?.();
  image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  return image;
};

const staticSignatureChanged = (prev, next) => {
  if (!prev) return true;
  for (const [key, value] of Object.entries(next)) {
    if (prev[key] !== value) return true;
  }
  return false;
};

const signatureChangeReasons = (prev, next) => {
  if (!prev) return ["initial"];
  const reasons = new Set();
  for (const [key, value] of Object.entries(next || {})) {
    if (prev[key] !== value) reasons.add(signatureReasonForKey(key));
  }
  return reasons.size > 0 ? [...reasons] : ["unknown"];
};

const signatureReasonForKey = (key) => {
  if (key === "revision" || key === "visibleRevision" || key === "exploredRevision") return "fog-revision";
  if (key === "visibleGrid" || key === "exploredGrid" || key === "fog" || key === "revealAll") return "fog-grid";
  if (key === "map" || key === "terrain" || key === "resources") return "map-data";
  if (key === "mapWidth" || key === "mapHeight" || key === "tileSize") return "map-size";
  if (key === "size" || key === "scale" || key === "offX" || key === "offY" || key === "presentation") return "presentation";
  if (key === "style") return "style";
  if (key === "layout") return "resource-layout";
  return "other";
};

// Per-terrain fill (matches the renderer palette; rock reads as impassable).
const terrainFill = (code, tx, ty) => {
  if (code === TERRAIN.ROCK) return hex(COLORS.rock);
  if (code === TERRAIN.WATER) return hex(COLORS.water);
  const n = hash2(tx, ty);
  if (n > 0.78) return hex(COLORS.field);
  if (n < 0.18) return hex(COLORS.mud);
  return hex((tx + ty) % 2 === 0 ? COLORS.grass : COLORS.grassAlt);
};

const hash2 = (x, y) => {
  let n = (x * 374761393 + y * 668265263) | 0;
  n = (n ^ (n >>> 13)) | 0;
  n = Math.imul(n, 1274126177);
  return ((n ^ (n >>> 16)) >>> 0) / 4294967295;
};

/**
 * The minimap widget. Wiring (main.js) constructs one and calls `render()` once per
 * frame after fog/state have been updated.
 */
export class Minimap {
  /**
   * @param {HTMLCanvasElement} canvasEl the `#minimap` canvas.
   * @param {import("./state.js").GameState} state shared game state.
   * @param {import("./camera.js").Camera} camera semantic camera (for viewport footprint + focus).
   * @param {import("./fog.js").Fog} fog the local fog overlay grids.
   * @param {{issueCommand(command: object): object|boolean}} commandIssuer gameplay command seam.
   * @param {import("./client_intent.js").ClientIntent} [options.clientIntent] browser-local command/placement intent facade.
   * @param {boolean|function(): boolean} [options.commandsEnabled] whether minimap clicks may issue commands.
   */
  constructor(canvasEl, state, camera, fog, commandIssuer, inputRouter = null, options = {}) {
    this.canvas = canvasEl;
    this.ctx = canvasEl.getContext("2d");
    this.state = state;
    this.camera = camera;
    this.fog = fog;
    this.commandIssuer = commandIssuer;
    this.clientIntent = options.clientIntent || null;
    this.inputRouter = inputRouter;
    this.commandsEnabled = options.commandsEnabled ?? true;
    this._unregisterInputZone = null;
    this._hoverWorld = null;
    this._hoverShiftKey = false;

    this.size = canvasEl.width; // assumed square (220 per index.html)

    // Cached world->canvas transform, recomputed when the map first arrives.
    this._scale = 1; // canvas px per world px
    this._offX = 0; // canvas px letterbox offset
    this._offY = 0;
    this._mapW = 0; // map world width/height in px (for invalidation)
    this._mapH = 0;
    this._canvasHeight = canvasEl.height;

    this._staticCanvasFactory = typeof options.staticCanvasFactory === "function"
      ? options.staticCanvasFactory
      : null;
    this._terrainLayer = null;
    this._terrainLayerCtx = null;
    this._terrainLayerSignature = null;
    this._resourceLayer = null;
    this._resourceLayerCtx = null;
    this._resourceLayerSignature = null;
    this._fogLayer = null;
    this._fogLayerCtx = null;
    this._fogLayerSignature = null;
    this._playerBlipMaskLayer = null;
    this._playerBlipMaskLayerCtx = null;

    this._dragging = false;
    this._activePointerGesture = null;
    this._pings = [];
    this._artilleryMarkers = [];
    this._borderPulseUntil = 0;
    this._artilleryIconImage = options.artilleryIconImage || null;
    this._artilleryIconReady = !!this._artilleryIconImage;
    if (!this._artilleryIconImage && options.artilleryIconSvg) {
      this._artilleryIconImage = createSvgImage(options.artilleryIconSvg, canvasEl, () => {
        this._artilleryIconReady = true;
      });
    }

    // Bound handlers retained so destroy() can remove the exact references.
    this._onContextMenu = (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
    };
    this._onCanvasPointerDown = this._handleCanvasPointerDown.bind(this);
    this._onCanvasPointerMove = this._handleCanvasPointerMove.bind(this);
    this._onCanvasPointerUp = this._handleCanvasPointerUp.bind(this);
    this._onCanvasPointerCancel = this._handleCanvasPointerCancel.bind(this);
    this._onWindowBlur = this._handleWindowBlur.bind(this);

    this._installInput();
  }

  // --- Transform -------------------------------------------------------------

  _renderMap() {
    return this.state.map;
  }

  /** (Re)compute the world→canvas scale/offset from the current map dimensions. */
  _ensureTransform() {
    this._syncCanvasSize();
    const map = this._renderMap();
    if (!map) return false;
    const worldW = map.width * map.tileSize;
    const worldH = map.height * map.tileSize;
    if (worldW === this._mapW && worldH === this._mapH) return true;

    this._mapW = worldW;
    this._mapH = worldH;
    // Uniform scale to fit the whole map; letterbox the shorter axis.
    this._scale = Math.min(this.size / worldW, this.size / worldH);
    this._offX = (this.size - worldW * this._scale) / 2;
    this._offY = (this.size - worldH * this._scale) / 2;
    return true;
  }

  _syncCanvasSize() {
    const width = Number.isFinite(this.canvas.width) && this.canvas.width > 0
      ? this.canvas.width
      : this.size;
    const height = Number.isFinite(this.canvas.height) && this.canvas.height > 0
      ? this.canvas.height
      : width;
    if (width === this.size && height === this._canvasHeight) return;
    this.size = width;
    this._canvasHeight = height;
    this._mapW = 0;
    this._mapH = 0;
    this._invalidateStaticLayers();
  }

  _invalidateStaticLayers() {
    this._terrainLayerSignature = null;
    this._resourceLayerSignature = null;
    this._fogLayerSignature = null;
  }

  /** World px → canvas px. */
  _worldToCanvas(wx, wy) {
    return { x: this._offX + wx * this._scale, y: this._offY + wy * this._scale };
  }

  /** Canvas px → world px (clamped to map bounds). */
  _canvasToWorld(cx, cy) {
    const wx = (cx - this._offX) / this._scale;
    const wy = (cy - this._offY) / this._scale;
    const maxX = Math.max(0, this._mapW - 1);
    const maxY = Math.max(0, this._mapH - 1);
    return {
      x: Math.max(0, Math.min(maxX, wx)),
      y: Math.max(0, Math.min(maxY, wy)),
    };
  }

  /** Convert a mouse event into canvas-local pixel coords. */
  _eventToCanvas(ev) {
    const rect = this.canvas.getBoundingClientRect();
    // Account for any CSS scaling between the element box and the canvas backing size.
    const sx = this.canvas.width / rect.width;
    const sy = this.canvas.height / rect.height;
    return { x: (ev.clientX - rect.left) * sx, y: (ev.clientY - rect.top) * sy };
  }

  // --- Rendering -------------------------------------------------------------

  /** Draw the full minimap for the current frame. */
  render(frameViews = null, { profiler = null } = {}) {
    this._profiler = profiler || null;
    const ctx = this.ctx;
    if (!ctx) return;
    this._syncCanvasSize();

    // Void background (outside the map / before a map exists).
    ctx.fillStyle = hex(COLORS.bgVoid);
    ctx.fillRect(0, 0, this.size, this.size);

    if (!this._ensureTransform()) return;

    const entities = this._minimapEntities(frameViews);
    this._drawTerrainLayer();
    this._drawEntities(entities, { deferForegroundPlayer: true });
    this._drawFog();
    this._drawResourceLayer();
    this._drawPlayerOwnedEntityOutline(entities);
    this._drawEntities(entities, { foregroundPlayerOnly: true });
    const now = performance.now();
    this._drawArtilleryFiringMarkers(now);
    this._drawViewport();
    this._drawPings(now);
  }

  /**
   * Add a transient world-position ping. Audio cooldown/suppression never gates this.
   * @param {number} x world px
   * @param {number} y world px
   * @param {"info"|"warn"|"alert"} [severity]
   */
  ping(x, y, severity = "alert") {
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
      this.pulseBorder();
      return;
    }
    this._pings.push({ x, y, severity, startedAt: performance.now() });
  }

  /** Add a globally visible short-lived artillery firing marker. */
  markArtilleryFiring(ev) {
    const x = Number(ev?.x);
    const y = Number(ev?.y);
    if (!Number.isFinite(x) || !Number.isFinite(y)) return;
    const owner = Number.isInteger(ev?.owner) ? ev.owner : 0;
    const facing = Number.isFinite(ev?.facing) ? ev.facing : 0;
    this._artilleryMarkers.push({ owner, x, y, facing, startedAt: performance.now() });
    if (this._artilleryMarkers.length > 48) {
      this._artilleryMarkers.splice(0, this._artilleryMarkers.length - 48);
    }
  }

  /** Pulse the minimap border when an alert has no resolvable world position. */
  pulseBorder() {
    this._borderPulseUntil = Math.max(this._borderPulseUntil, performance.now() + BORDER_PULSE_MS);
  }

  /** Fill one minimap cell per tile with its terrain color. */
  _drawTerrain() {
    this._paintTerrain(this.ctx, this._renderMap());
  }

  _paintTerrain(ctx, map) {
    const ts = map.tileSize;
    // Cell size in canvas px; +1 to avoid hairline seams between adjacent cells.
    const cw = ts * this._scale + 1;
    const ch = ts * this._scale + 1;
    for (let ty = 0; ty < map.height; ty++) {
      for (let tx = 0; tx < map.width; tx++) {
        const code = map.terrain[ty * map.width + tx];
        ctx.fillStyle = terrainFill(code, tx, ty);
        const p = this._worldToCanvas(tx * ts, ty * ts);
        ctx.fillRect(p.x, p.y, cw, ch);
      }
    }
  }

  _drawTerrainLayer() {
    const map = this._renderMap();
    const layer = this._ensureStaticLayer("terrain");
    if (!layer) {
      this._drawTerrain();
      return;
    }
    const signature = this._terrainStaticSignature(map);
    if (staticSignatureChanged(this._terrainLayerSignature, signature)) {
      this._recordMinimapInvalidation("terrain", this._terrainLayerSignature, signature);
      this._recordMinimapDiagnostic("minimap.cache.terrain.miss");
      layer.ctx.clearRect(0, 0, this.size, this.size);
      this._paintTerrain(layer.ctx, map);
      this._terrainLayerSignature = signature;
    } else {
      this._recordMinimapDiagnostic("minimap.cache.terrain.hit");
    }
    this.ctx.drawImage(layer.canvas, 0, 0);
  }

  /** Draw non-depleted static resource blips after fog so they are always visible. */
  _drawResources() {
    this._paintResources(this.ctx, this._renderMap());
  }

  _paintResources(ctx, map) {
    const r = MINIMAP_STATIC_ENTITY_BLIP_RADIUS;
    for (const node of map.resources || []) {
      if (node.remaining === 0) continue;
      const p = this._worldToCanvas(node.x, node.y);
      if (node.kind === "oil") {
        ctx.fillStyle = hex(COLORS.oil);
        ctx.fillRect(p.x - r, p.y - r, r * 2, r * 2);
        ctx.strokeStyle = "#ffffff";
        ctx.lineWidth = 0.8;
        ctx.strokeRect(p.x - r, p.y - r, r * 2, r * 2);
      } else {
        ctx.fillStyle = hex(COLORS.steel);
        ctx.fillRect(p.x - r, p.y - r, r * 2, r * 2);
      }
    }
  }

  _drawResourceLayer() {
    const map = this._renderMap();
    const resources = map.resources || [];
    if (resources.length === 0) return;
    const layer = this._ensureStaticLayer("resource");
    if (!layer) {
      this._drawResources();
      return;
    }
    const signature = this._resourceStaticSignature(map, resources);
    if (staticSignatureChanged(this._resourceLayerSignature, signature)) {
      this._recordMinimapInvalidation("resource", this._resourceLayerSignature, signature);
      this._recordMinimapDiagnostic("minimap.cache.resource.miss");
      layer.ctx.clearRect(0, 0, this.size, this.size);
      this._paintResources(layer.ctx, map);
      this._resourceLayerSignature = signature;
    } else {
      this._recordMinimapDiagnostic("minimap.cache.resource.hit");
    }
    this.ctx.drawImage(layer.canvas, 0, 0);
  }

  _ensureStaticLayer(kind) {
    const layerProp = kind === "terrain"
      ? "_terrainLayer"
      : kind === "resource" ? "_resourceLayer" : "_fogLayer";
    const ctxProp = kind === "terrain"
      ? "_terrainLayerCtx"
      : kind === "resource" ? "_resourceLayerCtx" : "_fogLayerCtx";
    if (!this[layerProp]) {
      this[layerProp] = this._createStaticCanvas();
    }
    const canvas = this[layerProp];
    if (!canvas) return null;
    if (canvas.width !== this.size) canvas.width = this.size;
    if (canvas.height !== this.size) canvas.height = this.size;
    if (!this[ctxProp]) this[ctxProp] = canvas.getContext?.("2d") || null;
    const ctx = this[ctxProp];
    return ctx ? { canvas, ctx } : null;
  }

  _ensurePlayerBlipMaskLayer() {
    if (!this._playerBlipMaskLayer) {
      this._playerBlipMaskLayer = this._createDynamicCanvas();
    }
    const canvas = this._playerBlipMaskLayer;
    if (!canvas) return null;
    if (canvas.width !== this.size) canvas.width = this.size;
    if (canvas.height !== this.size) canvas.height = this.size;
    if (!this._playerBlipMaskLayerCtx) {
      this._playerBlipMaskLayerCtx = canvas.getContext?.("2d") || null;
    }
    const ctx = this._playerBlipMaskLayerCtx;
    return ctx ? { canvas, ctx } : null;
  }

  _createStaticCanvas() {
    if (this._staticCanvasFactory) return this._staticCanvasFactory();
    return this._createDynamicCanvas();
  }

  _createDynamicCanvas() {
    if (typeof OffscreenCanvas !== "undefined") return new OffscreenCanvas(1, 1);
    const doc = this.canvas?.ownerDocument || (typeof document !== "undefined" ? document : null);
    return doc?.createElement ? doc.createElement("canvas") : null;
  }

  _terrainStaticSignature(map) {
    return {
      map,
      terrain: map.terrain,
      mapWidth: map.width,
      mapHeight: map.height,
      tileSize: map.tileSize,
      size: this.size,
      scale: this._scale,
      offX: this._offX,
      offY: this._offY,
      presentation: this._canvasPresentationSignature(),
      style: terrainStyleSignature(),
    };
  }

  _resourceStaticSignature(map, resources) {
    return {
      map,
      resources,
      mapWidth: map.width,
      mapHeight: map.height,
      tileSize: map.tileSize,
      size: this.size,
      scale: this._scale,
      offX: this._offX,
      offY: this._offY,
      presentation: this._canvasPresentationSignature(),
      style: resourceStyleSignature(),
      layout: resourceLayoutSignature(resources),
    };
  }

  _canvasPresentationSignature() {
    const dpr = Number(globalThis.window?.devicePixelRatio ?? globalThis.devicePixelRatio ?? 1) || 1;
    let rectW = 0;
    let rectH = 0;
    if (typeof this.canvas.getBoundingClientRect === "function") {
      const rect = this.canvas.getBoundingClientRect();
      rectW = rect?.width || 0;
      rectH = rect?.height || 0;
    }
    return `${this.canvas.width}x${this.canvas.height}@${dpr}:${rectW}x${rectH}`;
  }

  /**
   * Draw the fog overlay over terrain/entities from a cached layer. The layer is rebuilt only
   * when Fog.revision or presentation inputs change; resource marks still draw above it.
   */
  _drawFog() {
    const map = this._renderMap();
    const fog = this.fog;
    if (!fog) return;
    const signature = this._fogLayerStaticSignature(map, fog);
    if (!signature) {
      this._recordMinimapDiagnostic("minimap.cache.fog.uncached");
      this._paintFog(this.ctx, map, fog);
      return;
    }
    const layer = this._ensureStaticLayer("fog");
    if (!layer) {
      this._recordMinimapDiagnostic("minimap.cache.fog.uncached");
      this._paintFog(this.ctx, map, fog);
      return;
    }
    if (staticSignatureChanged(this._fogLayerSignature, signature)) {
      this._recordMinimapInvalidation("fog", this._fogLayerSignature, signature);
      this._recordMinimapDiagnostic("minimap.cache.fog.miss");
      layer.ctx.clearRect(0, 0, this.size, this.size);
      this._paintFog(layer.ctx, map, fog);
      this._fogLayerSignature = signature;
    } else {
      this._recordMinimapDiagnostic("minimap.cache.fog.hit");
    }
    this.ctx.drawImage(layer.canvas, 0, 0);
  }

  _paintFog(ctx, map, fog) {
    if (fog.revealAll) return;
    const ts = map.tileSize;
    const ch = ts * this._scale + 1;
    const visibleGrid = this._fogGridForMap(fog.visibleGrid, map);
    const exploredGrid = this._fogGridForMap(fog.exploredGrid, map);
    const useGrids = !!visibleGrid && !!exploredGrid;
    const exploredFill = hex(COLORS.fogExplored);
    const unexploredFill = hex(COLORS.fogUnexplored);

    // Stone/water tiles keep only a light wash of fog so the map's shape stays legible.
    ctx.save();
    for (let ty = 0; ty < map.height; ty++) {
      let runStart = -1;
      let runFillStyle = "";
      let runAlpha = 0;
      const flushRun = (endTx) => {
        if (runStart < 0) return;
        const p = this._worldToCanvas(runStart * ts, ty * ts);
        ctx.globalAlpha = runAlpha;
        ctx.fillStyle = runFillStyle;
        ctx.fillRect(p.x, p.y, (endTx - runStart) * ts * this._scale + 1, ch);
        runStart = -1;
      };
      for (let tx = 0; tx < map.width; tx++) {
        const i = ty * map.width + tx;
        const visible = useGrids ? visibleGrid[i] === 1 : fog.isVisible(tx, ty);
        if (visible) {
          flushRun(tx);
          continue;
        }
        const impassable = isImpassableTerrainCode(map.terrain[i]);
        const explored = useGrids ? exploredGrid[i] === 1 : fog.isExplored(tx, ty);
        const fillStyle = explored ? exploredFill : unexploredFill;
        const alpha = (explored ? FOG_EXPLORED_ALPHA : FOG_UNEXPLORED_ALPHA)
          * (impassable ? IMPASSABLE_FOG_SCALE : 1);
        if (runStart >= 0 && runFillStyle === fillStyle && runAlpha === alpha) {
          continue;
        }
        flushRun(tx);
        runStart = tx;
        runFillStyle = fillStyle;
        runAlpha = alpha;
      }
      flushRun(map.width);
    }
    ctx.restore();
  }

  _fogGridForMap(grid, map) {
    const cellCount = map.width * map.height;
    return grid && grid.length === cellCount ? grid : null;
  }

  _fogLayerStaticSignature(map, fog) {
    if (!Number.isFinite(fog.revision)) return null;
    if (!this._fogGridForMap(fog.visibleGrid, map) || !this._fogGridForMap(fog.exploredGrid, map)) {
      return null;
    }
    return {
      map,
      terrain: map.terrain,
      mapWidth: map.width,
      mapHeight: map.height,
      tileSize: map.tileSize,
      size: this.size,
      scale: this._scale,
      offX: this._offX,
      offY: this._offY,
      presentation: this._canvasPresentationSignature(),
      fog,
      fogWidth: fog.width,
      fogHeight: fog.height,
      visibleGrid: fog.visibleGrid,
      exploredGrid: fog.exploredGrid,
      revision: fog.revision,
      visibleRevision: fog.visibleRevision,
      exploredRevision: fog.exploredRevision,
      revealAll: !!fog.revealAll,
      style: [
        COLORS.fogExplored,
        COLORS.fogUnexplored,
        FOG_EXPLORED_ALPHA,
        FOG_UNEXPLORED_ALPHA,
        IMPASSABLE_FOG_SCALE,
      ].join(","),
    };
  }

  _minimapEntities(frameViews = null) {
    this._recordMinimapDiagnostic(
      Array.isArray(frameViews?.currentEntities)
        ? "entityViews.cache.hit.minimap.current"
        : "entityViews.uncached.minimap.current",
    );
    const entities = Array.isArray(frameViews?.currentEntities)
      ? frameViews.currentEntities
      : this.state.entitiesInterpolated(1);
    const visibleEntities = entities;
    this._recordMinimapDiagnostic("minimap.entities.blips", visibleEntities.length);
    return visibleEntities;
  }

  /** Draw colored blips for visible entities. Foreground player blips draw last over resources. */
  _drawEntities(entities, { deferForegroundPlayer = false, foregroundPlayerOnly = false } = {}) {
    const ctx = this.ctx;
    if (!Array.isArray(entities)) return;
    for (const e of entities) {
      const playerOwned = this._isPlayerOwnedMinimapEntity(e);
      const foregroundPlayer = this._isForegroundPlayerMinimapEntity(e);
      if (foregroundPlayerOnly && !foregroundPlayer) continue;
      if (deferForegroundPlayer && foregroundPlayer) continue;
      const color = this._blipColor(e);
      this._drawEntityBlip(ctx, e, color, playerOwned);
    }
  }

  _isPlayerOwnedMinimapEntity(e) {
    const owner = Number(e?.owner);
    return Number.isFinite(owner) && owner !== 0 && !isResource(e?.kind);
  }

  _isForegroundPlayerMinimapEntity(e) {
    return this._isPlayerOwnedMinimapEntity(e) && !e?.visionOnly;
  }

  _drawPlayerOwnedEntityOutline(entities) {
    if (!Array.isArray(entities)) return;
    if (!entities.some((e) => this._isForegroundPlayerMinimapEntity(e))) return;

    const layer = this._ensurePlayerBlipMaskLayer();
    if (!layer) return;

    const { canvas, ctx: maskCtx } = layer;
    maskCtx.clearRect(0, 0, this.size, this.size);

    for (const e of entities) {
      if (!this._isForegroundPlayerMinimapEntity(e)) continue;
      this._drawEntityBlip(maskCtx, e, MINIMAP_PLAYER_BLIP_OUTLINE_COLOR, true, { scoutStroke: false });
    }

    const ctx = this.ctx;
    ctx.save();
    for (const [dx, dy] of MINIMAP_PLAYER_BLIP_OUTLINE_OFFSETS) {
      ctx.drawImage(canvas, dx, dy);
    }
    ctx.restore();
  }

  _drawEntityBlip(ctx, e, color, playerOwned, { scoutStroke = true } = {}) {
    const p = this._worldToCanvas(e.x, e.y);
    ctx.fillStyle = color;
    if (e.kind === KIND.SCOUT_PLANE) {
      this._drawScoutPlaneBlip(ctx, p.x, p.y, color, { stroke: scoutStroke });
      return;
    }
    const r = playerOwned
      ? MINIMAP_OWNED_ENTITY_BLIP_RADIUS
      : MINIMAP_STATIC_ENTITY_BLIP_RADIUS;
    ctx.fillRect(p.x - r, p.y - r, r * 2, r * 2);
  }

  _drawScoutPlaneBlip(ctx, cx, cy, color, { stroke = true } = {}) {
    const s = MINIMAP_BLIP_SCALE;
    ctx.save();
    ctx.strokeStyle = "#101010";
    ctx.fillStyle = color;
    ctx.lineWidth = 0.8;
    ctx.beginPath();
    ctx.moveTo(cx + 2.7 * s, cy);
    ctx.lineTo(cx - 1.8 * s, cy - 2.2 * s);
    ctx.lineTo(cx - 0.9 * s, cy);
    ctx.lineTo(cx - 1.8 * s, cy + 2.2 * s);
    ctx.closePath();
    ctx.fill();
    if (stroke) ctx.stroke();
    ctx.restore();
  }

  _recordMinimapInvalidation(kind, prev, next) {
    this._recordMinimapDiagnostic(`minimap.invalidate.${kind}`);
    for (const reason of signatureChangeReasons(prev, next)) {
      this._recordMinimapDiagnostic(`minimap.invalidate.${kind}.${reason}`);
    }
  }

  _recordMinimapDiagnostic(label, amount = 1) {
    this._profiler?.recordDiagnosticCounter?.(label, amount);
  }

  /** Blip color for an entity: own=green, ally=blue, enemy=player color/red, neutral=yellow. */
  _blipColor(e) {
    if (e.owner === 0 || isResource(e.kind)) return hex(COLORS.selectNeutral);
    if (ownOwner(this.state, e.owner)) return hex(COLORS.selectOwn);
    if (allyOwner(this.state, e.owner)) return hex(COLORS.selectAlly);
    // Enemy: prefer the player's assigned color if we know it, else the enemy tint.
    const player = this._playerById(e.owner);
    return (player && player.color) || hex(COLORS.selectEnemy);
  }

  _markerOwnerColor(owner) {
    if (owner === 0) return hex(COLORS.selectNeutral);
    if (ownOwner(this.state, owner)) return hex(COLORS.selectOwn);
    if (allyOwner(this.state, owner)) return hex(COLORS.selectAlly);
    const player = this._playerById(owner);
    return (player && player.color) || hex(COLORS.selectEnemy);
  }

  _playerById(id) {
    const players = this.state.players || [];
    return players.find((p) => p.id === id) || null;
  }

  /** Draw the semantic camera ground footprint without fabricating bounds for partial views. */
  _drawViewport() {
    const polygon = this._viewportGroundPolygon();
    if (polygon.length < 2) return;
    const ctx = this.ctx;
    ctx.save();
    ctx.strokeStyle = "rgba(255,255,255,0.85)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    polygon.forEach((point, index) => {
      const canvasPoint = this._worldToCanvas(point.x, point.y);
      const x = Math.round(canvasPoint.x) + 0.5;
      const y = Math.round(canvasPoint.y) + 0.5;
      if (index === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    if (polygon.length >= 3) ctx.closePath();
    ctx.stroke();
    ctx.restore();
  }

  _drawPings(now) {
    const ctx = this.ctx;
    if (!ctx) return;
    this._pings = this._pings.filter((p) => now - p.startedAt < PING_MS);
    for (const ping of this._pings) {
      const t = (now - ping.startedAt) / PING_MS;
      const p = this._worldToCanvas(ping.x, ping.y);
      const radius = 4 + 15 * t;
      ctx.save();
      ctx.globalAlpha = 1 - t;
      ctx.strokeStyle = ping.severity === "warn" ? "#ffd166" : "#ff4d4d";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(p.x, p.y, radius, 0, Math.PI * 2);
      ctx.stroke();
      ctx.restore();
    }
    if (now < this._borderPulseUntil) {
      const t = 1 - (this._borderPulseUntil - now) / BORDER_PULSE_MS;
      ctx.save();
      ctx.globalAlpha = Math.max(0, 1 - t);
      ctx.strokeStyle = "#ff4d4d";
      ctx.lineWidth = 3;
      ctx.strokeRect(1.5, 1.5, this.size - 3, this.size - 3);
      ctx.restore();
    }
  }

  _drawArtilleryFiringMarkers(now) {
    const ctx = this.ctx;
    if (!ctx) return;
    this._artilleryMarkers = this._artilleryMarkers.filter(
      (marker) => now - marker.startedAt < ARTILLERY_MINIMAP_MARKER_MS,
    );
    for (const marker of this._artilleryMarkers) {
      const p = this._worldToCanvas(marker.x, marker.y);
      const age = Math.max(0, now - marker.startedAt);
      const progress = Math.min(1, age / ARTILLERY_MINIMAP_MARKER_MS);
      this._drawArtilleryMarker(p.x, p.y, marker.facing, this._markerOwnerColor(marker.owner), progress);
    }
  }

  _drawArtilleryMarker(cx, cy, facing, color, progress) {
    const ctx = this.ctx;
    const alpha = 1 - progress * 0.35;
    ctx.save();
    ctx.translate(cx, cy);
    ctx.globalAlpha = alpha;
    ctx.rotate(Number.isFinite(facing) ? facing : 0);
    if (this._artilleryIconReady && this._artilleryIconImage) {
      ctx.drawImage(
        this._artilleryIconImage,
        -ARTILLERY_MINIMAP_ICON_W / 2,
        -ARTILLERY_MINIMAP_ICON_H / 2,
        ARTILLERY_MINIMAP_ICON_W,
        ARTILLERY_MINIMAP_ICON_H,
      );
    } else {
      this._drawFallbackArtilleryIcon(ctx, color);
    }
    ctx.restore();
  }

  _drawFallbackArtilleryIcon(ctx, color) {
    ctx.fillStyle = "rgba(17,13,10,0.88)";
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.1;
    ctx.beginPath();
    ctx.moveTo(-5, -3);
    ctx.lineTo(1, -3);
    ctx.lineTo(2.5, 3);
    ctx.lineTo(-5, 3);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(0, 0);
    ctx.lineTo(7, 0);
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(-2.5, -3.4, 1.2, 0, TWO_PI);
    ctx.arc(-2.5, 3.4, 1.2, 0, TWO_PI);
    ctx.fill();
  }

  _viewportGroundPolygon() {
    const polygon = this.camera?.viewportGroundPolygon?.();
    if (!Array.isArray(polygon)) return [];
    if (polygon.some((point) => !Number.isFinite(point?.x) || !Number.isFinite(point?.y))) return [];
    return polygon;
  }

  _focusCamera(world) {
    if (!Number.isFinite(world?.x) || !Number.isFinite(world?.y)) return;
    this.camera?.focusAt?.({ x: world.x, y: world.y });
  }

  // --- Input -----------------------------------------------------------------

  /** Install native Pointer Events for recenter (primary) and commands (right). */
  _installInput() {
    const c = this.canvas;
    if (this.inputRouter) {
      this._unregisterInputZone = this.inputRouter.registerZone(this.inputZone());
    }
    // Suppress the browser context menu so right-click can mean "move here".
    c.addEventListener("contextmenu", this._onContextMenu, CONTEXT_MENU_EVENT_OPTIONS);
    c.addEventListener("pointerdown", this._onCanvasPointerDown);
    c.addEventListener("pointermove", this._onCanvasPointerMove);
    c.addEventListener("pointerup", this._onCanvasPointerUp);
    c.addEventListener("pointercancel", this._onCanvasPointerCancel);
    window.addEventListener("blur", this._onWindowBlur);
  }

  /**
   * Remove all installed listeners (e.g. on game teardown / rematch) so stale
   * minimaps stop driving an old camera. Mirrors Input.destroy().
   */
  destroy() {
    const c = this.canvas;
    this._cancelActivePointerGesture();
    if (this._unregisterInputZone) {
      this._unregisterInputZone();
      this._unregisterInputZone = null;
    }
    c.removeEventListener("contextmenu", this._onContextMenu, CONTEXT_MENU_EVENT_OPTIONS);
    c.removeEventListener("pointerdown", this._onCanvasPointerDown);
    c.removeEventListener("pointermove", this._onCanvasPointerMove);
    c.removeEventListener("pointerup", this._onCanvasPointerUp);
    c.removeEventListener("pointercancel", this._onCanvasPointerCancel);
    window.removeEventListener("blur", this._onWindowBlur);
    this._clearMinimapSetupPreview();
    this._terrainLayer = null;
    this._terrainLayerCtx = null;
    this._terrainLayerSignature = null;
    this._resourceLayer = null;
    this._resourceLayerCtx = null;
    this._resourceLayerSignature = null;
    this._fogLayer = null;
    this._fogLayerCtx = null;
    this._fogLayerSignature = null;
  }

  _issueCommand(command) {
    const selected = typeof this.state?.selectedEntities === "function"
      ? this.state.selectedEntities()
      : [];
    const result = issueGameplayCommand(this.commandIssuer, command);
    this._intent()?.recordPlannedCommand?.(command, selected, result);
    return result;
  }

  _intent() {
    return this.clientIntent;
  }

  _addCommandFeedback(kind, x, y, append = false, radiusTiles = null) {
    return this._intent()?.addCommandFeedback?.(
      kind,
      x,
      y,
      append,
      radiusTiles,
      performance.now(),
      commandFeedbackOwner(this.state),
    );
  }

  updateCommandTargetPreview() {
    if (this._intent()?.commandTarget !== "setupAntiTankGuns") {
      this._clearMinimapSetupPreview();
      return false;
    }
    if (!this._hoverWorld) return false;
    return this._refreshSetupPreviewAt(this._hoverWorld.x, this._hoverWorld.y, this._hoverShiftKey);
  }

  inputZone() {
    return {
      priority: 100,
      contains: (ev) => this._containsClientPoint(ev.clientX, ev.clientY),
      pointerDown: (ev) => this._handlePointerDown(ev),
      pointerMove: (ev) => this._handlePointerMove(ev),
      pointerUp: () => this._handlePointerUp(),
    };
  }

  _routerEvent(ev, source) {
    return {
      clientX: ev.clientX,
      clientY: ev.clientY,
      button: ev.button,
      shiftKey: ev.shiftKey,
      ctrlKey: ev.ctrlKey,
      metaKey: ev.metaKey,
      altKey: ev.altKey,
      source,
      originalEvent: ev,
    };
  }

  _containsClientPoint(clientX, clientY) {
    const rect = this.canvas.getBoundingClientRect();
    return clientX >= rect.left && clientX <= rect.right && clientY >= rect.top && clientY <= rect.bottom;
  }

  _commandsEnabled() {
    if (this.state?.controlPolicy?.kind === "lab") {
      return !!this.state.controlPolicy.canUseCommandSurface?.(this.state);
    }
    if (typeof this.commandsEnabled === "function") return this.commandsEnabled() !== false;
    return this.commandsEnabled !== false;
  }

  _handleCanvasPointerDown(ev) {
    if (this._activePointerGesture) {
      // A second contact is a pinch or multi-touch inspection, never a target tap.
      if (this._activePointerGesture.pointerId !== ev.pointerId) this._cancelActivePointerGesture();
      return;
    }
    if (ev.button === 2) {
      this._handlePointerDown(this._routerEvent(ev, "dom"));
      return;
    }
    if (!this._isPrimaryPointerGesture(ev) || !this._ensureTransform()) return;

    const point = this._eventToCanvas(ev);
    const world = this._canvasToWorld(point.x, point.y);
    this._activePointerGesture = {
      pointerId: ev.pointerId,
      startClientX: ev.clientX,
      startClientY: ev.clientY,
      moved: false,
      commandTarget: this._intent()?.commandTarget || null,
      shiftKey: !!ev.shiftKey,
      ctrlKey: !!ev.ctrlKey,
      metaKey: !!ev.metaKey,
      altKey: !!ev.altKey,
    };
    this._capturePointer(ev.pointerId);
    // Unarmed primary presses retain the desktop recenter-on-press behavior.
    // Armed targets wait for an unambiguous release so an inspection drag cannot fire them.
    if (!this._activePointerGesture.commandTarget) {
      this._dragging = true;
      this._focusCamera(world);
    }
    ev.preventDefault();
  }

  _handlePointerDown(ev) {
    if (!this._ensureTransform()) return;
    const cp = this._eventToCanvas(ev);
    const w = this._canvasToWorld(cp.x, cp.y);
    if (ev.button === 2) {
      // Right-click: issue a context-sensitive order for the currently selected own units.
      ev.originalEvent?.preventDefault();
      ev.originalEvent?.stopPropagation();
      if (!this._commandsEnabled()) return true;
      this._issueOrder(w.x, w.y, !!ev.shiftKey);
      return true;
    } else if (ev.button === 0) {
      ev.originalEvent?.preventDefault();
      // Left-click while a command target is armed: issue the command instead of panning.
      if (this._intent()?.commandTarget) {
        return this._issuePrimaryTarget(w, ev);
      }
      // Default: recenter the camera (and start a drag).
      this._dragging = true;
      this._focusCamera(w);
      return true;
    }
    return false;
  }

  _issuePrimaryTarget(world, ev) {
    this._issueOrder(world.x, world.y, !!ev.shiftKey);
    const issued = typeof this._intent()?.issueCommandTarget === "function"
      ? this._intent().issueCommandTarget(ev)
      : { keepArmed: false };
    if (!issued.keepArmed) this._intent()?.endCommandTarget?.();
    return true;
  }

  _handleCanvasPointerMove(ev) {
    const gesture = this._activePointerGesture;
    if (!gesture || gesture.pointerId !== ev.pointerId) return;
    if (!gesture.moved && this._gestureMovedBeyondTapSlop(gesture, ev)) {
      gesture.moved = true;
      this._dragging = true;
    }
    this._handlePointerMove(this._routerEvent(ev, "dom"));
    ev.preventDefault();
  }

  _handlePointerMove(ev) {
    const hovering = this._updateHoverFromEvent(ev);
    if (!this._dragging) return hovering;
    const cp = this._eventToCanvas(ev);
    const w = this._canvasToWorld(cp.x, cp.y);
    this._focusCamera(w);
    ev.originalEvent?.preventDefault();
    return true;
  }

  _handleCanvasPointerUp(ev) {
    const gesture = this._activePointerGesture;
    if (!gesture || gesture.pointerId !== ev.pointerId) return;
    this._releasePointer(ev.pointerId);
    this._activePointerGesture = null;
    this._dragging = false;
    if (
      !gesture.moved &&
      gesture.commandTarget &&
      commandTargetsMatch(this._intent()?.commandTarget, gesture.commandTarget)
    ) {
      const actionEvent = this._routerEvent(ev, "dom");
      actionEvent.shiftKey = gesture.shiftKey;
      actionEvent.ctrlKey = gesture.ctrlKey;
      actionEvent.metaKey = gesture.metaKey;
      actionEvent.altKey = gesture.altKey;
      if (this._ensureTransform() && this._containsClientPoint(ev.clientX, ev.clientY)) {
        const point = this._eventToCanvas(ev);
        this._issuePrimaryTarget(this._canvasToWorld(point.x, point.y), actionEvent);
      }
    }
    ev.preventDefault();
  }

  _handlePointerUp() {
    this._dragging = false;
    return true;
  }

  _handleCanvasPointerCancel(ev) {
    if (!this._activePointerGesture || this._activePointerGesture.pointerId !== ev.pointerId) return;
    this._cancelActivePointerGesture();
    ev.preventDefault();
  }

  _handleWindowBlur() {
    this._cancelActivePointerGesture();
  }

  _isPrimaryPointerGesture(ev) {
    return ev.button === 0 && ev.isPrimary !== false && Number.isFinite(ev.pointerId);
  }

  _gestureMovedBeyondTapSlop(gesture, ev) {
    const dx = ev.clientX - gesture.startClientX;
    const dy = ev.clientY - gesture.startClientY;
    return dx * dx + dy * dy >= MINIMAP_TAP_SLOP_PX * MINIMAP_TAP_SLOP_PX;
  }

  _capturePointer(pointerId) {
    try {
      this.canvas.setPointerCapture?.(pointerId);
    } catch {}
  }

  _releasePointer(pointerId) {
    try {
      this.canvas.releasePointerCapture?.(pointerId);
    } catch {}
  }

  _cancelActivePointerGesture() {
    const gesture = this._activePointerGesture;
    if (gesture) this._releasePointer(gesture.pointerId);
    this._activePointerGesture = null;
    this._dragging = false;
    this._hoverWorld = null;
    this._clearMinimapSetupPreview();
  }

  _updateHoverFromEvent(ev) {
    if (!this._ensureTransform()) return false;
    if (!this._containsClientPoint(ev.clientX, ev.clientY)) {
      this._hoverWorld = null;
      this._clearMinimapSetupPreview();
      return false;
    }
    const cp = this._eventToCanvas(ev);
    const w = this._canvasToWorld(cp.x, cp.y);
    this._hoverWorld = w;
    this._hoverShiftKey = !!ev.shiftKey;
    const handled = this._refreshSetupPreviewAt(w.x, w.y, this._hoverShiftKey);
    if (handled) ev.originalEvent?.preventDefault();
    return handled;
  }

  _refreshSetupPreviewAt(wx, wy, shiftKey = false) {
    const intent = this._intent();
    if (intent?.commandTarget !== "setupAntiTankGuns") {
      this._clearMinimapSetupPreview();
      return false;
    }
    const guns = this._setupPreviewEntities(this._setupPreviewQueued(shiftKey));
    if (guns.length === 0) {
      this._clearMinimapSetupPreview();
      return false;
    }
    intent.updateAntiTankGunSetupPreview?.({
      source: "minimap",
      mouseX: wx,
      mouseY: wy,
      guns,
    });
    return true;
  }

  _clearMinimapSetupPreview() {
    const intent = this._intent();
    if (intent?.antiTankGunSetupPreview?.source === "minimap") {
      intent.updateAntiTankGunSetupPreview?.(null);
    }
  }

  _setupPreviewQueued(shiftKey = false) {
    return !!shiftKey || this._intent()?.commandComposer?.shiftPreserved === true;
  }

  _setupPreviewEntities(queued = false) {
    return this._selectedOwnSupportWeapons()
      .map((e) => plannedEntityForIntent(this._intent(), e))
      .map((e) => queued ? supportWeaponSetupPreviewEntity(e) : e);
  }

  _selectedOwnSupportWeapons() {
    const sel = this.state.selectedEntities() || [];
    return sel.filter((e) =>
      ownOwner(this.state, e.owner) &&
      (e.kind === KIND.ANTI_TANK_GUN || e.kind === KIND.ARTILLERY));
  }

  /** Issue the minimap's current command to the world point for any selected own units. */
  _issueOrder(wx, wy, queued = false) {
    const commandTarget = this._intent()?.commandTarget;
    const sel = this.state.selectedEntities() || [];
    if (commandTarget === "setupAntiTankGuns") {
      const supportWeapons = this._selectedOwnSupportWeapons().map((e) => e.id);
      if (supportWeapons.length > 0) {
        this._issueCommand(cmd.setupAntiTankGuns(supportWeapons, wx, wy, queued));
        this._addCommandFeedback("move", wx, wy, queued);
      }
      return;
    }
    const landUnitIds = [];
    for (const e of sel) {
      // Only own, controllable units take move orders (skip buildings/resources/enemies).
      if (ownOwner(this.state, e.owner) && isUnit(e.kind) && e.kind !== KIND.SCOUT_PLANE) {
        landUnitIds.push(e.id);
      }
    }
    if (landUnitIds.length === 0) {
      const producers = sel
        .filter((e) => ownOwner(this.state, e.owner) && isProducerBuilding(e.kind))
        .map((e) => e.id);
      if (producers.length === 0) return;
      const resource = resourceRallyTargetAt(this.state.map, wx, wy);
      if (resource?.kind === KIND.OIL) return;
      const kind = commandTarget === "attack" ? ORDER_STAGE.ATTACK_MOVE : ORDER_STAGE.MOVE;
      const rallyPoint = resource?.kind === KIND.STEEL ? resource : { x: wx, y: wy };
      const node = resource?.kind === KIND.STEEL ? resource.id : null;
      for (const building of producers) {
        this._issueCommand(cmd.setRally(building, rallyPoint.x, rallyPoint.y, queued, kind, node));
      }
      this._addCommandFeedback(kind === ORDER_STAGE.ATTACK_MOVE ? "attack" : "move", rallyPoint.x, rallyPoint.y, queued);
      return;
    }
    if (commandTarget === "attack") {
      if (landUnitIds.length > 0) {
        this._issueCommand(cmd.attackMove(landUnitIds, wx, wy, queued));
      }
      if (landUnitIds.length > 0) {
        this._addCommandFeedback("attack", wx, wy, queued);
      }
      return;
    }
    if (commandTarget?.kind === "ability") {
      const ability = commandTarget.ability;
      const definition = ABILITIES[ability];
      const carriers = definition?.carriers;
      const abilityUnits = Array.isArray(carriers)
        ? sel
            .filter((e) => ownOwner(this.state, e.owner) && carriers.includes(e.kind))
            .map((e) => e.id)
        : landUnitIds;
      if (abilityUnits.length === 0) return;
      const selectedCarriers = sel
        .filter((e) => abilityUnits.includes(e.id))
        .map((e) => plannedEntityForIntent(this._intent(), e));
      const artilleryLocks = isArtilleryFireAbility(ability)
        ? buildArtilleryTargetLocks({
          ability,
          carriers: selectedCarriers,
          rawX: wx,
          rawY: wy,
          map: this.state.map,
          tileSize: this.state.map?.tileSize,
          definition,
          queued,
        })
        : [];
      const radiusTiles = abilityTargetRadiusTiles(definition, ability, this.state);
      this._issueCommand(cmd.useAbility(ability, abilityUnits, wx, wy, queued));
      if (isArtilleryFireAbility(ability)) {
        for (const lock of artilleryLocks) {
          this._addCommandFeedback("artillery", lock.x, lock.y, queued, radiusTiles);
        }
        return;
      }
      this._addCommandFeedback("attack", wx, wy, queued, radiusTiles);
      return;
    }
    this._issueCommand(cmd.move(landUnitIds, wx, wy, queued));
    this._addCommandFeedback("move", wx, wy, queued);
  }
}

function resourceRallyTargetAt(map, x, y) {
  const radius = Math.max(0, Number(map?.tileSize) || 0) * 0.5;
  const radius2 = radius * radius;
  let best = null;
  let bestDist2 = Infinity;
  for (const node of map?.resources || []) {
    if (node?.remaining === 0 || !isResource(node?.kind)) continue;
    const dx = Number(node.x) - x;
    const dy = Number(node.y) - y;
    const dist2 = dx * dx + dy * dy;
    if (!Number.isFinite(dist2) || dist2 > radius2) continue;
    if (dist2 < bestDist2 || (dist2 === bestDist2 && node.id < best?.id)) {
      best = node;
      bestDist2 = dist2;
    }
  }
  return best;
}

function supportWeaponSetupPreviewEntity(entity) {
  const origin = latestMovementOrderPlanPoint(entity);
  return origin ? { ...entity, x: origin.x, y: origin.y } : entity;
}

function plannedEntityForIntent(intent, entity) {
  return typeof intent?.entityWithPlannedOrder === "function"
    ? intent.entityWithPlannedOrder(entity)
    : entity;
}

function latestMovementOrderPlanPoint(entity) {
  if (!Array.isArray(entity?.orderPlan)) return null;
  let origin = null;
  for (const marker of entity.orderPlan) {
    if (
      (marker?.kind === ORDER_STAGE.MOVE || marker?.kind === ORDER_STAGE.ATTACK_MOVE) &&
      Number.isFinite(marker.x) &&
      Number.isFinite(marker.y)
    ) {
      origin = { x: marker.x, y: marker.y };
    }
  }
  return origin;
}

function ownOwner(state, owner) {
  if (state?.controlPolicy?.kind === "lab") {
    if (typeof state.controlPolicy.isCommandOwner === "function") {
      return state.controlPolicy.isCommandOwner(owner, state);
    }
    return state.controlPolicy.canControlOwner(owner, state);
  }
  return typeof state?.isOwnOwner === "function"
    ? state.isOwnOwner(owner)
    : Number(owner) === state?.playerId;
}

function allyOwner(state, owner) {
  if (state?.controlPolicy?.kind === "lab") {
    return typeof state.controlPolicy.isCommandAllyOwner === "function"
      ? state.controlPolicy.isCommandAllyOwner(owner, state)
      : false;
  }
  return typeof state?.isAllyOwner === "function" && state.isAllyOwner(owner);
}

function commandFeedbackOwner(state) {
  if (state?.controlPolicy?.kind === "lab") {
    const owner = typeof state.controlPolicy.feedbackOwner === "function"
      ? state.controlPolicy.feedbackOwner(state)
      : typeof state.controlPolicy.issueAsOwnerForSelection === "function"
        ? state.controlPolicy.issueAsOwnerForSelection(state.selectedEntities?.() || [])
        : null;
    const ownerId = Number(owner);
    return Number.isInteger(ownerId) && ownerId > 0 ? ownerId : null;
  }
  const ownerId = Number(state?.playerId);
  return Number.isInteger(ownerId) && ownerId > 0 ? ownerId : null;
}

function abilityTargetRadiusTiles(definition, ability, state) {
  const baseRadius = definition?.radiusTiles || 0;
  if (ability === ABILITY.SMOKE && commandUpgrades(state).includes(UPGRADE.SMOKE_PLUS)) {
    return definition?.upgradedRadiusTiles || baseRadius;
  }
  return baseRadius;
}

function commandUpgrades(state) {
  if (typeof state?.controlPolicy?.commandUpgrades === "function") {
    const upgrades = state.controlPolicy.commandUpgrades(state);
    return Array.isArray(upgrades) ? upgrades : [];
  }
  return Array.isArray(state?.upgrades) ? state.upgrades : [];
}

function issueGameplayCommand(sender, command) {
  if (sender && typeof sender.issueCommand === "function") {
    return sender.issueCommand(command);
  }
  if (sender && typeof sender.command === "function" && sender.command.length < 2) {
    return sender.command(command);
  }
  return false;
}
