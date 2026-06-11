// Renderer — PixiJS scene graph + per-frame drawing. See docs/design/client-ui.md §4.1 / §4.2.
//
// Owns a single PIXI.Application whose stage holds one `world` container that is
// positioned/scaled from the Camera each frame, plus a screen-space overlay layer
// for the drag selection box. Layers are drawn back-to-front in this order:
//
//   terrain → resources → building-shadows → buildings → unit-shadows → units
//   → selection-rings → hp-bars → fog → shot-reveals → feedback → placement-ghost → drag-box
//
// Terrain is drawn once into a cached RenderTexture (it never changes mid-match).
// Everything else is redrawn each frame, but per-entity Graphics are pooled and
// reconciled by entity id so we never churn the scene graph: each frame we touch
// the live ids, then hide any pooled object whose id was not seen.
//
// PixiJS v7 is loaded globally as `PIXI`; we never import it.

import { COLORS } from "../config.js";
import { isUnit, isBuilding, isResource } from "../protocol.js";
import { _drawBuilding } from "./buildings.js";
import {
  _drawSelectionAndHp,
  _hpBar,
  _icon,
  _ownerColors,
  _queueLabel,
  _ringRadius,
  _shadow,
  _slot,
  _tintFor,
  _vehicleShadow,
} from "./entities.js";
import {
  _drawAbilityTargetPreview,
  _drawArtilleryImpacts,
  _drawArtilleryLaunches,
  _drawArtilleryTargets,
  _drawAtGunSetupPreview,
  _drawCommandFeedback,
  _drawDebugPathOverlay,
  _drawMuzzleFlashes,
  _drawMortarImpacts,
  _drawMortarLaunches,
  _drawSelectedMortarRanges,
  _drawMortarShells,
  _drawMortarTargets,
  _drawOrderPlan,
  _drawPlacement,
  _drawRallyPoints,
  _drawResourceMiningPreview,
  _drawSmokeCanisters,
  _drawSmokes,
  drawSelectionBox,
} from "./feedback.js";
import { _drawFog, _fogLevel } from "./fog.js";
import { LAYERS, _sweep } from "./layers.js";
import { _drawResource } from "./resources.js";
import { buildStaticMap } from "./terrain.js";
import {
  _deployedWeaponSetupVisual,
  _drawShotRevealUnit,
  _drawUnit,
  _sweepSetupVisuals,
  _sweepTankMotion,
  _tankMotionVisual,
} from "./units.js";

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
    this._smokeGfx = new PIXI.Graphics();
    this.layers.smokes.addChild(this._smokeGfx);
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
      shotRevealShadows: new Map(),
      shotReveals: new Map(),
    };
    // Ids touched this frame, per pool, so we can hide stale entries afterwards.
    this._seen = {};
    for (const key of Object.keys(this._pools)) this._seen[key] = new Set();
    // Consecutive-frames-unseen counter per id (across all pools + icons), so we
    // hide briefly but evict after a grace period — server ids are never reused.
    this._unseen = new Map();
    // Local animation state for deployed-weapon setup / teardown visuals.
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
    const regularEntities = entities.filter((e) => !e.shotReveal);
    const shotReveals = entities.filter((e) => e.shotReveal);
    const selection = state.selection || new Set();
    const colorByOwner = this._ownerColors(state);
    const liveIds = new Set();

    // Nodes currently being mined: any worker latched to them. Used by
    // _drawResource to overlay an X marker.
    this._miningNodes = new Set();
    for (const e of regularEntities) {
      if (e.latchedNode) this._miningNodes.add(e.latchedNode);
    }

    // Two passes so silhouettes layer correctly: resources + buildings first
    // (footprints sit under units), then units. Selection rings / hp bars are
    // their own layers and are filled inline.
    for (const e of regularEntities) {
      liveIds.add(e.id);
      if (isResource(e.kind)) this._drawResource(e, fog);
      else if (isBuilding(e.kind)) this._drawBuilding(e, colorByOwner, state);
    }
    for (const e of regularEntities) {
      liveIds.add(e.id);
      if (isUnit(e.kind)) this._drawUnit(e, colorByOwner, state);
    }
    // Selection rings + HP bars after shapes are placed so they read on top.
    for (const e of regularEntities) {
      liveIds.add(e.id);
      this._drawSelectionAndHp(e, selection, state);
    }
    for (const e of shotReveals) {
      liveIds.add(e.id);
      this._drawShotRevealUnit(e, colorByOwner, state);
    }
    // Hide pooled objects whose id was not touched this frame.
    this._sweep();
    this._sweepSetupVisuals(liveIds);
    this._sweepTankMotion(liveIds);

    // Overlays.
    this._smokeGfx.clear();
    this._drawSmokes(state);
    this._drawFog(fog);
    this._drawSmokeCanisters(state);
    this._drawCommandFeedback(state);
    this._drawMortarTargets(state);
    this._drawMortarLaunches(state);
    this._drawMortarShells(state);
    this._drawMortarImpacts(state);
    this._drawArtilleryLaunches(state);
    this._drawArtilleryTargets(state);
    this._drawArtilleryImpacts(state);
    this._drawSelectedMortarRanges(state);
    this._drawAbilityTargetPreview(state);
    this._drawAtGunSetupPreview(state);
    this._drawOrderPlan(state);
    this._drawDebugPathOverlay(state, regularEntities);
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
  /**
   * @private
   * @returns {number} tint for an owned entity (grey for neutral / unknown owner)
   */
  /**
   * Resolve the current visual transition progress for a deployed weapon.
   * The server only sends the discrete setup state, so the client smooths the
   * transition locally between snapshots.
   * @private
   * @param {{id:number, setupState?:string}} e
   * @returns {{prongFactor:number, barrel:boolean}}
   */
  /**
   * Fetch (or lazily create) a pooled Graphics for `id` in `poolName`, mark it seen,
   * make it visible, and clear it ready for redraw.
   * @private
   * @returns {PIXI.Graphics}
   */
  /** Soft circular drop shadow at (cx,cy) with the given radius. @private */
  /**
   * Drop setup-animation state for entities that are no longer visible.
   * @private
   * @param {Set<number>} liveIds
   */
  /**
   * Drop track-animation state for tanks that are no longer visible.
   * @private
   * @param {Set<number>} liveIds
   */
  /**
   * Derive visual tread movement from actual interpolated tank movement.
   * @private
   * @param {{id:number,x:number,y:number,owner:number,state?:string}} e
   * @param {number} facing
   * @param {import("./state.js").GameState} state
   * @param {{halfLen:number,halfWidth:number}} body
   */
  /**
   * Low-poly PS1 silhouettes tinted by owner. The shapes are intentionally neutral:
   * no national insignia, flags, stars, crosses, eagles, or historical unit badges.
   * @private
   */
  /**
   * Draw a short-lived shot reveal on layers above fog. These entities are visual-only and
   * non-interactive; normal visibility still comes from the authoritative fog-filtered snapshot.
   * @private
   */
  /**
   * Blocky footprint tinted by owner, with a plain two-letter stencil. Under
   * construction (`buildProgress < 1`) → translucent with a horizontal progress bar.
   * Producing (`prodProgress`) → a top-edge progress bar.
   * @private
   */
  /**
    * Resource node: steel = tan supply crates, oil = olive fuel drums; size/opacity
   * scale with `remaining`. Dimmed when the tile is currently not visible (explored
   * memory) so it reads as a remembered node.
   * @private
   */
  /**
   * Selection ring (own=green, enemy=red, neutral=yellow) for selected entities, and
   * an HP bar above any entity that is damaged or selected.
   * @private
   */
  // --- Geometry helpers for rings / hp bars --------------------------------

  /**
   * Footprint-aware selection ring geometry (slightly flattened ellipse hugging the
   * base of the silhouette).
   * @private
   * @returns {{rx:number, ry:number, cy:number}}
   */
  /**
   * Draw an HP bar centered above the entity. Color steps from good→mid→low.
   * @private
   */
  // --- Icon glyphs (pooled Text) -------------------------------------------

  /**
   * Draw / reposition the building's icon glyph. PIXI.Text objects are pooled by
   * entity id on the buildings layer alongside the footprint Graphics.
   * @private
   */
  /**
   * Show/hide the queued-unit count label for a building. Pooled by entity id.
   * @private
   */
  // --- Overlays ------------------------------------------------------------

  /**
   * Draw the fog overlay from the Fog grids: unexplored = heavily dimmed, explored =
   * dimmed at FOG_EXPLORED_ALPHA, visible = clear. Rendered in world space over the
   * whole map; merged into horizontal runs per row to keep the rect count low.
   * @private
   */
  /**
   * @private
   * @returns {0|1|2|3} 0 visible, 1 explored (dim), 2 unexplored (dark), 3 unexplored impassable (light dim)
   */
  /**
   * Draw the build placement ghost from `state.placement`: footprint-sized
   * rounded-rect tinted green (valid) or red (invalid), at the candidate tile.
   * @private
   */
  /** Draw short-lived local markers for issued move / attack commands. @private */
  /** Draw AT gun setup and selected deployed field-of-fire wedges. @private */
  /** Draw selected own units' server-accepted active + queued order plan. @private */
  /**
   * Draw rally-point markers for selected own unit-producing buildings: a faint line from the
   * building to the rally point and a small flag at the point. Appends to the feedback graphics,
   * which `_drawCommandFeedback` clears earlier this frame.
   * @private
   */
  /** Draw the resource hover's nearest City Centre mining link. @private */
  /**
   * Draw a brief muzzle flash on the attacker plus a yellow tracer line to the
   * target, then a fainter continuation past the target for overpenetration.
   * Size scales by attacker kind (tank > AT gun > MG > rifleman).
   * @private
   */
  /**
   * Draw the drag selection rectangle in SCREEN space, or clear it when passed null.
   * @param {{x:number,y:number,w:number,h:number}|null} rect screen-space rect
   */
  // --- Pool maintenance ----------------------------------------------------

  /**
   * Hide every pooled object (per layer + icons) whose id was not touched this
   * frame. We hide rather than destroy during a brief grace period so entities
   * that flicker out of vision for a frame reuse their slot; once an id has been
   * unseen for SWEEP_EVICT_FRAMES we destroy its objects and drop the map
   * entries (server ids are never reused, so they would otherwise grow forever).
   * @private
   */
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
    this._smokeGfx.destroy();
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

Object.assign(Renderer.prototype, {
  buildStaticMap,
  _ownerColors,
  _tintFor,
  _deployedWeaponSetupVisual,
  _slot,
  _shadow,
  _vehicleShadow,
  _sweepSetupVisuals,
  _sweepTankMotion,
  _tankMotionVisual,
  _drawUnit,
  _drawShotRevealUnit,
  _drawBuilding,
  _drawResource,
  _drawSelectionAndHp,
  _ringRadius,
  _hpBar,
  _icon,
  _queueLabel,
  _drawFog,
  _fogLevel,
  _drawPlacement,
  _drawCommandFeedback,
  _drawSmokeCanisters,
  _drawArtilleryLaunches,
  _drawArtilleryTargets,
  _drawArtilleryImpacts,
  _drawAbilityTargetPreview,
  _drawSelectedMortarRanges,
  _drawSmokes,
  _drawAtGunSetupPreview,
  _drawOrderPlan,
  _drawDebugPathOverlay,
  _drawRallyPoints,
  _drawResourceMiningPreview,
  _drawMuzzleFlashes,
  _drawMortarLaunches,
  _drawMortarTargets,
  _drawMortarShells,
  _drawMortarImpacts,
  drawSelectionBox,
  _sweep,
});
