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
  TANK_BODY,
  PLAYER_PALETTE,
  RESOURCE_AMOUNTS,
  isProducerBuilding,
} from "./config.js";
import {
  KIND,
  SETUP,
  STATE,
  isUnit,
  isBuilding,
  isResource,
} from "./protocol.js";

// Frames an entity id may go unseen before its pooled objects are destroyed and
// dropped. Short enough to keep dead ids from accumulating, long enough that a
// one-frame vision flicker reuses the slot rather than churning it (~2s @60fps).
const SWEEP_EVICT_FRAMES = 120;

// Machine-gunner setup / teardown visuals are time-based on the client so the
// transition reads smoothly between snapshots.
const MACHINE_GUNNER_ANIM_MS = 1000;
const WEAPON_RECOIL_PX = {
  [KIND.RIFLEMAN]: 8.0,
  [KIND.MACHINE_GUNNER]: 5.5,
  [KIND.AT_TEAM]: 13.0,
  [KIND.TANK]: 9.0,
};
const ZERO_OFFSET = Object.freeze({ x: 0, y: 0 });

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
    // Keep interpolated entity positions fractional. Nearest scaling preserves
    // the low-res look without snapping smooth server-snapshot interpolation.
    this.app.renderer.roundPixels = false;
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
    // Local animation state for machine-gunner setup / teardown visuals.
    this._setupVisuals = new Map();
    // Local visual-only track phase for tanks. The server owns movement; this
    // just turns interpolated distance/facing deltas into tread offsets.
    this._tankMotion = new Map();

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
    this._map = { width: map.width, height: map.height, tileSize: map.tileSize, terrain: map.terrain };
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
    const liveIds = new Set();

    // Nodes currently being mined: any worker latched to them. Used by
    // _drawResource to overlay an X marker.
    this._miningNodes = new Set();
    for (const e of entities) {
      if (e.latchedNode) this._miningNodes.add(e.latchedNode);
    }

    // Two passes so silhouettes layer correctly: resources + buildings first
    // (footprints sit under units), then units. Selection rings / hp bars are
    // their own layers and are filled inline.
    for (const e of entities) {
      liveIds.add(e.id);
      if (isResource(e.kind)) this._drawResource(e, fog);
      else if (isBuilding(e.kind)) this._drawBuilding(e, colorByOwner, state);
    }
    for (const e of entities) {
      liveIds.add(e.id);
      if (isUnit(e.kind)) this._drawUnit(e, colorByOwner, state);
    }
    // Selection rings + HP bars after shapes are placed so they read on top.
    for (const e of entities) {
      liveIds.add(e.id);
      this._drawSelectionAndHp(e, selection, state);
    }

    // Hide pooled objects whose id was not touched this frame.
    this._sweep();
    this._sweepSetupVisuals(liveIds);
    this._sweepTankMotion(liveIds);

    // Overlays.
    this._drawFog(fog);
    this._drawCommandFeedback(state);
    this._drawRallyPoints(state);
    this._drawResourceMiningPreview(state);
    this._drawMuzzleFlashes(state);
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
   * Resolve the current visual transition progress for a machine gunner.
   * The server only sends the discrete setup state, so the client smooths the
   * transition locally between snapshots.
   * @private
   * @param {{id:number, setupState?:string}} e
   * @returns {{prongFactor:number, barrel:boolean}}
   */
  _machineGunnerSetupVisual(e) {
    const now = performance.now();
    const setupState = e.setupState || SETUP.PACKED;
    const prev = this._setupVisuals.get(e.id);
    if (!prev || prev.state !== setupState) {
      this._setupVisuals.set(e.id, { state: setupState, changedAt: now });
    }
    const rec = this._setupVisuals.get(e.id);
    const elapsed = now - rec.changedAt;
    const t = smoothstep01(elapsed / MACHINE_GUNNER_ANIM_MS);

    if (setupState === SETUP.SETTING_UP) {
      return { prongFactor: t, barrel: false };
    }
    if (setupState === SETUP.TEARING_DOWN) {
      return { prongFactor: 1 - t, barrel: false };
    }
    if (setupState === SETUP.DEPLOYED) {
      return { prongFactor: 1, barrel: e.state !== STATE.MOVE };
    }
    return { prongFactor: 0, barrel: false };
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
   * Drop setup-animation state for entities that are no longer visible.
   * @private
   * @param {Set<number>} liveIds
   */
  _sweepSetupVisuals(liveIds) {
    for (const id of [...this._setupVisuals.keys()]) {
      if (!liveIds.has(id)) this._setupVisuals.delete(id);
    }
  }

  /**
   * Drop track-animation state for tanks that are no longer visible.
   * @private
   * @param {Set<number>} liveIds
   */
  _sweepTankMotion(liveIds) {
    for (const id of [...this._tankMotion.keys()]) {
      if (!liveIds.has(id)) this._tankMotion.delete(id);
    }
  }

  /**
   * Derive visual tread movement from actual interpolated tank movement.
   * @private
   * @param {{id:number,x:number,y:number,owner:number,state?:string}} e
   * @param {number} facing
   * @param {import("./state.js").GameState} state
   * @param {{halfLen:number,halfWidth:number}} body
   */
  _tankMotionVisual(e, facing, state, body) {
    const prev = this._tankMotion.get(e.id);
    let leftPhase = prev ? prev.leftPhase : 0;
    let rightPhase = prev ? prev.rightPhase : 0;
    let leftDir = 0;
    let rightDir = 0;
    let activity = 0;

    if (prev) {
      const dx = e.x - prev.x;
      const dy = e.y - prev.y;
      const dist = Math.hypot(dx, dy);
      const turn = angleDelta(prev.facing, facing);
      const avgFacing = prev.facing + turn * 0.5;
      const forward = Math.cos(avgFacing);
      const forwardY = Math.sin(avgFacing);
      const forwardMove = dx * forward + dy * forwardY;
      const lateralMove = -dx * forwardY + dy * forward;
      const drive = Math.abs(forwardMove) >= Math.abs(lateralMove) * 0.5
        ? forwardMove
        : Math.sign(forwardMove || 1) * dist;
      const turnTravel = turn * body.halfWidth;
      const leftDelta = drive - turnTravel;
      const rightDelta = drive + turnTravel;
      leftPhase += leftDelta;
      rightPhase += rightDelta;
      leftDir = Math.sign(leftDelta);
      rightDir = Math.sign(rightDelta);
      activity = clamp01((Math.abs(leftDelta) + Math.abs(rightDelta)) / 4);
    }

    const ownTank = e.owner === state.playerId;
    const oil = state.resources ? state.resources.oil : null;
    const oilStarved = ownTank && oil === 0 && (e.state === STATE.MOVE || e.state === STATE.ATTACK);
    const lowOil = ownTank && typeof oil === "number" && oil > 0 && oil <= 5;
    const next = { x: e.x, y: e.y, facing, leftPhase, rightPhase };
    this._tankMotion.set(e.id, next);
    return { leftPhase, rightPhase, leftDir, rightDir, activity, lowOil, oilStarved };
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
    const weaponFacing = typeof e.weaponFacing === "number" ? e.weaponFacing : facing;
    const recoilProgress = typeof state.weaponRecoil === "function"
      ? state.weaponRecoil(e.id, e.kind, performance.now())
      : 0;
    const recoil = weaponRecoilOffset(e.kind, recoilProgress);
    const tankKick = e.kind === KIND.TANK
      ? recoilVector(weaponFacing, recoil * 0.85)
      : ZERO_OFFSET;

    // Shadow on its own layer (under all units).
    const sh = this._slot("unitShadows", e.id);
    sh.position.set(e.x + tankKick.x, e.y + tankKick.y);
    this._shadow(sh, 0, 0, isVehicleBodyKind(e.kind) ? tankBodyVisual(stat).shadowRadius : r);

    // Body on the unit layer.
    const g = this._slot("units", e.id);
    g.position.set(e.x + tankKick.x, e.y + tankKick.y);
    g.lineStyle(2, 0x1a1712, 0.95);

    if (e.kind === KIND.RIFLEMAN || e.kind === KIND.MACHINE_GUNNER || e.kind === KIND.AT_TEAM) {
      drawInfantryBase(g, r, tint, facing);
      if (e.kind === KIND.RIFLEMAN) {
        drawInfantryRifle(g, r, facing, recoil);
      } else if (e.kind === KIND.AT_TEAM) {
        drawInfantryPanzerfaust(g, r, facing, recoil);
      } else {
        drawInfantryMachineGun(g, r, facing, weaponFacing, this._machineGunnerSetupVisual(e), recoil);
      }
    } else if (e.kind === KIND.SCOUT_CAR) {
      // Scout cars currently use the tank-like vehicle movement model server-side.
      // Replace with truck/wheeled movement semantics once that model exists.
      const body = tankBodyVisual(STATS[e.kind]);
      const motion = this._tankMotionVisual(e, facing, state, body);
      drawScoutCar(g, body, tint, facing, weaponFacing, motion, recoil);
    } else if (e.kind === KIND.TANK) {
      // Hull follows movement facing; turret/barrel follow weapon facing.
      const body = tankBodyVisual(STATS[e.kind]);
      const motion = this._tankMotionVisual(e, facing, state, body);
      drawTankTracks(g, body, facing, motion);
      drawTankHull(g, body, tint, facing);

      const barrel = polar(weaponFacing, Math.max(body.halfLen * 0.8, body.halfLen + 8 - recoil));
      g.lineStyle(5, 0x241d17, 0.95);
      g.moveTo(0, 0);
      g.lineTo(barrel.x, barrel.y);

      g.lineStyle(2, 0x1a1712, 0.95);
      g.beginFill(lightenColor(tint, 0.12));
      drawRotatedRect(g, 1, 0, body.halfLen * 0.72, body.halfWidth * 0.9, weaponFacing);
      g.endFill();

      const nose = polar(facing, body.halfLen - 2);
      g.lineStyle(2, 0xd8d0b0, 0.75);
      g.moveTo(nose.x - Math.cos(facing) * 5, nose.y - Math.sin(facing) * 5);
      g.lineTo(nose.x, nose.y);
      drawTankFuelCue(g, body, facing, motion);
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
      // Latched miners get a small clamp marker above the unit.
      if (e.latchedNode) {
        g.lineStyle(2, 0xf2d16b, 0.95);
        g.moveTo(-r * 0.55, -r * 1.15);
        g.lineTo(-r * 0.2, -r * 1.45);
        g.lineTo(r * 0.2, -r * 1.45);
        g.lineTo(r * 0.55, -r * 1.15);
      }
    }

    // Facing indicator: a short pale tick from center outward.
    if (
      e.kind !== KIND.RIFLEMAN &&
      e.kind !== KIND.MACHINE_GUNNER &&
      e.kind !== KIND.AT_TEAM &&
      e.kind !== KIND.SCOUT_CAR &&
      e.kind !== KIND.TANK
    ) {
      const fp = polar(facing, r + 3);
      g.lineStyle(2, 0xd8d0b0, 0.85);
      g.moveTo(0, 0);
      g.lineTo(fp.x, fp.y);
    }
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
    if (e.kind === KIND.CITY_CENTRE) {
      g.drawRect(x0 + w * 0.12, y0 + h * 0.18, w * 0.62, h * 0.52);
      g.drawRect(x0 + w * 0.68, y0 + h * 0.1, w * 0.16, h * 0.32);
      g.beginFill(0x1a1712, bodyAlpha * 0.7);
      g.drawRect(x0 + w * 0.76, y0 + h * 0.02, w * 0.08, h * 0.22);
    } else if (e.kind === KIND.FACTORY) {
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

    // Queue depth label: show items waiting behind the active production slot.
    const queueDepth = (e.prodQueue ?? 0) - 1;
    this._queueLabel(e, e.x, y0 + 14, queueDepth, bodyAlpha);
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
    const full = RESOURCE_AMOUNTS[e.kind] || 1;
    const frac = e.remaining == null ? 1 : clamp01(e.remaining / full);
    const r = base * (0.55 + 0.45 * frac);

    const ts = (this._map && this._map.tileSize) || 32;
    const visible = !fog || fog.isVisible(Math.floor(e.x / ts), Math.floor(e.y / ts));
    const alpha = visible ? 1 : 0.7;

    const g = this._slot("resources", e.id);
    g.position.set(e.x, e.y);
    g.alpha = alpha;

    if (e.kind === KIND.OIL) {
      // Fuel drums: utilitarian but faction-neutral.
      // White outline improves contrast against dark ground and fog.
      g.lineStyle(2.5, 0xffffff, 0.95);
      g.drawRect(-r * 0.78, -r * 0.58, r * 0.52, r * 1.09);
      g.drawRect(-r * 0.21, -r * 0.71, r * 0.54, r * 1.23);
      g.drawRect(r * 0.35, -r * 0.53, r * 0.46, r * 1.06);

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

    // X marker over a node that a worker is actively mining.
    if (this._miningNodes && this._miningNodes.has(e.id)) {
      const xr = r * 0.45;
      const xColor = e.kind === KIND.OIL ? 0xffffff : 0x1a1712;
      g.lineStyle(2.5, xColor, 0.95);
      g.moveTo(-xr, -xr);
      g.lineTo(xr, xr);
      g.moveTo(xr, -xr);
      g.lineTo(-xr, xr);
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
    if (isVehicleBodyKind(e.kind)) {
      const body = tankBodyVisual(stat);
      return { rx: body.halfLen + 4, ry: body.halfWidth + 5, cy: 2 };
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
      if (isVehicleBodyKind(e.kind)) {
        const body = tankBodyVisual(stat);
        halfW = body.halfLen * 0.8;
        topY = e.y - body.shadowRadius - 8;
      } else {
        const r = stat.size || 9;
        halfW = Math.max(10, r);
        topY = e.y - r - 8;
      }
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

  /**
   * Show/hide the queued-unit count label for a building. Pooled by entity id.
   * @private
   */
  _queueLabel(e, cx, cy, count, bodyAlpha) {
    if (!this._queueLabelPool) this._queueLabelPool = new Map();
    let t = this._queueLabelPool.get(e.id);
    if (!t) {
      t = new PIXI.Text("", {
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
        fontSize: 11,
        fill: 0xffe080,
        align: "center",
        fontWeight: "700",
        stroke: 0x000000,
        strokeThickness: 3,
      });
      t.anchor.set(0.5, 0);
      this._queueLabelPool.set(e.id, t);
      this.layers.buildings.addChild(t);
    }
    if (count > 0) {
      const label = `+${count}`;
      if (t.text !== label) t.text = label;
      t.visible = true;
      t.alpha = bodyAlpha;
      t.position.set(cx, cy);
    } else {
      t.visible = false;
    }
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
      // Run-length merge contiguous tiles sharing a fog level (0=clear,1=dim,2=dark,3=impassable-dim).
      let runStart = 0;
      let runLevel = this._fogLevel(fog, 0, ty);
      for (let tx = 1; tx <= w; tx++) {
        const level = tx < w ? this._fogLevel(fog, tx, ty) : -1;
        if (level !== runLevel) {
          if (runLevel > 0) {
            const color = runLevel === 2 ? COLORS.fogUnexplored : COLORS.fogExplored;
            const a = runLevel === 2
              ? FOG_UNEXPLORED_ALPHA
              : runLevel === 3
                ? FOG_UNEXPLORED_ALPHA * 0.56
                : FOG_EXPLORED_ALPHA;
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
   * @returns {0|1|2|3} 0 visible, 1 explored (dim), 2 unexplored (dark), 3 unexplored impassable (light dim)
   */
  _fogLevel(fog, tx, ty) {
    if (fog.isVisible(tx, ty)) return 0;
    if (this._map && isImpassableAt(this._map, tx, ty)) {
      return fog.isExplored(tx, ty) ? 0 : 3;
    }
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
   * Draw rally-point markers for selected own unit-producing buildings: a faint line from the
   * building to the rally point and a small flag at the point. Appends to the feedback graphics,
   * which `_drawCommandFeedback` clears earlier this frame.
   * @private
   */
  _drawRallyPoints(state) {
    if (!state || typeof state.selectedEntities !== "function") return;
    const g = this._feedbackGfx;
    const color = COLORS.selectOwn;
    for (const e of state.selectedEntities()) {
      if (e.owner !== state.playerId) continue;
      if (!isBuilding(e.kind) || !isProducerBuilding(e.kind)) continue;
      const rally = e.rally;
      if (!rally) continue;
      const [rx, ry] = rally;

      // Link from the building to the rally point.
      g.lineStyle(2, color, 0.5);
      g.moveTo(e.x, e.y);
      g.lineTo(rx, ry);

      // Flag: pole + pennant + base dot.
      g.lineStyle(2.5, color, 0.95);
      g.moveTo(rx, ry);
      g.lineTo(rx, ry - 20);
      g.beginFill(color, 0.9);
      g.drawPolygon([rx, ry - 20, rx + 13, ry - 16, rx, ry - 11]);
      g.endFill();
      g.lineStyle(0);
      g.beginFill(color, 0.85);
      g.drawCircle(rx, ry, 3);
      g.endFill();
    }
  }

  /** Draw the resource hover's nearest City Centre mining link. @private */
  _drawResourceMiningPreview(state) {
    if (!state || !state.resourceMiningPreview) return;
    const g = this._feedbackGfx;
    const p = state.resourceMiningPreview;
    const ccStat = STATS[KIND.CITY_CENTRE] || {};
    const ts = (this._map && this._map.tileSize) || 32;
    const ccEndpoint = rectEdgePointTowardCenter(
      p.resourceX,
      p.resourceY,
      p.ccX,
      p.ccY,
      ((ccStat.footW || 3) * ts) / 2,
      ((ccStat.footH || 3) * ts) / 2,
    );

    if (p.inRange) {
      g.lineStyle(4, 0x4aa3ff, 0.95);
      g.beginFill(0x4aa3ff, 0.18);
      g.drawCircle(p.resourceX, p.resourceY, 9);
      g.endFill();
      return;
    }

    g.lineStyle(2.5, 0xd64d45, 0.9);
    dashedLine(g, p.resourceX, p.resourceY, ccEndpoint.x, ccEndpoint.y, 14, 9);
  }

  /**
   * Draw a brief muzzle flash on the attacker plus a yellow tracer line to the
   * target, then a fainter continuation past the target for overpenetration.
   * Size scales by attacker kind (tank > MG > rifleman).
   * @private
   */
  _drawMuzzleFlashes(state) {
    const g = this._feedbackGfx;
    if (!state || typeof state.liveMuzzleFlashes !== "function") return;
    const now = performance.now();
    const flashes = state.liveMuzzleFlashes(now);
    if (!flashes.length) return;

    for (const f of flashes) {
      const attacker = state.entityById(f.from);
      if (!attacker) continue;
      const target = state.entityById(f.to);

      const age = now - f.createdAt;
      const t = clamp01(age / 240);
      const fade = 1 - t;

      const baseR = muzzleFlashRadius(attacker.kind);
      if (baseR <= 0) continue;

      const facing = isVehicleBodyKind(attacker.kind) && typeof attacker.weaponFacing === "number"
        ? attacker.weaponFacing
        : typeof attacker.facing === "number"
        ? attacker.facing
        : target
        ? Math.atan2(target.y - attacker.y, target.x - attacker.x)
        : 0;
      const stat = STATS[attacker.kind] || {};
      const reach = isBuilding(attacker.kind)
        ? Math.max(stat.footW || 2, stat.footH || 2) * ((this._map && this._map.tileSize) || 32) * 0.5
        : (stat.size || 9) * 1.1;
      const mx = attacker.x + Math.cos(facing) * reach;
      const my = attacker.y + Math.sin(facing) * reach;

      if (target) {
        const dx = target.x - mx;
        const dy = target.y - my;
        const shotLen = Math.hypot(dx, dy);
        // Mirror the server overpenetration band: a round that hits a tank stops dead (no tail),
        // and AT teams punch twice as deep as everyone else.
        const tileSize = (this._map && this._map.tileSize) || 32;
        const penFactor = target.kind === KIND.TANK ? 0 : attacker.kind === KIND.AT_TEAM ? 0.5 : 0.25;
        const tailLen = (stat.rangeTiles || 0) * tileSize * penFactor;

        g.lineStyle(1.5, 0xffe066, 0.9 * fade);
        g.moveTo(mx, my);
        g.lineTo(target.x, target.y);

        if (shotLen > 0.001 && tailLen > 0) {
          const ux = dx / shotLen;
          const uy = dy / shotLen;
          const ex = target.x + ux * tailLen;
          const ey = target.y + uy * tailLen;
          g.lineStyle(1.0, 0xffd84a, 0.42 * fade);
          g.moveTo(target.x, target.y);
          g.lineTo(ex, ey);
        }
      }

      // Flash: bright core that scales up slightly then fades.
      const r = baseR * (0.7 + 0.45 * t);
      g.lineStyle(0);
      g.beginFill(0xfff2a8, 0.85 * fade);
      g.drawCircle(mx, my, r);
      g.endFill();
      g.beginFill(0xffd84a, 0.55 * fade);
      g.drawCircle(mx, my, r * 0.55);
      g.endFill();
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
    if (this._queueLabelPool) for (const id of this._queueLabelPool.keys()) ids.add(id);
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
    if (this._queueLabelPool) {
      const seen = this._seen.buildings;
      for (const [id, t] of this._queueLabelPool) {
        if (seen.has(id)) continue;
        if (evict.has(id)) {
          this.layers.buildings.removeChild(t);
          t.destroy();
          this._queueLabelPool.delete(id);
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
    if (this._queueLabelPool) {
      for (const t of this._queueLabelPool.values()) t.destroy();
      this._queueLabelPool.clear();
    }
    this._unseen.clear();
    this._setupVisuals.clear();
    this._tankMotion.clear();

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
    this._setupVisuals.clear();

    // Detach the canvas from the DOM, then destroy the app + WebGL context.
    const view = this.app.view;
    if (view && view.parentNode) view.parentNode.removeChild(view);
    this.app.destroy(true, { children: true, texture: true, baseTexture: true });
  }
}

// --- Small pure helpers ----------------------------------------------------

/** Per-attacker muzzle-flash radius in world px. 0 means no flash for this kind. */
function muzzleFlashRadius(kind) {
  if (kind === KIND.TANK) return 18;
  if (kind === KIND.AT_TEAM) return 11;
  if (kind === KIND.SCOUT_CAR) return 9;
  if (kind === KIND.MACHINE_GUNNER) return 9;
  if (kind === KIND.RIFLEMAN) return 7;
  return 0;
}

function weaponRecoilOffset(kind, progress) {
  return (WEAPON_RECOIL_PX[kind] || 0) * clamp01(progress);
}

/** Clamp a number to [0,1]. */
function clamp01(v) {
  if (v == null || Number.isNaN(v)) return 0;
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

/** Smoothstep easing on [0,1]. */
function smoothstep01(v) {
  const t = clamp01(v);
  return t * t * (3 - 2 * t);
}

function lerp(a, b, t) {
  return a + (b - a) * t;
}

function dashedLine(g, x1, y1, x2, y2, dash, gap) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const len = Math.hypot(dx, dy);
  if (len <= 0.001) return;
  const ux = dx / len;
  const uy = dy / len;
  let cursor = 0;
  while (cursor < len) {
    const end = Math.min(cursor + dash, len);
    g.moveTo(x1 + ux * cursor, y1 + uy * cursor);
    g.lineTo(x1 + ux * end, y1 + uy * end);
    cursor = end + gap;
  }
}

function rectEdgePointTowardCenter(fromX, fromY, centerX, centerY, halfW, halfH) {
  const dx = centerX - fromX;
  const dy = centerY - fromY;
  if (Math.hypot(dx, dy) <= 0.001) return { x: centerX, y: centerY };

  const minX = centerX - halfW;
  const maxX = centerX + halfW;
  const minY = centerY - halfH;
  const maxY = centerY + halfH;
  let tEnter = 0;
  let tExit = 1;

  if (Math.abs(dx) > 0.001) {
    const tx1 = (minX - fromX) / dx;
    const tx2 = (maxX - fromX) / dx;
    tEnter = Math.max(tEnter, Math.min(tx1, tx2));
    tExit = Math.min(tExit, Math.max(tx1, tx2));
  } else if (fromX < minX || fromX > maxX) {
    return { x: centerX, y: centerY };
  }

  if (Math.abs(dy) > 0.001) {
    const ty1 = (minY - fromY) / dy;
    const ty2 = (maxY - fromY) / dy;
    tEnter = Math.max(tEnter, Math.min(ty1, ty2));
    tExit = Math.min(tExit, Math.max(ty1, ty2));
  } else if (fromY < minY || fromY > maxY) {
    return { x: centerX, y: centerY };
  }

  if (tEnter <= 0.001 || tEnter > tExit || tEnter > 1) return { x: centerX, y: centerY };
  return { x: fromX + dx * tEnter, y: fromY + dy * tEnter };
}

/** Point at angle `a` (radians) and distance `d` from the origin. */
function polar(a, d) {
  return { x: Math.cos(a) * d, y: Math.sin(a) * d };
}

function recoilVector(a, d) {
  return d > 0 ? polar(a + Math.PI, d) : ZERO_OFFSET;
}

function offsetPoint(p, offset) {
  return { x: p.x + offset.x, y: p.y + offset.y };
}

function rotatePoint(x, y, a) {
  const c = Math.cos(a);
  const s = Math.sin(a);
  return { x: x * c - y * s, y: x * s + y * c };
}

function rotatedPolygon(points, a) {
  const out = [];
  for (let i = 0; i < points.length; i += 2) {
    const p = rotatePoint(points[i], points[i + 1], a);
    out.push(p.x, p.y);
  }
  return out;
}

function drawRotatedRect(g, cx, cy, w, h, a) {
  drawRotatedRectOffset(g, cx, cy, w, h, a, ZERO_OFFSET);
}

function drawFreeRotatedRect(g, cx, cy, w, h, a) {
  const hw = w / 2;
  const hh = h / 2;
  const corners = [
    [-hw, -hh],
    [hw, -hh],
    [hw, hh],
    [-hw, hh],
  ];
  const polygon = [];
  for (const [x, y] of corners) {
    const p = rotatePoint(x, y, a);
    polygon.push(cx + p.x, cy + p.y);
  }
  g.drawPolygon(polygon);
}

function drawRotatedRectOffset(g, cx, cy, w, h, a, offset) {
  const hw = w / 2;
  const hh = h / 2;
  const corners = [
    [cx - hw, cy - hh],
    [cx + hw, cy - hh],
    [cx + hw, cy + hh],
    [cx - hw, cy + hh],
  ];
  const polygon = [];
  for (const [x, y] of corners) {
    const p = rotatePoint(x, y, a);
    polygon.push(p.x + offset.x, p.y + offset.y);
  }
  g.drawPolygon(polygon);
}

function tankBodyVisual(stat = {}) {
  const body = stat.body || TANK_BODY;
  const halfLen = body.length * 0.5;
  const halfWidth = body.width * 0.5;
  const clearance = body.clearance || 0;
  return {
    halfLen,
    halfWidth,
    clearance,
    shadowRadius: Math.hypot(halfLen + clearance, halfWidth + clearance),
  };
}

function isVehicleBodyKind(kind) {
  return kind === KIND.TANK || kind === KIND.SCOUT_CAR;
}

function drawTankTracks(g, body, facing, motion) {
  const trackW = 5;
  const trackY = body.halfWidth - trackW * 0.5;
  const trackLen = body.halfLen * 2;
  g.lineStyle(1.5, 0x100d0a, 0.95);
  g.beginFill(0x15120f, 0.96);
  drawRotatedRect(g, 0, -trackY, trackLen, trackW, facing);
  drawRotatedRect(g, 0, trackY, trackLen, trackW, facing);
  g.endFill();

  drawTrackTreads(g, body, facing, -trackY, motion.leftPhase, motion.leftDir, motion.activity);
  drawTrackTreads(g, body, facing, trackY, motion.rightPhase, motion.rightDir, motion.activity);
}

function drawTrackTreads(g, body, facing, y, phase, dir, activity) {
  const spacing = 6;
  const treadW = 2.4;
  const treadH = 4.4;
  const alpha = lerp(0.35, 0.82, clamp01(activity));
  const offset = positiveMod(phase * 0.85, spacing);
  g.beginFill(dir < 0 ? 0x8f7f5e : 0xd8d0b0, alpha);
  for (let x = -body.halfLen - spacing; x <= body.halfLen + spacing; x += spacing) {
    const treadX = x + offset;
    if (treadX < -body.halfLen || treadX > body.halfLen) continue;
    drawRotatedRect(g, treadX, y, treadW, treadH, facing);
  }
  g.endFill();
}

function drawTankHull(g, body, tint, facing) {
  const inset = 2;
  g.beginFill(tint);
  g.drawPolygon(rotatedPolygon([
    -body.halfLen + inset, -body.halfWidth + 3,
    body.halfLen - 6, -body.halfWidth + 3,
    body.halfLen, -body.halfWidth + 7,
    body.halfLen, body.halfWidth - 7,
    body.halfLen - 6, body.halfWidth - 3,
    -body.halfLen + inset, body.halfWidth - 3,
    -body.halfLen, body.halfWidth - 7,
    -body.halfLen, -body.halfWidth + 7,
  ], facing));
  g.endFill();

  g.beginFill(0x1a1712, 0.24);
  drawRotatedRect(g, -2, 0, body.halfLen * 1.15, body.halfWidth * 0.82, facing);
  g.endFill();

  g.beginFill(lightenColor(tint, 0.06), 0.95);
  drawRotatedRect(g, body.halfLen - 7, 0, 7, body.halfWidth * 1.35, facing);
  g.endFill();

  g.beginFill(0x1a1712, 0.22);
  drawRotatedRect(g, body.halfLen - 3, 0, 3, body.halfWidth * 1.2, facing);
  g.endFill();
}

function drawTankFuelCue(g, body, facing, motion) {
  if (!motion.lowOil && !motion.oilStarved) return;
  const x = -body.halfLen + 6;
  const y = -body.halfWidth - 4;
  const color = motion.oilStarved ? 0xd47a5f : 0xc9b56a;
  g.lineStyle(2, color, motion.oilStarved ? 0.95 : 0.75);
  drawRotatedRectOutline(g, x, y, 8, 5, facing);
  if (motion.oilStarved) {
    const a = facing;
    const p1 = rotatePoint(x - 3, y - 1.5, a);
    const p2 = rotatePoint(x + 3, y + 1.5, a);
    const p3 = rotatePoint(x + 3, y - 1.5, a);
    const p4 = rotatePoint(x - 3, y + 1.5, a);
    g.moveTo(p1.x, p1.y);
    g.lineTo(p2.x, p2.y);
    g.moveTo(p3.x, p3.y);
    g.lineTo(p4.x, p4.y);
  }
}

function drawScoutCar(g, body, tint, facing, weaponFacing, motion, recoil) {
  const sideAlpha = lerp(0.62, 0.88, motion.activity);

  // Single blocky truck hull with enclosed side running gear; nothing protrudes past the body.
  g.beginFill(tint);
  drawRotatedRect(g, 0, 0, body.halfLen * 2, body.halfWidth * 2, facing);
  g.endFill();

  g.beginFill(0x15120f, sideAlpha);
  drawRotatedRect(g, -body.halfLen * 0.02, -body.halfWidth * 0.78, body.halfLen * 1.72, body.halfWidth * 0.22, facing);
  drawRotatedRect(g, -body.halfLen * 0.02, body.halfWidth * 0.78, body.halfLen * 1.72, body.halfWidth * 0.22, facing);
  g.endFill();

  g.beginFill(lightenColor(tint, 0.08), 0.96);
  drawRotatedRect(g, -body.halfLen * 0.32, 0, body.halfLen * 0.96, body.halfWidth * 1.44, facing);
  g.endFill();

  g.beginFill(lightenColor(tint, 0.14), 0.95);
  drawRotatedRect(g, body.halfLen * 0.36, 0, body.halfLen * 0.58, body.halfWidth * 1.42, facing);
  g.endFill();

  g.beginFill(0x211b14, 0.82);
  drawRotatedRect(g, body.halfLen * 0.68, 0, body.halfLen * 0.22, body.halfWidth * 1.2, facing);
  drawRotatedRect(g, body.halfLen * 0.24, -body.halfWidth * 0.36, body.halfLen * 0.18, body.halfWidth * 0.34, facing);
  drawRotatedRect(g, body.halfLen * 0.24, body.halfWidth * 0.36, body.halfLen * 0.18, body.halfWidth * 0.34, facing);
  g.endFill();

  g.lineStyle(2, 0xd8d0b0, 0.6);
  const hoodA = rotatePoint(body.halfLen * 0.48, -body.halfWidth * 0.45, facing);
  const hoodB = rotatePoint(body.halfLen * 0.48, body.halfWidth * 0.45, facing);
  g.moveTo(hoodA.x, hoodA.y);
  g.lineTo(hoodB.x, hoodB.y);

  const gunner = rotatePoint(-body.halfLen * 0.42, 0, facing);
  const mount = rotatePoint(-body.halfLen * 0.32, 0, facing);
  g.beginFill(0x1a1712, 0.9);
  g.drawCircle(mount.x, mount.y, body.halfWidth * 0.32);
  g.endFill();

  const a = weaponFacing;
  const gunnerTorso = offsetPoint(gunner, {
    x: Math.cos(a + Math.PI) * body.halfWidth * 0.1,
    y: Math.sin(a + Math.PI) * body.halfWidth * 0.1,
  });
  g.beginFill(lightenColor(tint, 0.14), 0.98);
  drawFreeRotatedRect(g, gunnerTorso.x, gunnerTorso.y, body.halfWidth * 0.5, body.halfWidth * 0.64, a);
  g.endFill();

  const gunnerHead = {
    x: gunner.x + Math.cos(a) * body.halfWidth * 0.2,
    y: gunner.y + Math.sin(a) * body.halfWidth * 0.2,
  };
  g.beginFill(lightenColor(tint, 0.24), 0.98);
  g.drawCircle(gunnerHead.x, gunnerHead.y, body.halfWidth * 0.18);
  g.endFill();

  const kick = recoilVector(a, recoil);
  const handSpan = body.halfWidth * 0.32;
  const grip = offsetPoint({
    x: gunner.x + Math.cos(a) * body.halfWidth * 0.2,
    y: gunner.y + Math.sin(a) * body.halfWidth * 0.2,
  }, kick);
  g.lineStyle(2, 0xd8d0b0, 0.86);
  g.moveTo(gunner.x - Math.sin(a) * handSpan, gunner.y + Math.cos(a) * handSpan);
  g.lineTo(grip.x, grip.y);
  g.moveTo(gunner.x + Math.sin(a) * handSpan, gunner.y - Math.cos(a) * handSpan);
  g.lineTo(grip.x, grip.y);

  const stock = offsetPoint({
    x: gunner.x + Math.cos(a + Math.PI) * body.halfWidth * 0.34,
    y: gunner.y + Math.sin(a + Math.PI) * body.halfWidth * 0.34,
  }, kick);
  const muzzle = offsetPoint({
    x: gunner.x + Math.cos(a) * (body.halfLen * 0.78),
    y: gunner.y + Math.sin(a) * (body.halfLen * 0.78),
  }, kick);
  g.lineStyle(3, 0x17130f, 0.98);
  g.moveTo(stock.x, stock.y);
  g.lineTo(muzzle.x, muzzle.y);
  g.beginFill(0x32291f, 0.98);
  const receiver = offsetPoint({
    x: gunner.x + Math.cos(a) * body.halfWidth * 0.42,
    y: gunner.y + Math.sin(a) * body.halfWidth * 0.42,
  }, kick);
  drawFreeRotatedRect(g, receiver.x, receiver.y, body.halfWidth * 0.58, body.halfWidth * 0.3, a);
  g.endFill();

  const shroud = offsetPoint({
    x: gunner.x + Math.cos(a) * body.halfWidth * 0.9,
    y: gunner.y + Math.sin(a) * body.halfWidth * 0.9,
  }, kick);
  g.beginFill(0x241d17, 0.98);
  drawFreeRotatedRect(g, shroud.x, shroud.y, body.halfWidth * 0.82, body.halfWidth * 0.18, a);
  g.endFill();

  const nose = polar(facing, body.halfLen - 2);
  g.lineStyle(2, 0xd8d0b0, 0.72);
  g.moveTo(nose.x - Math.cos(facing) * 4, nose.y - Math.sin(facing) * 4);
  g.lineTo(nose.x, nose.y);
}

function drawRotatedRectOutline(g, cx, cy, w, h, a) {
  const hw = w / 2;
  const hh = h / 2;
  const corners = [
    rotatePoint(cx - hw, cy - hh, a),
    rotatePoint(cx + hw, cy - hh, a),
    rotatePoint(cx + hw, cy + hh, a),
    rotatePoint(cx - hw, cy + hh, a),
  ];
  g.moveTo(corners[0].x, corners[0].y);
  for (let i = 1; i < corners.length; i += 1) g.lineTo(corners[i].x, corners[i].y);
  g.lineTo(corners[0].x, corners[0].y);
}

function drawInfantryBase(g, r, tint, facing) {
  // Shared combat-infantry body: same soldier, different oversized weapon.
  g.lineStyle(2, 0x1a1712, 0.95);
  g.beginFill(tint);
  g.drawPolygon(rotatedPolygon([
    r * 0.72, 0,
    r * 0.22, -r * 0.62,
    -r * 0.58, -r * 0.48,
    -r * 0.78, 0,
    -r * 0.58, r * 0.48,
    r * 0.22, r * 0.62,
  ], facing));
  g.endFill();

  const head = polar(facing, r * 0.72);
  g.beginFill(lightenColor(tint, 0.16));
  g.drawCircle(head.x, head.y, r * 0.34);
  g.endFill();

  const rear = polar(facing + Math.PI, r * 0.68);
  const shoulderL = polar(facing - 1.42, r * 0.48);
  const shoulderR = polar(facing + 1.42, r * 0.48);
  g.lineStyle(2, 0x1a1712, 0.5);
  g.moveTo(rear.x, rear.y);
  g.lineTo(shoulderL.x, shoulderL.y);
  g.moveTo(rear.x, rear.y);
  g.lineTo(shoulderR.x, shoulderR.y);
}

function drawInfantryRifle(g, r, facing, recoil) {
  const a = facing - 0.2;
  const kick = recoilVector(a, recoil);
  const stock = offsetPoint(polar(a + Math.PI, r * 0.18), kick);
  const muzzle = offsetPoint(polar(a, r * 1.82), kick);
  const hand = offsetPoint(polar(a, r * 0.55), kick);

  g.lineStyle(3, 0x2a2119, 0.96);
  g.moveTo(stock.x, stock.y);
  g.lineTo(muzzle.x, muzzle.y);
  g.lineStyle(2, 0xd8d0b0, 0.85);
  g.moveTo(hand.x - Math.sin(a) * r * 0.32, hand.y + Math.cos(a) * r * 0.32);
  g.lineTo(hand.x + Math.sin(a) * r * 0.32, hand.y - Math.cos(a) * r * 0.32);
}

function drawInfantryPanzerfaust(g, r, facing, recoil) {
  const a = facing - 0.12;
  const kick = recoilVector(a, recoil);
  const rearDist = r * 0.42;
  const muzzleDist = r * 2.05;
  const warheadDist = r * 1.55;
  const rear = offsetPoint(polar(a + Math.PI, rearDist), kick);
  const muzzle = offsetPoint(polar(a, muzzleDist), kick);
  const warhead = polar(a, warheadDist);
  const rearBlock = polar(a + Math.PI, rearDist);

  g.lineStyle(5, 0x2a2119, 0.98);
  g.moveTo(rear.x, rear.y);
  g.lineTo(muzzle.x, muzzle.y);

  g.beginFill(0x3d3528, 0.98);
  drawRotatedRectOffset(g, warhead.x, warhead.y, r * 0.52, r * 0.62, a, kick);
  g.endFill();
  g.beginFill(0xd8d0b0, 0.88);
  drawRotatedRectOffset(g, rearBlock.x, rearBlock.y, r * 0.34, r * 0.48, a, kick);
  g.endFill();
}

function drawInfantryMachineGun(g, r, facing, weaponFacing, setup, recoil) {
  const deploy = clamp01(setup.prongFactor);
  const carryA = facing + 0.86;
  const aimA = weaponFacing;
  const a = angleLerp(carryA, aimA, smoothstep01(deploy));
  const kick = recoilVector(a, recoil);
  const stockRearDist = lerp(r * 0.76, r * 0.58, deploy);
  const muzzleDist = lerp(r * 1.36, r * 2.46, deploy);
  const stockRear = offsetPoint(polar(a + Math.PI, stockRearDist), kick);
  const muzzle = offsetPoint(polar(a, muzzleDist), kick);

  // MG42-inspired profile: shoulder stock, box receiver, long perforated shroud, no rotary barrels.
  g.lineStyle(3, 0x17130f, 0.98);
  g.moveTo(stockRear.x, stockRear.y);
  g.lineTo(muzzle.x, muzzle.y);

  const stockCenterX = lerp(-r * 0.42, -r * 0.28, deploy);
  g.beginFill(0x4a3420, 0.96);
  drawRotatedRectOffset(g, stockCenterX, 0, r * 0.62, r * 0.38, a, kick);
  g.endFill();

  const receiverX = lerp(r * 0.04, r * 0.2, deploy);
  g.beginFill(0x32291f, 0.98);
  drawRotatedRectOffset(g, receiverX, 0, r * 0.72, r * 0.48, a, kick);
  g.endFill();

  g.beginFill(0xd8d0b0, 0.82);
  drawRotatedRectOffset(g, receiverX + r * 0.08, -r * 0.2, r * 0.56, r * 0.12, a, kick);
  g.endFill();

  const shroudX = lerp(r * 0.62, r * 1.08, deploy);
  const shroudW = lerp(r * 0.72, r * 1.22, deploy);
  g.beginFill(0x241d17, 0.98);
  drawRotatedRectOffset(g, shroudX, 0, shroudW, r * 0.24, a, kick);
  g.endFill();

  g.beginFill(0xd8d0b0, 0.72);
  const slotCount = deploy > 0.55 ? 4 : 3;
  for (let i = 0; i < slotCount; i += 1) {
    const t = slotCount === 1 ? 0.5 : i / (slotCount - 1);
    drawRotatedRectOffset(
      g,
      shroudX - shroudW * 0.3 + shroudW * 0.6 * t,
      0,
      r * 0.09,
      r * 0.14,
      a,
      kick,
    );
  }
  g.endFill();

  const muzzleBase = offsetPoint(polar(a, muzzleDist - r * 0.18), kick);
  g.lineStyle(2, 0xd8d0b0, 0.78);
  g.moveTo(muzzleBase.x - Math.sin(a) * r * 0.22, muzzleBase.y + Math.cos(a) * r * 0.22);
  g.lineTo(muzzleBase.x + Math.sin(a) * r * 0.22, muzzleBase.y - Math.cos(a) * r * 0.22);

  const grip = offsetPoint(polar(a + Math.PI, r * 0.02), kick);
  g.lineStyle(3, 0xd8d0b0, 0.86);
  g.moveTo(grip.x - Math.sin(a) * r * 0.34, grip.y + Math.cos(a) * r * 0.34);
  g.lineTo(grip.x + Math.sin(a) * r * 0.34, grip.y - Math.cos(a) * r * 0.34);

  if (deploy > 0.02) {
    const bipodRoot = offsetPoint(polar(a, lerp(r * 0.9, r * 1.72, deploy)), kick);
    const legLen = r * lerp(0.38, 1.0, deploy);
    const spread = lerp(0.32, 0.72, deploy);
    const left = {
      x: bipodRoot.x + Math.cos(a + spread) * legLen,
      y: bipodRoot.y + Math.sin(a + spread) * legLen,
    };
    const right = {
      x: bipodRoot.x + Math.cos(a - spread) * legLen,
      y: bipodRoot.y + Math.sin(a - spread) * legLen,
    };
    g.lineStyle(3, 0xd8d0b0, 0.9);
    g.moveTo(bipodRoot.x, bipodRoot.y);
    g.lineTo(left.x, left.y);
    g.moveTo(bipodRoot.x, bipodRoot.y);
    g.lineTo(right.x, right.y);
  }

  if (setup.barrel || deploy > 0.75) {
    g.beginFill(0x241d17, 0.96);
    drawRotatedRectOffset(g, muzzleDist, 0, r * 0.22, r * 0.16, a, kick);
    g.endFill();
  }
}

function angleLerp(a, b, t) {
  let d = (b - a) % (Math.PI * 2);
  if (d > Math.PI) d -= Math.PI * 2;
  if (d < -Math.PI) d += Math.PI * 2;
  return a + d * clamp01(t);
}

function angleDelta(from, to) {
  let d = (to - from) % (Math.PI * 2);
  if (d > Math.PI) d -= Math.PI * 2;
  if (d < -Math.PI) d += Math.PI * 2;
  return d;
}

function positiveMod(value, modulus) {
  return ((value % modulus) + modulus) % modulus;
}

function lightenColor(color, amount) {
  const r = Math.min(255, ((color >> 16) & 0xff) + Math.round(255 * amount));
  const g = Math.min(255, ((color >> 8) & 0xff) + Math.round(255 * amount));
  const b = Math.min(255, (color & 0xff) + Math.round(255 * amount));
  return (r << 16) | (g << 8) | b;
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
  if (code === 3) return n > 0.74 ? 0x4a5a3e : 0x2a3a1e;
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
  return code === 1 || code === 2 || code === 3;
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
