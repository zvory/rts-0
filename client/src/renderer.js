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
  FOG_UNEXPLORED_ALPHA,
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

// Frames an entity id may go unseen before its pooled objects are destroyed and
// dropped. Short enough to keep dead ids from accumulating, long enough that a
// one-frame vision flicker reuses the slot rather than churning it (~2s @60fps).
const SWEEP_EVICT_FRAMES = 120;

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
  "feedback",
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
      antialias: false,
      resolution: window.devicePixelRatio || 1,
      autoDensity: true,
      backgroundColor: COLORS.bgVoid,
      width: canvasParent.clientWidth || window.innerWidth,
      height: canvasParent.clientHeight || window.innerHeight,
    });
    PIXI.settings.SCALE_MODE = PIXI.SCALE_MODES.NEAREST;
    this.app.renderer.roundPixels = true;
    this.app.view.style.imageRendering = "pixelated";
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
    this._feedbackGfx = new PIXI.Graphics();
    this.layers.feedback.addChild(this._feedbackGfx);
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
    // Consecutive-frames-unseen counter per id (across all pools + icons), so we
    // hide briefly but evict after a grace period — server ids are never reused.
    this._unseen = new Map();

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
        let color = terrainColor(code, tx, ty);
        g.beginFill(color);
        g.drawRect(tx * ts, ty * ts, ts, ts);
        g.endFill();

        // Coarse texture blocks keep the ground readable while selling the
        // low-resolution PS1 look. No symbols or national markings are used.
        const blocks = ts >= 32 ? 4 : 2;
        const block = ts / blocks;
        for (let by = 0; by < blocks; by++) {
          for (let bx = 0; bx < blocks; bx++) {
            const n = hash2(tx * 17 + bx, ty * 17 + by);
            if (n < 0.42) continue;
            const overlay = terrainOverlayColor(code, n);
            g.beginFill(overlay, code === 2 ? 0.22 : 0.16);
            g.drawRect(tx * ts + bx * block, ty * ts + by * block, Math.ceil(block), Math.ceil(block));
            g.endFill();
          }
        }

        drawImpassableEdge(g, map, tx, ty, code, ts);
      }
    }

    // Rasterize to a texture so the (potentially huge) terrain is a single sprite.
    const tex = this.app.renderer.generateTexture(g, {
      region: new PIXI.Rectangle(0, 0, map.width * ts, map.height * ts),
      scaleMode: PIXI.SCALE_MODES.NEAREST,
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
    this._drawCommandFeedback(state);
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
   * Low-poly PS1 silhouettes tinted by owner. The shapes are intentionally neutral:
   * no national insignia, flags, stars, crosses, eagles, or historical unit badges.
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
    g.lineStyle(2, 0x1a1712, 0.95);

    if (e.kind === KIND.RIFLEMAN) {
      // Infantry wedge with a short rifle line.
      g.beginFill(tint);
      const a = facing;
      const tip = polar(a, r * 1.2);
      const left = polar(a + 2.45, r * 0.85);
      const right = polar(a - 2.45, r * 0.85);
      g.moveTo(tip.x, tip.y);
      g.lineTo(left.x, left.y);
      g.lineTo(polar(a + Math.PI, r * 0.35).x, polar(a + Math.PI, r * 0.35).y);
      g.lineTo(right.x, right.y);
      g.closePath();
      g.endFill();
      const rifleA = facing - 0.18;
      const muzzle = polar(rifleA, r * 1.45);
      const grip = polar(rifleA + Math.PI, r * 0.15);
      g.lineStyle(2, 0x2a2119, 0.95);
      g.moveTo(grip.x, grip.y);
      g.lineTo(muzzle.x, muzzle.y);
    } else if (e.kind === KIND.MACHINE_GUNNER) {
      // Wider support team block with a braced gun line.
      g.beginFill(tint);
      g.drawPolygon([
        -r * 0.95, -r * 0.55,
        r * 0.75, -r * 0.55,
        r * 1.0, 0,
        r * 0.75, r * 0.55,
        -r * 0.95, r * 0.55,
      ]);
      g.endFill();
      g.lineStyle(3, 0x2a2119, 0.9);
      g.moveTo(-r * 0.25, 0);
      g.lineTo(r * 1.35, 0);
    } else if (e.kind === KIND.AT_TEAM) {
      // Two-person anti-armor marker with a long launcher slash.
      g.beginFill(tint);
      g.moveTo(0, -r);
      g.lineTo(r, 0);
      g.lineTo(0, r);
      g.lineTo(-r, 0);
      g.closePath();
      g.endFill();
      g.beginFill(0x1a1712, 0.65);
      g.drawRect(-r * 0.65, -r * 0.16, r * 1.3, r * 0.32);
      g.endFill();
      g.lineStyle(3, 0x2a2119, 0.9);
      g.moveTo(-r * 0.85, r * 0.45);
      g.lineTo(r * 0.85, -r * 0.45);
    } else if (e.kind === KIND.TANK) {
      // Chunky flat-shaded armor, closer to a low-poly model than an icon.
      g.beginFill(tint);
      g.drawPolygon([
        -r * 1.05, -r * 0.75,
        r * 0.85, -r * 0.75,
        r * 1.08, -r * 0.38,
        r * 1.08, r * 0.52,
        r * 0.72, r * 0.82,
        -r * 0.95, r * 0.82,
        -r * 1.18, r * 0.35,
        -r * 1.18, -r * 0.38,
      ]);
      g.endFill();
      g.beginFill(0x1a1712, 0.28);
      g.drawRect(-r * 0.55, -r * 0.42, r * 1.05, r * 0.82);
      g.endFill();
      const barrel = polar(facing, r * 1.2);
      g.lineStyle(4, 0x241d17, 0.95);
      g.moveTo(0, 0);
      g.lineTo(barrel.x, barrel.y);
    } else {
      // Engineer (and any other unit kind): compact tool-carrying block.
      g.beginFill(tint);
      g.drawPolygon([
        0, -r,
        r * 0.85, -r * 0.25,
        r * 0.55, r * 0.9,
        -r * 0.55, r * 0.9,
        -r * 0.85, -r * 0.25,
      ]);
      g.endFill();
      // Carried-resource pip so harvesters read at a glance.
      if (e.carrying) {
        const cc = e.carryingKind === KIND.OIL ? COLORS.oil : COLORS.steel;
        g.beginFill(cc);
        g.drawRect(-r * 0.35, -r * 1.15, r * 0.7, r * 0.45);
        g.endFill();
      }
    }

    // Facing indicator: a short pale tick from center outward.
    const fp = polar(facing, r + 3);
    g.lineStyle(2, 0xd8d0b0, 0.85);
    g.moveTo(0, 0);
    g.lineTo(fp.x, fp.y);
  }

  /**
   * Blocky footprint tinted by owner, with a plain two-letter stencil. Under
   * construction (`buildProgress < 1`) → translucent with a horizontal progress bar.
   * Producing (`prodProgress`) → a top-edge progress bar.
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
    sh.drawRect(x0 + 4, y0 + 6, w, h);
    sh.endFill();

    const g = this._slot("buildings", e.id);
    g.position.set(0, 0);
    g.lineStyle(2, 0x1a1712, underConstruction ? 0.55 : 0.95);
    g.beginFill(0x2b2a23, bodyAlpha);
    g.drawRect(x0, y0, w, h);
    g.endFill();

    // Player-tinted roof/yard slabs, all neutral geometry.
    g.lineStyle(0);
    g.beginFill(tint, bodyAlpha * 0.82);
    if (e.kind === KIND.INDUSTRIAL_CENTER) {
      g.drawRect(x0 + w * 0.12, y0 + h * 0.18, w * 0.62, h * 0.52);
      g.drawRect(x0 + w * 0.68, y0 + h * 0.1, w * 0.16, h * 0.32);
      g.beginFill(0x1a1712, bodyAlpha * 0.7);
      g.drawRect(x0 + w * 0.76, y0 + h * 0.02, w * 0.08, h * 0.22);
    } else if (e.kind === KIND.BUNKER) {
      g.drawPolygon([
        x0 + w * 0.18, y0 + h * 0.3,
        x0 + w * 0.5, y0 + h * 0.12,
        x0 + w * 0.82, y0 + h * 0.3,
        x0 + w * 0.75, y0 + h * 0.75,
        x0 + w * 0.25, y0 + h * 0.75,
      ]);
      g.beginFill(0x1a1712, bodyAlpha * 0.75);
      g.drawRect(x0 + w * 0.3, y0 + h * 0.44, w * 0.4, h * 0.12);
    } else if (e.kind === KIND.TANK_FACTORY) {
      g.drawRect(x0 + w * 0.12, y0 + h * 0.18, w * 0.76, h * 0.26);
      g.drawRect(x0 + w * 0.18, y0 + h * 0.54, w * 0.64, h * 0.26);
      g.beginFill(0x1a1712, bodyAlpha * 0.55);
      for (let i = 0; i < 3; i++) g.drawRect(x0 + w * (0.2 + i * 0.2), y0 + h * 0.56, w * 0.08, h * 0.22);
    } else if (e.kind === KIND.DEPOT) {
      g.drawRect(x0 + w * 0.16, y0 + h * 0.22, w * 0.68, h * 0.2);
      g.drawRect(x0 + w * 0.16, y0 + h * 0.52, w * 0.68, h * 0.2);
    } else {
      g.drawRect(x0 + w * 0.12, y0 + h * 0.18, w * 0.76, h * 0.56);
      g.beginFill(0x1a1712, bodyAlpha * 0.42);
      g.drawRect(x0 + w * 0.22, y0 + h * 0.26, w * 0.56, h * 0.12);
      g.drawRect(x0 + w * 0.22, y0 + h * 0.5, w * 0.56, h * 0.12);
    }
    g.endFill();

    // Stencil label — pooled Text reused per building id (see _icon).
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
      // Unit production progress bar along the roof line.
      const bw = w * 0.78;
      const bx = e.x - bw / 2;
      const by = y0 + 6;
      g.beginFill(COLORS.hpBack, 0.9);
      g.drawRect(bx, by, bw, 5);
      g.endFill();
      g.beginFill(COLORS.hpGood);
      g.drawRect(bx, by, bw * clamp01(e.prodProgress), 5);
      g.endFill();
    }
  }

  /**
    * Resource node: steel = tan supply crates, oil = olive fuel drums; size/opacity
   * scale with `remaining`. Dimmed when the tile is currently not visible (explored
   * memory) so it reads as a remembered node.
   * @private
   */
  _drawResource(e, fog) {
    const stat = STATS[e.kind] || {};
    const base = stat.size || 11;
    // Scale a little with remaining amount (clamped) so depleted nodes shrink.
    const full = e.kind === KIND.OIL ? 5000 : 1500;
    const frac = e.remaining == null ? 1 : clamp01(e.remaining / full);
    const r = base * (0.55 + 0.45 * frac);

    const ts = (this._map && this._map.tileSize) || 32;
    const visible = !fog || fog.isVisible(Math.floor(e.x / ts), Math.floor(e.y / ts));
    const alpha = visible ? 1 : 0.7;

    const g = this._slot("resources", e.id);
    g.position.set(e.x, e.y);
    g.alpha = alpha;

    if (e.kind === KIND.OIL) {
      // Fuel drums: industrial but faction-neutral.
      g.lineStyle(1.5, 0x1a1712, 0.85);
      g.beginFill(COLORS.oil);
      g.drawRect(-r * 0.75, -r * 0.55, r * 0.48, r * 1.05);
      g.drawRect(-r * 0.18, -r * 0.68, r * 0.5, r * 1.18);
      g.drawRect(r * 0.38, -r * 0.5, r * 0.42, r);
      g.endFill();
      g.lineStyle(0);
      g.beginFill(0x263225, 0.45);
      g.drawRect(-r * 0.72, -r * 0.06, r * 1.48, r * 0.12);
      g.drawRect(-r * 0.16, -r * 0.26, r * 0.46, r * 0.12);
      g.endFill();
    } else {
      // Supply crates: replaces sci-fi crystals with wartime materiel.
      g.lineStyle(1.2, 0x1a1712, 0.85);
      const crates = [
        { dx: -r * 0.45, dy: -r * 0.25, s: 0.65 },
        { dx: r * 0.25, dy: -r * 0.2, s: 0.7 },
        { dx: -r * 0.05, dy: r * 0.35, s: 0.8 },
      ];
      for (const c of crates) {
        const cs = r * c.s;
        g.beginFill(COLORS.steel);
        g.drawRect(c.dx - cs * 0.45, c.dy - cs * 0.35, cs * 0.9, cs * 0.7);
        g.endFill();
        g.lineStyle(1, 0x5a5134, 0.8);
        g.moveTo(c.dx - cs * 0.38, c.dy);
        g.lineTo(c.dx + cs * 0.38, c.dy);
        g.moveTo(c.dx, c.dy - cs * 0.3);
        g.lineTo(c.dx, c.dy + cs * 0.3);
        g.lineStyle(1.2, 0x1a1712, 0.85);
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
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
        fontSize: 24,
        fill: 0xd8d0b0,
        align: "center",
        fontWeight: "700",
      });
      t.anchor.set(0.5);
      this._iconPool.set(e.id, t);
      this.layers.buildings.addChild(t);
    }
    if (t.text !== glyph) t.text = glyph;
    t.visible = true;
    t.alpha = 0.78 * alpha;
    t.position.set(cx, cy);
    // Scale the (fixed-size) glyph to roughly fit the footprint.
    const s = (size * 0.95) / 24;
    t.scale.set(s);
    // Track on the buildings pool's seen-set so the sweep keeps it alive.
    this._seen.buildings.add(e.id);
  }

  // --- Overlays ------------------------------------------------------------

  /**
   * Draw the fog overlay from the Fog grids: unexplored = heavily dimmed, explored =
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
            const color = runLevel === 2 ? COLORS.fogUnexplored : COLORS.fogExplored;
            const a = runLevel === 2 ? FOG_UNEXPLORED_ALPHA : FOG_EXPLORED_ALPHA;
            g.beginFill(color, a);
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

  /** Draw short-lived local markers for issued move / attack commands. @private */
  _drawCommandFeedback(state) {
    const g = this._feedbackGfx;
    g.clear();
    if (!state || typeof state.liveCommandFeedback !== "function") return;

    const now = performance.now();
    for (const f of state.liveCommandFeedback(now)) {
      const age = now - f.createdAt;
      const t = clamp01(age / 650);
      const alpha = (1 - t) * 0.95;
      const r = 12 + t * 10;
      const color = f.kind === "attack" ? COLORS.selectEnemy : COLORS.selectOwn;

      g.lineStyle(2, color, alpha);
      if (f.kind === "attack") {
        g.moveTo(f.x - r, f.y - r);
        g.lineTo(f.x + r, f.y + r);
        g.moveTo(f.x + r, f.y - r);
        g.lineTo(f.x - r, f.y + r);
        g.drawCircle(f.x, f.y, r * 0.72);
      } else {
        g.drawCircle(f.x, f.y, r * 0.72);
        g.moveTo(f.x, f.y - r);
        g.lineTo(f.x + r * 0.72, f.y);
        g.lineTo(f.x, f.y + r);
        g.lineTo(f.x - r * 0.72, f.y);
        g.lineTo(f.x, f.y - r);
      }
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
   * frame. We hide rather than destroy during a brief grace period so entities
   * that flicker out of vision for a frame reuse their slot; once an id has been
   * unseen for SWEEP_EVICT_FRAMES we destroy its objects and drop the map
   * entries (server ids are never reused, so they would otherwise grow forever).
   * @private
   */
  _sweep() {
    // Tally which ids were touched in any pool this frame, then bump/reset the
    // shared per-id unseen counter so an id alive in one layer isn't evicted
    // from another (e.g. a building's footprint + its icon).
    const seenAny = new Set();
    for (const key of Object.keys(this._seen)) {
      for (const id of this._seen[key]) seenAny.add(id);
    }

    const evict = new Set();
    const ids = new Set([...this._unseen.keys()]);
    for (const key of Object.keys(this._pools)) {
      for (const id of this._pools[key].keys()) ids.add(id);
    }
    if (this._iconPool) for (const id of this._iconPool.keys()) ids.add(id);
    for (const id of ids) {
      if (seenAny.has(id)) {
        this._unseen.delete(id);
      } else {
        const n = (this._unseen.get(id) || 0) + 1;
        if (n >= SWEEP_EVICT_FRAMES) evict.add(id);
        else this._unseen.set(id, n);
      }
    }

    for (const key of Object.keys(this._pools)) {
      const pool = this._pools[key];
      const seen = this._seen[key];
      for (const [id, g] of pool) {
        if (seen.has(id)) continue;
        if (evict.has(id)) {
          this.layers[key].removeChild(g);
          g.destroy();
          pool.delete(id);
        } else {
          g.visible = false;
        }
      }
    }
    if (this._iconPool) {
      const seen = this._seen.buildings;
      for (const [id, t] of this._iconPool) {
        if (seen.has(id)) continue;
        if (evict.has(id)) {
          this.layers.buildings.removeChild(t);
          t.destroy();
          this._iconPool.delete(id);
        } else {
          t.visible = false;
        }
      }
    }
    for (const id of evict) this._unseen.delete(id);
  }

  /**
   * Tear down the whole renderer: destroy every pooled and long-lived display
   * object, the terrain texture, detach the canvas, and destroy the PIXI app
   * (and its WebGL context). Guards against being called twice. main.js's
   * Match.destroy() calls this on rematch so we don't leak a context per game.
   */
  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;

    // Per-id pooled Graphics across every layer.
    for (const key of Object.keys(this._pools)) {
      const pool = this._pools[key];
      for (const g of pool.values()) g.destroy();
      pool.clear();
    }
    // Pooled icon Text objects.
    if (this._iconPool) {
      for (const t of this._iconPool.values()) t.destroy();
      this._iconPool.clear();
    }
    this._unseen.clear();

    // Long-lived single Graphics.
    this._fogGfx.destroy();
    this._feedbackGfx.destroy();
    this._placementGfx.destroy();
    this._dragGfx.destroy();

    // Cached terrain sprite + its generated texture.
    if (this._terrainSprite) {
      this._terrainSprite.destroy(true);
      this._terrainSprite = null;
    }

    // Detach the canvas from the DOM, then destroy the app + WebGL context.
    const view = this.app.view;
    if (view && view.parentNode) view.parentNode.removeChild(view);
    this.app.destroy(true, { children: true, texture: true, baseTexture: true });
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

/** Deterministic 0..1 noise for terrain dithering. */
function hash2(x, y) {
  let n = (x * 374761393 + y * 668265263) | 0;
  n = (n ^ (n >>> 13)) | 0;
  n = Math.imul(n, 1274126177);
  return ((n ^ (n >>> 16)) >>> 0) / 4294967295;
}

/** Base color for a terrain tile code. Codes match server terrain constants. */
function terrainColor(code, tx, ty) {
  if (code === 1) return COLORS.rock;
  if (code === 2) return COLORS.water;
  const n = hash2(tx, ty);
  if (n > 0.78) return COLORS.field;
  if (n < 0.18) return COLORS.mud;
  return (tx + ty) % 2 === 0 ? COLORS.grass : COLORS.grassAlt;
}

/** Muted overlay tint for blocky terrain texture. */
function terrainOverlayColor(code, n) {
  if (code === 1) return n > 0.74 ? 0x8a8777 : 0x4f4c43;
  if (code === 2) return n > 0.74 ? 0x527482 : 0x1d3d48;
  return n > 0.74 ? 0x817555 : 0x343127;
}

/** Draw dark perimeter strips only where impassable terrain borders passable ground. */
function drawImpassableEdge(g, map, tx, ty, code, ts) {
  if (!isImpassableTerrain(code)) return;

  const edge = Math.max(3, Math.floor(ts * 0.16));
  const color = code === 2 ? 0x0c2028 : 0x24231f;
  const x = tx * ts;
  const y = ty * ts;

  g.beginFill(color, 0.72);
  if (!isImpassableAt(map, tx, ty - 1)) g.drawRect(x, y, ts, edge);
  if (!isImpassableAt(map, tx, ty + 1)) g.drawRect(x, y + ts - edge, ts, edge);
  if (!isImpassableAt(map, tx - 1, ty)) g.drawRect(x, y, edge, ts);
  if (!isImpassableAt(map, tx + 1, ty)) g.drawRect(x + ts - edge, y, edge, ts);
  g.endFill();
}

function isImpassableAt(map, tx, ty) {
  if (tx < 0 || ty < 0 || tx >= map.width || ty >= map.height) return false;
  return isImpassableTerrain(map.terrain[ty * map.width + tx]);
}

function isImpassableTerrain(code) {
  return code === 1 || code === 2;
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
