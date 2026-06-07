// Minimap — the bottom-left overview canvas (`#minimap`, 220×220). Draws the terrain,
// the fog overlay, entity blips colored by owner, and the current camera viewport
// rectangle. Left-click/drag recenters the camera; right-click issues a context-sensitive
// order for the current own-unit selection. See docs/design/client-ui.md §4.1 (Minimap) and §4.2 (look).
//
// The minimap is a plain 2D canvas (not Pixi). World↔canvas conversion is a uniform
// scale derived from the map's pixel size and the (square) canvas size, letterboxed so
// non-square maps stay centered and undistorted.

import { cmd } from "./protocol.js";
import { TERRAIN, isResource, isUnit } from "./protocol.js";

const isImpassableTerrainCode = (code) => code === TERRAIN.ROCK || code === TERRAIN.WATER;
import { COLORS, FOG_EXPLORED_ALPHA, FOG_UNEXPLORED_ALPHA } from "./config.js";

const PING_MS = 900;
const BORDER_PULSE_MS = 700;

// Convert one of the 0xRRGGBB palette ints into a CSS color string.
const hex = (n) => "#" + n.toString(16).padStart(6, "0");

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
   * @param {import("./camera.js").Camera} camera the game camera (for the viewport rect + recenter).
   * @param {import("./fog.js").Fog} fog the local fog overlay grids.
   * @param {import("./net.js").Net} net network seam for right-click move orders.
   */
  constructor(canvasEl, state, camera, fog, net) {
    this.canvas = canvasEl;
    this.ctx = canvasEl.getContext("2d");
    this.state = state;
    this.camera = camera;
    this.fog = fog;
    this.net = net;

    this.size = canvasEl.width; // assumed square (220 per index.html)

    // Cached world->canvas transform, recomputed when the map first arrives.
    this._scale = 1; // canvas px per world px
    this._offX = 0; // canvas px letterbox offset
    this._offY = 0;
    this._mapW = 0; // map world width/height in px (for invalidation)
    this._mapH = 0;

    this._dragging = false;
    this._pings = [];
    this._borderPulseUntil = 0;

    // Bound handlers retained so destroy() can remove the exact references.
    this._onContextMenu = (ev) => ev.preventDefault();
    this._onCanvasMouseDown = this._handleCanvasMouseDown.bind(this);
    this._onWinMouseMove = this._handleWinMouseMove.bind(this);
    this._onWinMouseUp = this._handleWinMouseUp.bind(this);

    this._installInput();
  }

  // --- Transform -------------------------------------------------------------

  /** (Re)compute the world→canvas scale/offset from the current map dimensions. */
  _ensureTransform() {
    const map = this.state.map;
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
  render() {
    const ctx = this.ctx;
    if (!ctx) return;

    // Void background (outside the map / before a map exists).
    ctx.fillStyle = hex(COLORS.bgVoid);
    ctx.fillRect(0, 0, this.size, this.size);

    if (!this._ensureTransform()) return;

    this._drawTerrain();
    this._drawEntities();
    this._drawFog();
    this._drawResources();
    this._drawViewport();
    this._drawPings(performance.now());
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

  /** Pulse the minimap border when an alert has no resolvable world position. */
  pulseBorder() {
    this._borderPulseUntil = Math.max(this._borderPulseUntil, performance.now() + BORDER_PULSE_MS);
  }

  /** Fill one minimap cell per tile with its terrain color. */
  _drawTerrain() {
    const map = this.state.map;
    const ctx = this.ctx;
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

  /** Draw static resource blips after fog so they are always visible. */
  _drawResources() {
    const map = this.state.map;
    const ctx = this.ctx;
    const r = 2.2;
    for (const node of map.resources || []) {
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

  /**
   * Draw the fog overlay over the terrain/entities: unexplored heavily dimmed, explored but
   * not currently visible dimmed, visible clear. Drawn per tile from the fog grids.
   */
  _drawFog() {
    const map = this.state.map;
    const fog = this.fog;
    if (!fog) return;
    const ctx = this.ctx;
    const ts = map.tileSize;
    const cw = ts * this._scale + 1;
    const ch = ts * this._scale + 1;

    // Stone/water tiles keep only a light wash of fog so the map's shape stays legible.
    const IMPASSABLE_FOG_SCALE = 0.56;
    ctx.save();
    for (let ty = 0; ty < map.height; ty++) {
      for (let tx = 0; tx < map.width; tx++) {
        if (fog.isVisible(tx, ty)) continue; // clear
        const impassable = isImpassableTerrainCode(map.terrain[ty * map.width + tx]);
        const p = this._worldToCanvas(tx * ts, ty * ts);
        if (fog.isExplored(tx, ty)) {
          ctx.globalAlpha = FOG_EXPLORED_ALPHA * (impassable ? IMPASSABLE_FOG_SCALE : 1);
          ctx.fillStyle = hex(COLORS.fogExplored);
        } else {
          ctx.globalAlpha = FOG_UNEXPLORED_ALPHA * (impassable ? IMPASSABLE_FOG_SCALE : 1);
          ctx.fillStyle = hex(COLORS.fogUnexplored);
        }
        ctx.fillRect(p.x, p.y, cw, ch);
      }
    }
    ctx.restore();
  }

  /** Draw a colored blip for each visible entity (own/enemy/neutral). */
  _drawEntities() {
    const ctx = this.ctx;
    const entities = this.state.entitiesInterpolated(1);
    for (const e of entities) {
      const p = this._worldToCanvas(e.x, e.y);
      ctx.fillStyle = this._blipColor(e);
      // Buildings/resources read a touch larger than units so bases stand out.
      const r = e.owner !== 0 && !isResource(e.kind) ? 1.6 : 2.2;
      ctx.fillRect(p.x - r, p.y - r, r * 2, r * 2);
    }
  }

  /** Blip color for an entity: own=green, enemy=red, neutral(resources)=yellow. */
  _blipColor(e) {
    if (e.owner === 0 || isResource(e.kind)) return hex(COLORS.selectNeutral);
    if (e.owner === this.state.playerId) return hex(COLORS.selectOwn);
    // Enemy: prefer the player's assigned color if we know it, else the enemy tint.
    const player = this._playerById(e.owner);
    return (player && player.color) || hex(COLORS.selectEnemy);
  }

  _playerById(id) {
    const players = this.state.players || [];
    return players.find((p) => p.id === id) || null;
  }

  /** Draw the current camera viewport as a thin rectangle. */
  _drawViewport() {
    const view = this._viewportWorldRect();
    if (!view) return;
    const tl = this._worldToCanvas(view.x, view.y);
    const w = view.w * this._scale;
    const h = view.h * this._scale;
    const ctx = this.ctx;
    ctx.save();
    ctx.strokeStyle = "rgba(255,255,255,0.85)";
    ctx.lineWidth = 1;
    ctx.strokeRect(Math.round(tl.x) + 0.5, Math.round(tl.y) + 0.5, Math.round(w), Math.round(h));
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

  /**
   * The camera's current viewport as a world-space rect {x,y,w,h}. The camera exposes
   * top-left world coords (`x`,`y`) and `zoom`; the visible world size is the on-screen
   * viewport size divided by zoom. We read the viewport size the camera was given via
   * `setBounds` (`viewW`/`viewH`) and fall back to the live window size if absent.
   */
  _viewportWorldRect() {
    const cam = this.camera;
    if (!cam) return null;
    const zoom = cam.zoom || 1;
    const viewW = cam.viewW != null ? cam.viewW : window.innerWidth;
    const viewH = cam.viewH != null ? cam.viewH : window.innerHeight;
    return { x: cam.x || 0, y: cam.y || 0, w: viewW / zoom, h: viewH / zoom };
  }

  // --- Input -----------------------------------------------------------------

  /** Install pointer/context listeners for recenter (left) and commands (right). */
  _installInput() {
    const c = this.canvas;
    // Suppress the browser context menu so right-click can mean "move here".
    c.addEventListener("contextmenu", this._onContextMenu);
    c.addEventListener("mousedown", this._onCanvasMouseDown);
    // Move/up on window so a drag that leaves the canvas still tracks & releases.
    window.addEventListener("mousemove", this._onWinMouseMove);
    window.addEventListener("mouseup", this._onWinMouseUp);
  }

  /**
   * Remove all installed listeners (e.g. on game teardown / rematch) so stale
   * minimaps stop driving an old camera. Mirrors Input.destroy().
   */
  destroy() {
    const c = this.canvas;
    c.removeEventListener("contextmenu", this._onContextMenu);
    c.removeEventListener("mousedown", this._onCanvasMouseDown);
    window.removeEventListener("mousemove", this._onWinMouseMove);
    window.removeEventListener("mouseup", this._onWinMouseUp);
  }

  _handleCanvasMouseDown(ev) {
    if (!this._ensureTransform()) return;
    const cp = this._eventToCanvas(ev);
    const w = this._canvasToWorld(cp.x, cp.y);
    if (ev.button === 2) {
      // Right-click: issue a context-sensitive order for the currently selected own units.
      ev.preventDefault();
      this._issueOrder(w.x, w.y);
    } else if (ev.button === 0) {
      ev.preventDefault();
      // Left-click while a command target is armed: issue the command instead of panning.
      if (this.state.commandTarget) {
        this._issueOrder(w.x, w.y);
        this.state.endCommandTarget();
        return;
      }
      // Default: recenter the camera (and start a drag).
      this._dragging = true;
      this.camera.centerOn(w.x, w.y);
    }
  }

  _handleWinMouseMove(ev) {
    if (!this._dragging) return;
    const cp = this._eventToCanvas(ev);
    const w = this._canvasToWorld(cp.x, cp.y);
    this.camera.centerOn(w.x, w.y);
  }

  _handleWinMouseUp() {
    this._dragging = false;
  }

  /** Issue the minimap's current command to the world point for any selected own units. */
  _issueOrder(wx, wy) {
    const sel = this.state.selectedEntities() || [];
    const unitIds = [];
    for (const e of sel) {
      // Only own, controllable units take move orders (skip buildings/resources/enemies).
      if (e.owner === this.state.playerId && isUnit(e.kind)) {
        unitIds.push(e.id);
      }
    }
    if (unitIds.length === 0) return;
    if (this.state.commandTarget === "attack") {
      this.net.command(cmd.attackMove(unitIds, wx, wy));
      this.state.addCommandFeedback("attack", wx, wy);
      return;
    }
    this.net.command(cmd.move(unitIds, wx, wy));
    this.state.addCommandFeedback("move", wx, wy);
  }
}
