// Renderer — PixiJS scene graph + per-frame drawing. See DESIGN.md §4.1 / §4.2.
//
// Owns a single PIXI.Application whose stage holds one `world` container that is
// positioned/scaled from the Camera each frame, plus a screen-space overlay layer
// for the drag selection box. Layers are drawn back-to-front in this order:
//
//   terrain → resources → building-shadows → buildings → unit-shadows → units
//   → selection-rings → hp-bars → fog → placement-ghost → drag-box
//
// Terrain is drawn once into a cached RenderTexture (it never changes mid-match).
// Everything else is redrawn each frame, but per-entity Graphics are pooled and
// reconciled by entity id so we never churn the scene graph: each frame we touch
// the live ids, then hide any pooled object whose id was not seen.
//
// PixiJS v7 is loaded globally as `PIXI`; we never import it.

import {
  COLORS,
  FOG_EXPLORED_ALPHA,
  STATS,
  PLAYER_PALETTE,
} from "./config.js";
import {
  KIND,
  STATE,
  isUnit,
  isBuilding,
  isResource,
} from "./protocol.js";

const TWO_PI = Math.PI * 2;

// Layer names in back-to-front draw order. Index in this array == child index in `world`.
const LAYERS = [
  "terrain",
  "resources",
  "buildingShadows",
  "buildings",
  "unitShadows",
  "units",
  "selectionRings",
  "hpBars",
  "fog",
  "placement",
];

export class Renderer {
  /**
   * @param {HTMLElement} canvasParent element the Pixi canvas is appended to
   */
  constructor(canvasParent) {
    this._parent = canvasParent;

    /** The PIXI.Application. Exposed for the render loop / ticker. */
    this.app = new PIXI.Application({
      antialias: true,
      resolution: window.devicePixelRatio || 1,
      autoDensity: true,
      backgroundColor: COLORS.bgVoid,
      width: canvasParent.clientWidth || window.innerWidth,
      height: canvasParent.clientHeight || window.innerHeight,
    });
    canvasParent.appendChild(this.app.view);

    // World container — moved/scaled by the camera every frame.
    this.world = new PIXI.Container();
    this.world.sortableChildren = false;
    this.app.stage.addChild(this.world);

    // One persistent container per layer, in the documented draw order.
    /** @type {Record<string, PIXI.Container>} */
    this.layers = {};
    for (const name of LAYERS) {
      const c = new PIXI.Container();
      this.layers[name] = c;
      this.world.addChild(c);
    }

    // Long-lived single Graphics for the bulk overlays / per-frame vector draws.
    this._terrainSprite = null; // PIXI.Sprite of the cached terrain RenderTexture
    this._fogGfx = new PIXI.Graphics();
    this.layers.fog.addChild(this._fogGfx);
    this._placementGfx = new PIXI.Graphics();
    this.layers.placement.addChild(this._placementGfx);

    // Drag selection box lives in screen space (not affected by the camera).
    this._dragGfx = new PIXI.Graphics();
    this.app.stage.addChild(this._dragGfx);

    // Pools of reusable Graphics keyed by entity id, one per visual layer that
    // hosts per-entity objects. Reconciled by id each frame (see _reconcile).
    this._pools = {
      resources: new Map(),
      buildingShadows: new Map(),
      buildings: new Map(),
      unitShadows: new Map(),
      units: new Map(),
      selectionRings: new Map(),
      hpBars: new Map(),
    };
    // Ids touched this frame, per pool, so we can hide stale entries afterwards.
    this._seen = {};
    for (const key of Object.keys(this._pools)) this._seen[key] = new Set();

    /** Map metadata captured by buildStaticMap (tileSize, width, height in tiles). */
    this._map = null;
  }

  /**
   * Resize the renderer and any full-screen overlays.
   * @param {number} w width in screen px
   * @param {number} h height in screen px
   */
  resize(w, h) {
    this.app.renderer.resize(w, h);
  }

  /**
   * Draw the terrain ONCE into a cached RenderTexture and mount it as the terrain
   * layer's only child. Cheap to call again (it rebuilds the texture); the per-frame
   * `render` never redraws terrain.
   * @param {{width:number,height:number,tileSize:number,terrain:number[]}} map
   */
  buildStaticMap(map) {
    this._map = { width: map.width, height: map.height, tileSize: map.tileSize };
    const ts = map.tileSize;

    const g = new PIXI.Graphics();
    for (let ty = 0; ty < map.height; ty++) {
      for (let tx = 0; tx < map.width; tx++) {
        const code = map.terrain[ty * map.width + tx];
        let color;
        if (code === 1) color = COLORS.rock; // TERRAIN.ROCK
        else if (code === 2) color = COLORS.water; // TERRAIN.WATER
        else color = (tx + ty) % 2 === 0 ? COLORS.grass : COLORS.grassAlt; // checker
        g.beginFill(color);
        g.drawRect(tx * ts, ty * ts, ts, ts);
        g.endFill();
      }
    }

    // Rasterize to a texture so the (potentially huge) terrain is a single sprite.
    const tex = this.app.renderer.generateTexture(g, {
      region: new PIXI.Rectangle(0, 0, map.width * ts, map.height * ts),
    });
    g.destroy();

    const layer = this.layers.terrain;
    if (this._terrainSprite) {
      this._terrainSprite.destroy(true);
      layer.removeChildren();
    }
    this._terrainSprite = new PIXI.Sprite(tex);
    layer.addChild(this._terrainSprite);
  }

  /**
   * Per-frame draw. Positions the world container from the camera, then draws all
   * interpolated entities, fog, selection state and the placement ghost.
   * @param {import("./state.js").GameState} state
   * @param {import("./camera.js").Camera} camera
   * @param {import("./fog.js").Fog} fog
   * @param {number} alpha interpolation factor 0..1 between the two latest snapshots
   */
  render(state, camera, fog, alpha) {
    // Drive the world container from the camera (single transform for all layers).
    this.world.position.set(-camera.x * camera.zoom, -camera.y * camera.zoom);
    this.world.scale.set(camera.zoom);

    // Begin a fresh reconciliation pass.
    for (const key of Object.keys(this._seen)) this._seen[key].clear();

    const entities = state.entitiesInterpolated(alpha) || [];
    const selection = state.selection || new Set();
    const colorByOwner = this._ownerColors(state);

    // Two passes so silhouettes layer correctly: resources + buildings first
    // (footprints sit under units), then units. Selection rings / hp bars are
    // their own layers and are filled inline.
    for (const e of entities) {
      if (isResource(e.kind)) this._drawResource(e, fog);
      else if (isBuilding(e.kind)) this._drawBuilding(e, colorByOwner, state);
    }
    for (const e of entities) {
      if (isUnit(e.kind)) this._drawUnit(e, colorByOwner, state);
    }
    // Selection rings + HP bars after shapes are placed so they read on top.
    for (const e of entities) {
      this._drawSelectionAndHp(e, selection, state);
    }

    // Hide pooled objects whose id was not touched this frame.
    this._sweep();

    // Overlays.
    this._drawFog(fog);
    this._drawPlacement(state, fog);
  }

  // --- Entity drawing ------------------------------------------------------

  /**
   * Resolve a per-owner tint table for this frame: own player uses their assigned
   * color, others use their own color from the players list, neutral falls back to
   * a muted grey (resources are drawn with their own palette, not the owner tint).
   * @private
   * @returns {Map<number, number>} owner id -> 0xRRGGBB
   */
  _ownerColors(state) {
    const out = new Map();
    const players = state.players || [];
    for (let i = 0; i < players.length; i++) {
      const p = players[i];
      out.set(p.id, hexToInt(p.color || PLAYER_PALETTE[i % PLAYER_PALETTE.length]));
    }
    return out;
  }

  /**
   * @private
   * @returns {number} tint for an owned entity (grey for neutral / unknown owner)
   */
  _tintFor(owner, colorByOwner) {
    if (owner === 0) return 0x9aa0a8;
    return colorByOwner.get(owner) ?? 0x9aa0a8;
  }

  /**
   * Fetch (or lazily create) a pooled Graphics for `id` in `poolName`, mark it seen,
   * make it visible, and clear it ready for redraw.
   * @private
   * @returns {PIXI.Graphics}
   */
  _slot(poolName, id) {
    const pool = this._pools[poolName];
    let g = pool.get(id);
    if (!g) {
      g = new PIXI.Graphics();
      pool.set(id, g);
      this.layers[poolName].addChild(g);
    }
    this._seen[poolName].add(id);
    g.visible = true;
    g.clear();
    return g;
  }

  /** Soft circular drop shadow at (cx,cy) with the given radius. @private */
  _shadow(g, cx, cy, radius) {
    g.beginFill(COLORS.shadow, 0.28);
    g.drawEllipse(cx, cy + radius * 0.35, radius, radius * 0.6);
    g.endFill();
  }

  /**
   * Worker = small rounded body, soldier = forward chevron/triangle, heavy = chunky
   * rounded square. All tinted by owner, with a soft shadow, thin dark outline, and a
   * short facing indicator pointing along `facing`.
   * @private
   */
  _drawUnit(e, colorByOwner, state) {
    const stat = STATS[e.kind] || {};
    const r = stat.size || 9;
    const tint = this._tintFor(e.owner, colorByOwner);
    const facing = typeof e.facing === "number" ? e.facing : 0;

    // Shadow on its own layer (under all units).
    const sh = this._slot("unitShadows", e.id);
    sh.position.set(e.x, e.y);
    this._shadow(sh, 0, 0, r);

    // Body on the unit layer.
    const g = this._slot("units", e.id);
    g.position.set(e.x, e.y);
    g.lineStyle(1.5, 0x0a0c10, 0.9);

    if (e.kind === KIND.SOLDIER) {
      // Chevron/triangle pointing along facing.
      g.beginFill(tint);
      const a = facing;
      const tip = polar(a, r * 1.15);
      const left = polar(a + 2.5, r);
      const right = polar(a - 2.5, r);
      g.moveTo(tip.x, tip.y);
      g.lineTo(left.x, left.y);
      g.lineTo(right.x, right.y);
      g.closePath();
      g.endFill();
    } else if (e.kind === KIND.HEAVY) {
      // Chunky rounded square with a darker inner plate.
      g.beginFill(tint);
      g.drawRoundedRect(-r, -r, r * 2, r * 2, r * 0.4);
      g.endFill();
      g.beginFill(0x000000, 0.18);
      g.drawRoundedRect(-r * 0.5, -r * 0.5, r, r, r * 0.25);
      g.endFill();
    } else {
      // Worker (and any other unit kind): small rounded body.
      g.beginFill(tint);
      g.drawCircle(0, 0, r);
      g.endFill();
      // Carried-resource pip so harvesters read at a glance.
      if (e.carrying) {
        const cc = e.carryingKind === KIND.GAS ? COLORS.gas : COLORS.minerals;
        g.beginFill(cc);
        g.drawCircle(0, -r * 0.9, r * 0.35);
        g.endFill();
      }
    }

    // Facing indicator: a short bright tick from center outward.
    const fp = polar(facing, r + 3);
    g.lineStyle(2, 0xffffff, 0.85);
    g.moveTo(0, 0);
    g.lineTo(fp.x, fp.y);
  }

  /**
   * Rounded-rectangle footprint tinted by owner, with an icon glyph. Under
   * construction (`buildProgress < 1`) → translucent with a horizontal progress bar.
   * Producing (`prodProgress`) → a small progress arc in the corner.
   * @private
   */
  _drawBuilding(e, colorByOwner, state) {
    const stat = STATS[e.kind] || {};
    const ts = (this._map && this._map.tileSize) || 32;
    const w = (stat.footW || 2) * ts;
    const h = (stat.footH || 2) * ts;
    const tint = this._tintFor(e.owner, colorByOwner);
    const x0 = e.x - w / 2;
    const y0 = e.y - h / 2;

    const underConstruction = typeof e.buildProgress === "number" && e.buildProgress < 1;
    const bodyAlpha = underConstruction ? 0.45 : 1;

    // Shadow (slightly offset, under buildings).
    const sh = this._slot("buildingShadows", e.id);
    sh.position.set(0, 0);
    sh.beginFill(COLORS.shadow, 0.3);
    sh.drawRoundedRect(x0 + 3, y0 + 5, w, h, 6);
    sh.endFill();

    const g = this._slot("buildings", e.id);
    g.position.set(0, 0);
    g.lineStyle(2, 0x0a0c10, underConstruction ? 0.55 : 0.9);
    g.beginFill(tint, bodyAlpha);
    g.drawRoundedRect(x0, y0, w, h, 6);
    g.endFill();
    // Inner darker plate for depth.
    g.lineStyle(0);
    g.beginFill(0x000000, underConstruction ? 0.1 : 0.18);
    g.drawRoundedRect(x0 + w * 0.18, y0 + h * 0.18, w * 0.64, h * 0.64, 4);
    g.endFill();

    // Icon glyph — pooled Text reused per building id (see _icon).
    this._icon(e, e.x, e.y, Math.min(w, h) * 0.5, bodyAlpha);

    if (underConstruction) {
      // Construction progress bar across the footprint base.
      const bw = w * 0.8;
      const bx = e.x - bw / 2;
      const by = y0 + h - 6;
      g.beginFill(COLORS.hpBack, 0.85);
      g.drawRect(bx, by, bw, 4);
      g.endFill();
      g.beginFill(COLORS.hpGood);
      g.drawRect(bx, by, bw * clamp01(e.buildProgress), 4);
      g.endFill();
    } else if (typeof e.prodProgress === "number" && e.prodProgress > 0) {
      // Production progress arc in the top-right corner.
      const ax = x0 + w - 9;
      const ay = y0 + 9;
      const rr = 6;
      g.lineStyle(2.5, COLORS.hpBack, 0.85);
      g.arc(ax, ay, rr, 0, TWO_PI);
      g.lineStyle(2.5, COLORS.hpGood, 1);
      const start = -Math.PI / 2;
      g.arc(ax, ay, rr, start, start + TWO_PI * clamp01(e.prodProgress));
    }
  }

  /**
   * Resource node: minerals = cyan crystal cluster, gas = green geyser; size/opacity
   * scale with `remaining`. Dimmed when the tile is currently not visible (explored
   * memory) so it reads as a remembered node.
   * @private
   */
  _drawResource(e, fog) {
    const stat = STATS[e.kind] || {};
    const base = stat.size || 11;
    // Scale a little with remaining amount (clamped) so depleted nodes shrink.
    const full = e.kind === KIND.GAS ? 5000 : 1500;
    const frac = e.remaining == null ? 1 : clamp01(e.remaining / full);
    const r = base * (0.55 + 0.45 * frac);

    const ts = (this._map && this._map.tileSize) || 32;
    const visible = !fog || fog.isVisible(Math.floor(e.x / ts), Math.floor(e.y / ts));
    const alpha = visible ? 1 : 0.7;

    const g = this._slot("resources", e.id);
    g.position.set(e.x, e.y);
    g.alpha = alpha;

    if (e.kind === KIND.GAS) {
      // Geyser: a rounded green mound with a couple of vents.
      g.lineStyle(1.5, 0x0a0c10, 0.8);
      g.beginFill(COLORS.gas);
      g.drawEllipse(0, 0, r, r * 0.8);
      g.endFill();
      g.lineStyle(0);
      g.beginFill(0x0a2c18, 0.5);
      g.drawCircle(-r * 0.35, -r * 0.1, r * 0.22);
      g.drawCircle(r * 0.3, r * 0.05, r * 0.18);
      g.endFill();
    } else {
      // Minerals: a small cluster of cyan crystals.
      g.lineStyle(1.2, 0x0a0c10, 0.7);
      const shards = [
        { dx: 0, dy: -r * 0.2, s: 1 },
        { dx: -r * 0.7, dy: r * 0.25, s: 0.75 },
        { dx: r * 0.7, dy: r * 0.25, s: 0.75 },
      ];
      for (const sh of shards) {
        const cs = r * 0.55 * sh.s;
        g.beginFill(COLORS.minerals);
        g.moveTo(sh.dx, sh.dy - cs);
        g.lineTo(sh.dx + cs * 0.6, sh.dy);
        g.lineTo(sh.dx, sh.dy + cs);
        g.lineTo(sh.dx - cs * 0.6, sh.dy);
        g.closePath();
        g.endFill();
      }
    }
  }

  /**
   * Selection ring (own=green, enemy=red, neutral=yellow) for selected entities, and
   * an HP bar above any entity that is damaged or selected.
   * @private
   */
  _drawSelectionAndHp(e, selection, state) {
    const selected = selection.has(e.id);
    const damaged = e.maxHp && e.hp < e.maxHp;

    if (selected) {
      const g = this._slot("selectionRings", e.id);
      g.position.set(e.x, e.y);
      const ring = this._ringRadius(e);
      let color;
      if (e.owner === state.playerId) color = COLORS.selectOwn;
      else if (e.owner === 0) color = COLORS.selectNeutral;
      else color = COLORS.selectEnemy;
      // Glow + crisp ring.
      g.lineStyle(4, color, 0.25);
      g.drawEllipse(0, ring.cy, ring.rx, ring.ry);
      g.lineStyle(2, color, 0.95);
      g.drawEllipse(0, ring.cy, ring.rx, ring.ry);
    }

    if (damaged || selected) {
      const g = this._slot("hpBars", e.id);
      g.position.set(0, 0);
      this._hpBar(g, e);
    }
  }

  // --- Geometry helpers for rings / hp bars --------------------------------

  /**
   * Footprint-aware selection ring geometry (slightly flattened ellipse hugging the
   * base of the silhouette).
   * @private
   * @returns {{rx:number, ry:number, cy:number}}
   */
  _ringRadius(e) {
    const stat = STATS[e.kind] || {};
    if (isBuilding(e.kind)) {
      const ts = (this._map && this._map.tileSize) || 32;
      const w = (stat.footW || 2) * ts;
      const h = (stat.footH || 2) * ts;
      return { rx: w * 0.6, ry: h * 0.42, cy: 0 };
    }
    const r = (stat.size || 9) + 4;
    return { rx: r, ry: r * 0.7, cy: r * 0.35 };
  }

  /**
   * Draw an HP bar centered above the entity. Color steps from good→mid→low.
   * @private
   */
  _hpBar(g, e) {
    if (!e.maxHp) return;
    const frac = clamp01(e.hp / e.maxHp);
    const stat = STATS[e.kind] || {};
    let halfW;
    let topY;
    if (isBuilding(e.kind)) {
      const ts = (this._map && this._map.tileSize) || 32;
      const w = (stat.footW || 2) * ts;
      const h = (stat.footH || 2) * ts;
      halfW = Math.min(w * 0.45, 28);
      topY = e.y - h / 2 - 8;
    } else {
      const r = stat.size || 9;
      halfW = Math.max(10, r);
      topY = e.y - r - 8;
    }
    const x0 = e.x - halfW;
    const barW = halfW * 2;
    const barH = 4;

    g.beginFill(COLORS.hpBack, 0.9);
    g.drawRect(x0 - 1, topY - 1, barW + 2, barH + 2);
    g.endFill();

    let color = COLORS.hpGood;
    if (frac <= 0.33) color = COLORS.hpLow;
    else if (frac <= 0.66) color = COLORS.hpMid;
    g.beginFill(color);
    g.drawRect(x0, topY, barW * frac, barH);
    g.endFill();
  }

  // --- Icon glyphs (pooled Text) -------------------------------------------

  /**
   * Draw / reposition the building's icon glyph. PIXI.Text objects are pooled by
   * entity id on the buildings layer alongside the footprint Graphics.
   * @private
   */
  _icon(e, cx, cy, size, alpha) {
    if (!this._iconPool) this._iconPool = new Map();
    let t = this._iconPool.get(e.id);
    const glyph = (STATS[e.kind] && STATS[e.kind].icon) || "?";
    if (!t) {
      t = new PIXI.Text(glyph, {
        fontFamily: "system-ui, sans-serif",
        fontSize: 24,
        fill: 0xffffff,
        align: "center",
      });
      t.anchor.set(0.5);
      this._iconPool.set(e.id, t);
      this.layers.buildings.addChild(t);
    }
    if (t.text !== glyph) t.text = glyph;
    t.visible = true;
    t.alpha = 0.92 * alpha;
    t.position.set(cx, cy);
    // Scale the (fixed-size) glyph to roughly fit the footprint.
    const s = (size * 1.4) / 24;
    t.scale.set(s);
    // Track on the buildings pool's seen-set so the sweep keeps it alive.
    this._seen.buildings.add(e.id);
  }

  // --- Overlays ------------------------------------------------------------

  /**
   * Draw the fog overlay from the Fog grids: unexplored = solid dark, explored =
   * dimmed at FOG_EXPLORED_ALPHA, visible = clear. Rendered in world space over the
   * whole map; merged into horizontal runs per row to keep the rect count low.
   * @private
   */
  _drawFog(fog) {
    const g = this._fogGfx;
    g.clear();
    if (!fog || !this._map) return;
    const ts = this._map.tileSize;
    const w = fog.width;
    const h = fog.height;

    for (let ty = 0; ty < h; ty++) {
      // Run-length merge contiguous tiles sharing a fog level (0=clear,1=dim,2=dark).
      let runStart = 0;
      let runLevel = this._fogLevel(fog, 0, ty);
      for (let tx = 1; tx <= w; tx++) {
        const level = tx < w ? this._fogLevel(fog, tx, ty) : -1;
        if (level !== runLevel) {
          if (runLevel > 0) {
            const a = runLevel === 2 ? 1 : FOG_EXPLORED_ALPHA;
            g.beginFill(COLORS.fogUnexplored, a);
            g.drawRect(runStart * ts, ty * ts, (tx - runStart) * ts, ts);
            g.endFill();
          }
          runStart = tx;
          runLevel = level;
        }
      }
    }
  }

  /**
   * @private
   * @returns {0|1|2} 0 visible (clear), 1 explored (dim), 2 unexplored (dark)
   */
  _fogLevel(fog, tx, ty) {
    if (fog.isVisible(tx, ty)) return 0;
    if (fog.isExplored(tx, ty)) return 1;
    return 2;
  }

  /**
   * Draw the build placement ghost from `state.placement`: footprint-sized
   * rounded-rect tinted green (valid) or red (invalid), at the candidate tile.
   * @private
   */
  _drawPlacement(state, fog) {
    const g = this._placementGfx;
    g.clear();
    const p = state.placement;
    if (!p) return;
    const ts = (this._map && this._map.tileSize) || 32;
    const stat = STATS[p.building] || {};
    const w = (stat.footW || 2) * ts;
    const h = (stat.footH || 2) * ts;
    const x0 = p.tileX * ts;
    const y0 = p.tileY * ts;
    const color = p.valid ? COLORS.placeOk : COLORS.placeBad;

    g.lineStyle(2, color, 0.95);
    g.beginFill(color, 0.25);
    g.drawRoundedRect(x0, y0, w, h, 6);
    g.endFill();

    // Per-tile grid hint inside the footprint so the snap target is obvious.
    g.lineStyle(1, color, 0.4);
    for (let i = 1; i < (stat.footW || 2); i++) {
      g.moveTo(x0 + i * ts, y0);
      g.lineTo(x0 + i * ts, y0 + h);
    }
    for (let j = 1; j < (stat.footH || 2); j++) {
      g.moveTo(x0, y0 + j * ts);
      g.lineTo(x0 + w, y0 + j * ts);
    }
  }

  /**
   * Draw the drag selection rectangle in SCREEN space, or clear it when passed null.
   * @param {{x:number,y:number,w:number,h:number}|null} rect screen-space rect
   */
  drawSelectionBox(rect) {
    const g = this._dragGfx;
    g.clear();
    if (!rect) return;
    const { x, y, w, h } = normRect(rect);
    g.lineStyle(1.5, COLORS.dragBox, 0.95);
    g.beginFill(COLORS.dragBox, 0.12);
    g.drawRect(x, y, w, h);
    g.endFill();
  }

  // --- Pool maintenance ----------------------------------------------------

  /**
   * Hide every pooled object (per layer + icons) whose id was not touched this
   * frame. We hide rather than destroy so re-appearing entities reuse their slot.
   * @private
   */
  _sweep() {
    for (const key of Object.keys(this._pools)) {
      const pool = this._pools[key];
      const seen = this._seen[key];
      for (const [id, g] of pool) {
        if (!seen.has(id)) g.visible = false;
      }
    }
    if (this._iconPool) {
      const seen = this._seen.buildings;
      for (const [id, t] of this._iconPool) {
        if (!seen.has(id)) t.visible = false;
      }
    }
  }
}

// --- Small pure helpers ----------------------------------------------------

/** Clamp a number to [0,1]. */
function clamp01(v) {
  if (v == null || Number.isNaN(v)) return 0;
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

/** Point at angle `a` (radians) and distance `d` from the origin. */
function polar(a, d) {
  return { x: Math.cos(a) * d, y: Math.sin(a) * d };
}

/** Normalize a possibly-negative-size rect to positive width/height. */
function normRect(r) {
  const x = Math.min(r.x, r.x + r.w);
  const y = Math.min(r.y, r.y + r.h);
  return { x, y, w: Math.abs(r.w), h: Math.abs(r.h) };
}

/**
 * Parse a CSS color string ("#rrggbb" or "#rgb") to a 0xRRGGBB int. Already-numeric
 * inputs pass through. Falls back to a neutral grey on anything unexpected.
 */
function hexToInt(c) {
  if (typeof c === "number") return c;
  if (typeof c !== "string") return 0x9aa0a8;
  let s = c.trim().replace(/^#/, "");
  if (s.length === 3) s = s.split("").map((ch) => ch + ch).join("");
  const n = parseInt(s, 16);
  return Number.isNaN(n) ? 0x9aa0a8 : n;
}
