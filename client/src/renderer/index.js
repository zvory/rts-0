// Renderer — PixiJS scene graph + per-frame drawing. See docs/design/client-ui.md §4.1 / §4.2.
//
// Owns a single PIXI.Application whose stage holds one `world` container that is
// positioned/scaled from the Camera each frame, plus a screen-space overlay layer
// for the drag selection box. Layers are drawn back-to-front in this order:
//
//   terrain → decals → trenches → visual-samples → resources → building-shadows → buildings
//   → building-overlays → unit-shadows → trench-occupant-shadows → trench-occupant-lips
//   → units → smokes → selection-rings → hp-bars → fog → visual-sample-labels
//   → shot-reveal-shadows → shot-reveals → feedback/miss-toasts → placement-ghost → drag-box
//
// Terrain is drawn once into a cached RenderTexture (it never changes mid-match).
// Snapshot-backed ground decals and trench terrain stamp into persistent textures.
// Per-entity Graphics are pooled and reconciled by entity id so we never churn the
// scene graph: each frame we touch the live ids, then hide any pooled object whose
// id was not seen.
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
  _drawAbilityObjects,
  _drawBreakthroughAuras,
  _drawArtilleryImpacts,
  _drawArtilleryLaunches,
  _drawArtilleryTargets,
  _drawAntiTankGunSetupPreview,
  _drawAttackTargetPreview,
  _drawCommandFeedback,
  _drawDebugPathOverlay,
  _drawMuzzleFlashes,
  _drawMortarImpacts,
  _drawMortarLaunches,
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
import { _drawPanzerfaustImpacts, _drawPanzerfaustShots } from "./panzerfaust_feedback.js";
import { _drawMissToasts } from "./miss_toasts.js";
import { _drawSelectedMortarRanges, _drawSelectedUnitRanges } from "./unit_ranges.js";
import { _drawFog, _fogLevel } from "./fog.js";
import { buildRendererFeedbackView } from "./feedback_view_model.js";
import { LAYERS, _sweep } from "./layers.js";
import { GroundDecalLayer, _drawGroundDecals, _initGroundDecalsForMap } from "./decals.js";
import { _drawObserverMapAnalysisOverlay } from "./observer_map_analysis.js";
import {
  TrenchDecalLayer,
  _drawOccupiedTrenches,
  _drawTrenches,
  _initTrenchesForMap,
} from "./trenches.js";
import { VisualSampleLayer, _drawVisualSamples } from "./visual_samples.js";
import { createLiveRigDefinitions } from "./rigs/live_routing.js";
import { compileVisualUnitRigCandidates } from "./rigs/visual_override_rigs.js";
import {
  publishVisualUnitOverrideDiagnostics,
  resolveVisualUnitOverrides,
} from "./visual_unit_overrides.js";
import { createLivePngRigAtlases, loadPngRigAtlasTexture } from "./rigs/png_routing.js";
import { createLiveFrameStrips, loadFrameStripTexture } from "./rigs/frame_strip_routing.js";
import { createBuildingRigDefinitions } from "./rigs/building_routing.js";
import { _drawResource } from "./resources.js";
import { buildStaticMap, previewStaticTerrain } from "./terrain.js";
import {
  _deployedWeaponSetupVisual,
  _drawShotRevealUnit,
  _drawUnit,
  _frameStripMovementVisual,
  _rigRenderContextFor,
  _sweepFrameStripMotion,
  _sweepSetupVisuals,
  _sweepTankMotion,
  _tankMotionVisual,
} from "./units.js";

const RENDER_ERROR_LOG_INTERVAL_MS = 5000;
const MISSING_TEXTURE_SIZE_PX = 26;
const MISSING_TEXTURE_MAGENTA = 0xff00ff;
const MISSING_TEXTURE_DARK = 0x141018;

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
    this._groundDecals = new GroundDecalLayer({
      layer: this.layers.decals,
      pixi: PIXI,
      getDocument: () => (typeof document !== "undefined" ? document : null),
      recordDiagnostic: (label, amount) => this._recordRenderDiagnostic(label, amount),
    });
    this._trenchDecals = new TrenchDecalLayer({
      layer: this.layers.trenches,
      pixi: PIXI,
      getDocument: () => (typeof document !== "undefined" ? document : null),
      recordDiagnostic: (label, amount) => this._recordRenderDiagnostic(label, amount),
    });
    this._visualSamples = new VisualSampleLayer({
      sampleLayer: this.layers.visualSamples,
      labelLayer: this.layers.visualSampleLabels,
      pixi: PIXI,
      recordDiagnostic: (label, amount) => this._recordRenderDiagnostic(label, amount),
      recordError: (label, err) => this._recordRenderError(label, err),
    });
    this._visualUnitRigCandidates = null;
    this._visualUnitOverrideDiagnostics = {
      rules: 0,
      activeOverrides: 0,
      errors: 0,
      candidateCount: 0,
    };

    // Long-lived single Graphics for the bulk overlays / per-frame vector draws.
    this._terrainSprite = null; // PIXI.Sprite of the cached terrain RenderTexture
    this._fogGfx = new PIXI.Graphics();
    this.layers.fog.addChild(this._fogGfx);
    this._observerMapAnalysisGfx = new PIXI.Graphics();
    this._observerMapAnalysisHitLayer = new PIXI.Container();
    this._observerMapAnalysisLabels = new PIXI.Container();
    this._observerMapAnalysisLabelPool = new Map();
    this._observerMapAnalysisHitPool = new Map();
    this._observerMapAnalysisTooltip = new PIXI.Text("", {
      fontFamily: "Inter, system-ui, sans-serif",
      fontSize: 13,
      fontWeight: "700",
      fill: 0xfff3c4,
      stroke: 0x0f1115,
      strokeThickness: 4,
      wordWrap: true,
      wordWrapWidth: 320,
    });
    this._observerMapAnalysisTooltip.visible = false;
    this._observerMapAnalysisTooltip.anchor?.set?.(0.5, 1);
    this.layers.feedback.addChild(this._observerMapAnalysisGfx);
    this.layers.feedback.addChild(this._observerMapAnalysisHitLayer);
    this.layers.feedback.addChild(this._observerMapAnalysisLabels);
    this.layers.feedback.addChild(this._observerMapAnalysisTooltip);
    this._feedbackGfx = new PIXI.Graphics();
    this.layers.feedback.addChild(this._feedbackGfx);
    this._missToastPool = new Map();
    this._smokeGfx = new PIXI.Graphics();
    this.layers.smokes.addChild(this._smokeGfx);
    this._abilityObjectGfx = new PIXI.Graphics();
    this.layers.smokes.addChild(this._abilityObjectGfx);
    this._lineProjectileTrails = new Map();
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
      buildingOverlays: new Map(),
      unitShadows: new Map(),
      trenchOccupantShadows: new Map(),
      units: new Map(),
      trenchOccupantLips: new Map(),
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
    // Local frame-strip motion state. Sprite strips animate only when their
    // authoritative or predicted render position is actually changing.
    this._frameStripMotion = new Map();
    // Live SVG rig instances are routed per unit kind. Invalid or missing
    // definitions fail through the renderer's missing-texture guard.
    this._liveRigDefinitionsByKind = createLiveRigDefinitions();
    this._livePngRigAtlasesByKind = createLivePngRigAtlases();
    this._livePngRigAtlasTextures = new Map();
    this._assetReadiness = new Map();
    this._missingTextureEntityIds = new Set();
    this._renderFrameCount = 0;
    this._lastRenderErrorFrame = -1;
    this._loadLivePngRigAtlases();
    this._liveFrameStripsByKind = createLiveFrameStrips();
    this._liveFrameStripTextures = new Map();
    this._loadLiveFrameStrips();
    this._visualFrameStripTextures = new Map();
    this._visualFrameStripTextureLoads = new Map();
    this._buildingRigDefinitions = createBuildingRigDefinitions();
    this._liveRigPools = {
      liveUnitRigShadows: new Map(),
      liveUnitRigs: new Map(),
      liveUnitRigOverlays: new Map(),
      liveUnitRigEffects: new Map(),
      liveShotRevealRigShadows: new Map(),
      liveShotRevealRigs: new Map(),
      liveShotRevealRigOverlays: new Map(),
      liveShotRevealRigEffects: new Map(),
      buildingRigs: new Map(),
    };
    this._liveRigRoutes = {
      liveUnitRigShadows: { poolName: "liveUnitRigShadows", layerName: "unitShadows" },
      liveUnitRigs: { poolName: "liveUnitRigs", layerName: "units" },
      liveUnitRigOverlays: { poolName: "liveUnitRigOverlays", layerName: "units" },
      liveUnitRigEffects: { poolName: "liveUnitRigEffects", layerName: "units" },
      liveShotRevealRigShadows: { poolName: "liveShotRevealRigShadows", layerName: "shotRevealShadows" },
      liveShotRevealRigs: { poolName: "liveShotRevealRigs", layerName: "shotReveals" },
      liveShotRevealRigOverlays: { poolName: "liveShotRevealRigOverlays", layerName: "shotReveals" },
      liveShotRevealRigEffects: { poolName: "liveShotRevealRigEffects", layerName: "shotReveals" },
      buildingRigs: { poolName: "buildingRigs", layerName: "buildings" },
    };
    for (const key of Object.keys(this._liveRigPools)) this._seen[key] = new Set();

    this._renderErrors = new Map();

    /** Map metadata captured by buildStaticMap (tileSize, width, height in tiles). */
    this._map = null;
  }

  _loadLivePngRigAtlases() {
    for (const [kind, atlas] of this._livePngRigAtlasesByKind || []) {
      this._trackVisualAsset(`live-png:${kind}`, loadPngRigAtlasTexture(PIXI, atlas)
        .then((texture) => {
          return this._storeLoadedTexture(this._livePngRigAtlasTextures, kind, texture);
        })
        .catch((err) => {
          if (this._destroyed) return;
          console.warn(`RTS PNG rig atlas disabled for ${kind}: ${err?.message || err}`);
          throw err;
        }), { kind, source: "livePngAtlas" });
    }
  }

  _loadLiveFrameStrips() {
    for (const [kind, strip] of this._liveFrameStripsByKind || []) {
      this._trackVisualAsset(`live-frame-strip:${kind}`, loadFrameStripTexture(PIXI, strip)
        .then((texture) => {
          return this._storeLoadedTexture(this._liveFrameStripTextures, kind, texture);
        })
        .catch((err) => {
          if (this._destroyed) return;
          console.warn(`RTS frame strip disabled for ${kind}: ${err?.message || err}`);
          throw err;
        }), { kind, source: "liveFrameStrip" });
    }
  }

  _trackVisualAsset(id, promise, { kind = "", source = "asset" } = {}) {
    const record = { id, kind, source, status: "pending", message: "" };
    this._assetReadiness.set(id, record);
    record.promise = Promise.resolve(promise).then(
      (value) => {
        record.status = value ? "ready" : "failed";
        if (!value) record.message = `${source} did not produce a texture.`;
        return value;
      },
      (error) => {
        record.status = "failed";
        record.message = error?.message || String(error);
        return null;
      },
    );
    return record.promise;
  }

  captureReadiness({ subjectIds = [], subjectKinds = [] } = {}) {
    const ids = new Set(subjectIds.filter(Number.isInteger));
    const kinds = new Set(subjectKinds.filter((kind) => typeof kind === "string" && kind));
    const assets = [...this._assetReadiness.values()]
      .filter((asset) => !asset.kind || kinds.size === 0 || kinds.has(asset.kind))
      .map((asset) => ({
        id: asset.id,
        kind: asset.kind || null,
        source: asset.source,
        status: asset.status,
        message: asset.message || null,
      }));
    const decal = this.groundDecalDiagnostics();
    if (decal.assetStatus !== "idle") {
      assets.push({
        id: "ground-decals",
        kind: null,
        source: "groundDecalAtlas",
        status: decal.assetStatus === "ready" ? "ready" : decal.assetStatus,
        message: this._groundDecals?.assetLoadError?.message || null,
      });
    }
    const renderErrors = this._lastRenderErrorFrame === this._renderFrameCount
      ? [...this._renderErrors.entries()].map(([label, value]) => ({
        label,
        count: value.count,
        message: value.lastMessage || "",
      }))
      : [];
    const missingTextureSubjectIds = [...this._missingTextureEntityIds].filter((id) => ids.has(id));
    return {
      frame: this._renderFrameCount,
      assets,
      ready: assets.every((asset) => asset.status === "ready" || asset.status === "idle"),
      failedAssets: assets.filter((asset) => asset.status === "failed"),
      pendingAssets: assets.filter((asset) => asset.status === "pending"),
      renderErrors,
      missingTextureSubjectIds,
    };
  }

  _storeLoadedTexture(map, key, texture) {
    if (this._destroyed) {
      destroyRendererOwnedTexture(texture);
      return null;
    }
    if (texture) map?.set?.(key, texture);
    return texture || null;
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
  render(state, camera, fog, alpha, {
    clientIntent = null,
    frameViews = null,
    profiler = null,
    visualSamples = null,
    visualUnitOverrides = null,
    visualFrameStripOverrides = null,
    observerMapAnalysis = null,
  } = {}) {
    this._beginRenderFrame();
    this._profiler = profiler || null;
    const time = (label, fn) => profiler ? profiler.time(label, fn) : fn();
    // Drive the world container from the camera (single transform for all layers).
    this.world.position.set(-camera.x * camera.zoom, -camera.y * camera.zoom);
    this.world.scale.set(camera.zoom);

    // Begin a fresh reconciliation pass.
    for (const key of Object.keys(this._seen)) this._seen[key].clear();

    let entities = [];
    let regularEntities = [];
    let shotReveals = [];
    let selection = new Set();
    let colorByOwner = new Map();
    const liveIds = new Set();
    time("renderer.entityPrep", () => {
      this._recordRenderDiagnostic(
        Array.isArray(frameViews?.interpolatedEntities)
          ? "entityViews.cache.hit.renderer.interpolated"
          : "entityViews.uncached.renderer.interpolated",
      );
      entities = Array.isArray(frameViews?.interpolatedEntities)
        ? frameViews.interpolatedEntities
        : state.entitiesInterpolated(alpha) || [];
      regularEntities = entities.filter((e) => !e.shotReveal);
      shotReveals = entities.filter((e) => e.shotReveal);
      selection = state.selection || new Set();
      colorByOwner = this._ownerColors(state);
      profiler?.setContext({
        entityCount: entities.length,
        regularEntityCount: regularEntities.length,
        shotRevealCount: shotReveals.length,
        rememberedBuildingCount: Array.isArray(state?.rememberedBuildings) ? state.rememberedBuildings.length : 0,
        selectedCount: typeof state?.selection?.size === "number" ? state.selection.size : 0,
      });
      this._recordRenderDiagnostic("renderer.entities.total", entities.length);
      this._recordRenderDiagnostic("renderer.entities.regular", regularEntities.length);
      this._recordRenderDiagnostic("renderer.entities.shotReveal", shotReveals.length);
    });
    const feedbackView = time(
      "renderer.feedbackView",
      () => buildRendererFeedbackView(state, {
        clientIntent,
        entities,
        selectedEntities: frameViews?.selectedEntities,
      }),
    );
    time("renderer.groundDecals", () => {
      this._drawSafely("groundDecals", () => this._drawGroundDecals(state));
    });
    time("renderer.trenches", () => {
      this._drawSafely("trenches", () => this._drawTrenches(state));
    });
    time("renderer.visualSamples", () => {
      this._drawSafely("visualSamples", () => this._drawVisualSamples(visualSamples, { state, camera }));
    });
    let visualUnitOverrideMap = new Map();
    time("renderer.visualUnitOverrides", () => {
      visualUnitOverrideMap = this._resolveVisualUnitOverridesSafely(visualUnitOverrides, entities).overrides;
    });
    let visualFrameStripOverrideMap = new Map();
    time("renderer.visualFrameStripOverrides", () => {
      visualFrameStripOverrideMap = this._resolveVisualFrameStripOverrides(visualFrameStripOverrides);
    });
    time("renderer.trenchOccupants", () => {
      this._drawSafely("trenchOccupants", () => this._drawOccupiedTrenches(regularEntities, state));
    });

    // Nodes currently being mined: any worker latched to them. Used by
    // _drawResource to overlay an X marker.
    this._miningNodes = new Set();
    time("renderer.miningPrep", () => {
      for (const e of regularEntities) {
        if (e.latchedNode) this._miningNodes.add(e.latchedNode);
      }
    });

    // Two passes so silhouettes layer correctly: resources + buildings first
    // (footprints sit under units), then units. Selection rings / hp bars are
    // their own layers and are filled inline.
    time("renderer.resourcesBuildings", () => {
      for (const e of regularEntities) {
        liveIds.add(e.id);
        if (isResource(e.kind)) {
          this._drawEntitySafely("resource", e, "resources", () => this._drawResource(e, fog));
        } else if (isBuilding(e.kind)) {
          this._drawEntitySafely("building", e, "buildings", () => this._drawBuilding(e, colorByOwner, state));
        }
      }
      for (const e of state.rememberedBuildings || []) {
        this._drawEntitySafely("rememberedBuilding", e, "buildings", () => {
          this._drawBuilding(e, colorByOwner, state);
        });
      }
    });
    time("renderer.units", () => {
      for (const e of regularEntities) {
        liveIds.add(e.id);
        if (isUnit(e.kind)) {
          this._drawEntitySafely("unit", e, "units", () => {
            this._drawUnit(e, colorByOwner, state, {
              visualOverride: visualUnitOverrideMap.get(e.id) || null,
              visualFrameStrip: visualFrameStripOverrideMap.get(e.kind) || null,
            });
          });
        }
      }
    });
    // Selection rings + HP bars after shapes are placed so they read on top.
    time("renderer.selectionHp", () => {
      for (const e of regularEntities) {
        liveIds.add(e.id);
        this._drawSafely(`selectionHp:${e.kind || "unknown"}`, () => {
          this._drawSelectionAndHp(e, selection, feedbackView);
        });
      }
    });
    time("renderer.shotReveals", () => {
      for (const e of shotReveals) {
        liveIds.add(e.id);
        this._drawEntitySafely("shotReveal", e, "shotReveals", () => {
          this._drawShotRevealUnit(e, colorByOwner, state, {
            visualOverride: visualUnitOverrideMap.get(e.id) || null,
            visualFrameStrip: visualFrameStripOverrideMap.get(e.kind) || null,
          });
        });
      }
    });
    // Hide pooled objects whose id was not touched this frame.
    time("renderer.sweeps", () => {
      this._sweep();
      this._sweepSetupVisuals(liveIds);
      this._sweepTankMotion(liveIds);
      this._sweepFrameStripMotion(liveIds);
    });

    // Overlays.
    time("renderer.effectsOverlays", () => {
      this._recordRenderDiagnostic("renderer.graphics.clear.abilityObjects");
      this._abilityObjectGfx.clear();
      this._drawSafely("abilityObjects", () => this._drawAbilityObjects(feedbackView));
      this._recordRenderDiagnostic("renderer.graphics.clear.smokes");
      this._smokeGfx.clear();
      this._drawSafely("smokes", () => this._drawSmokes(feedbackView));
    });
    time("renderer.fogDraw", () => this._drawSafely("fog", () => this._drawFog(fog)));
    time("renderer.observerMapAnalysis", () => {
      this._drawSafely(
        "observerMapAnalysis",
        () => this._drawObserverMapAnalysisOverlay(observerMapAnalysis, { camera }),
      );
    });
    time("renderer.feedbackOverlays", () => {
      this._drawSafely("smokeCanisters", () => this._drawSmokeCanisters(feedbackView));
      this._drawSafely("commandFeedback", () => this._drawCommandFeedback(feedbackView));
      this._drawSafely("attackTargetPreview", () => this._drawAttackTargetPreview(feedbackView));
      this._drawSafely("mortarTargets", () => this._drawMortarTargets(feedbackView));
      this._drawSafely("mortarLaunches", () => this._drawMortarLaunches(feedbackView));
      this._drawSafely("mortarShells", () => this._drawMortarShells(feedbackView));
      this._drawSafely("mortarImpacts", () => this._drawMortarImpacts(feedbackView));
      this._drawSafely("artilleryLaunches", () => this._drawArtilleryLaunches(feedbackView));
      this._drawSafely("artilleryTargets", () => this._drawArtilleryTargets(feedbackView));
      this._drawSafely("artilleryImpacts", () => this._drawArtilleryImpacts(feedbackView));
      this._drawSafely("panzerfaustShots", () => this._drawPanzerfaustShots(feedbackView));
      this._drawSafely("panzerfaustImpacts", () => this._drawPanzerfaustImpacts(feedbackView));
      this._drawSafely("selectedUnitRanges", () => this._drawSelectedUnitRanges(feedbackView));
      this._drawSafely("breakthroughAuras", () => this._drawBreakthroughAuras(feedbackView, regularEntities));
      this._drawSafely("abilityTargetPreview", () => this._drawAbilityTargetPreview(feedbackView));
      this._drawSafely("antiTankGunSetupPreview", () => this._drawAntiTankGunSetupPreview(feedbackView));
      this._drawSafely("orderPlan", () => this._drawOrderPlan(feedbackView));
      this._drawSafely("debugPathOverlay", () => this._drawDebugPathOverlay(feedbackView, regularEntities));
      this._drawSafely("rallyPoints", () => this._drawRallyPoints(feedbackView));
      this._drawSafely("resourceMiningPreview", () => this._drawResourceMiningPreview(feedbackView));
      this._drawSafely("muzzleFlashes", () => this._drawMuzzleFlashes(feedbackView));
      this._drawSafely("missToasts", () => this._drawMissToasts(feedbackView));
    });
    time("renderer.placement", () => this._drawSafely("placement", () => this._drawPlacement(feedbackView, fog)));
  }

  _beginRenderFrame() {
    this._renderFrameCount += 1;
    // Capture readiness is a property of the current rendered frame. A texture can
    // legitimately be unavailable while an atlas is loading, then render normally
    // once it resolves; do not keep that transient fallback pinned forever.
    this._missingTextureEntityIds.clear();
  }

  _drawSafely(label, draw) {
    const safeLabel = diagnosticSegment(label);
    this._recordRenderDiagnostic(`renderer.redraw.attempt.${safeLabel}`);
    try {
      draw();
      this._recordRenderDiagnostic(`renderer.redraw.completed.${safeLabel}`);
      return true;
    } catch (err) {
      this._recordRenderDiagnostic(`renderer.redraw.failed.${safeLabel}`);
      this._recordRenderError(label, err);
      return false;
    }
  }

  _drawEntitySafely(label, entity, fallbackPool, draw) {
    const safeLabel = diagnosticSegment(label);
    this._recordRenderDiagnostic(`renderer.redraw.attempt.${safeLabel}`);
    try {
      draw();
      this._recordRenderDiagnostic(`renderer.redraw.completed.${safeLabel}`);
      return true;
    } catch (err) {
      this._recordRenderDiagnostic(`renderer.redraw.failed.${safeLabel}`);
      this._recordRenderError(`${label}:${entity?.kind || "unknown"}`, err);
      try {
        this._drawMissingTexture(entity, fallbackPool);
      } catch (fallbackErr) {
        this._recordRenderError(`${label}:missingTexture`, fallbackErr);
      }
      return false;
    }
  }

  _recordRenderDiagnostic(label, amount = 1) {
    this._profiler?.recordDiagnosticCounter?.(label, amount);
  }

  groundDecalDiagnostics() {
    return this._groundDecals?.diagnostics?.() || {
      totalStamped: 0,
      pendingDecals: 0,
      textureUpdateCount: 0,
      textureWidth: 0,
      textureHeight: 0,
      downsample: 0,
      layerChildCount: 0,
      assetStatus: "idle",
    };
  }

  trenchDiagnostics() {
    return this._trenchDecals?.diagnostics?.() || {
      visibleTrenches: 0,
      totalStamped: 0,
      textureUpdateCount: 0,
      textureWidth: 0,
      textureHeight: 0,
      downsample: 0,
      layerChildCount: 0,
    };
  }

  visualSampleDiagnostics() {
    return this._visualSamples?.diagnostics?.() || {
      visibleSamples: 0,
      invalidSamples: 0,
      totalRendered: 0,
      sampleDisplayObjects: 0,
      labelDisplayObjects: 0,
      layerChildCount: 0,
    };
  }

  visualUnitOverrideDiagnostics() {
    return this._visualUnitOverrideDiagnostics || {
      rules: 0,
      activeOverrides: 0,
      errors: 0,
      candidateCount: 0,
    };
  }

  _visualUnitRigCandidateRegistry() {
    if (!this._visualUnitRigCandidates) {
      this._visualUnitRigCandidates = compileVisualUnitRigCandidates();
    }
    return this._visualUnitRigCandidates;
  }

  _resolveVisualUnitOverridesSafely(rules, entities) {
    try {
      return this._resolveVisualUnitOverrides(rules, entities);
    } catch (err) {
      const error = Object.freeze({
        reason: "resolver-error",
        ruleId: "renderer",
        index: -1,
        candidateId: "",
        message: `Visual unit override resolution failed: ${err?.message || String(err)}`,
      });
      const result = {
        overrides: new Map(),
        errors: [error],
        diagnostics: Object.freeze({
          rules: Array.isArray(rules) ? rules.length : 0,
          activeOverrides: 0,
          errors: 1,
        }),
      };
      this._visualUnitOverrideDiagnostics = {
        ...result.diagnostics,
        candidateCount: this._visualUnitRigCandidates?.definitions?.size || 0,
      };
      publishVisualUnitOverrideDiagnostics(result);
      this._recordRenderError("visualUnitOverrides", err);
      this._recordRenderDiagnostic("renderer.visualUnitOverrides.active", 0);
      this._recordRenderDiagnostic("renderer.visualUnitOverrides.invalid", 1);
      return result;
    }
  }

  _resolveVisualUnitOverrides(rules, entities) {
    if (!Array.isArray(rules) || rules.length === 0) {
      const empty = {
        overrides: new Map(),
        errors: [],
        diagnostics: { rules: 0, activeOverrides: 0, errors: 0 },
      };
      this._visualUnitOverrideDiagnostics = {
        ...empty.diagnostics,
        candidateCount: this._visualUnitRigCandidates?.definitions?.size || 0,
      };
      publishVisualUnitOverrideDiagnostics(empty);
      return empty;
    }
    const registry = this._visualUnitRigCandidateRegistry();
    const resolved = resolveVisualUnitOverrides(rules, entities, registry.definitions, {
      candidateErrors: registry.errors,
    });
    this._visualUnitOverrideDiagnostics = {
      ...resolved.diagnostics,
      candidateCount: registry.definitions.size,
    };
    publishVisualUnitOverrideDiagnostics(resolved);
    for (const error of resolved.errors) {
      this._recordRenderError(
        `visualUnitOverride:${error.ruleId}:${error.reason}`,
        new Error(error.message),
      );
    }
    this._recordRenderDiagnostic("renderer.visualUnitOverrides.active", resolved.overrides.size);
    this._recordRenderDiagnostic("renderer.visualUnitOverrides.invalid", resolved.errors.length);
    return resolved;
  }

  _resolveVisualFrameStripOverrides(entries) {
    const resolved = new Map();
    const list = Array.isArray(entries) ? entries : [];
    for (const entry of list) {
      const kind = typeof entry?.kind === "string" ? entry.kind : "";
      const strip = entry?.strip || null;
      if (!kind || !strip?.enabled || !strip?.image) continue;
      const texture = this._visualFrameStripTextureFor(kind, strip);
      if (texture) resolved.set(kind, { strip, texture });
    }
    this._recordRenderDiagnostic("renderer.visualFrameStripOverrides.active", resolved.size);
    return resolved;
  }

  _visualFrameStripTextureFor(kind, strip) {
    const key = `${kind}:${strip.imageVersion || strip.image}`;
    if (this._visualFrameStripTextures.has(key)) {
      return this._visualFrameStripTextures.get(key);
    }
    if (!this._visualFrameStripTextureLoads.has(key)) {
      const load = this._trackVisualAsset(`visual-frame-strip:${key}`, loadFrameStripTexture(PIXI, strip)
        .then((texture) => {
          return this._storeLoadedTexture(this._visualFrameStripTextures, key, texture);
        })
        .catch((err) => {
          if (this._destroyed) return null;
          this._visualFrameStripTextures.set(key, null);
          console.warn(`RTS visual frame strip disabled for ${kind}: ${err?.message || err}`);
          throw err;
        }), { kind, source: "visualFrameStrip" });
      this._visualFrameStripTextureLoads.set(key, load);
    }
    return null;
  }

  _drawMissingTexture(entity, poolName) {
    if (!entity || entity.id == null || !this._pools?.[poolName]) return;
    if (Number.isInteger(entity.id)) this._missingTextureEntityIds.add(entity.id);
    const g = this._slot(poolName, entity.id);
    const x = Number.isFinite(entity.x) ? entity.x : 0;
    const y = Number.isFinite(entity.y) ? entity.y : 0;
    const size = MISSING_TEXTURE_SIZE_PX;
    const half = size / 2;
    const cell = size / 2;
    g.position.set(x, y);
    g.lineStyle(2, 0x0b0710, 0.95);
    g.beginFill(MISSING_TEXTURE_MAGENTA, 1);
    g.drawRect(-half, -half, cell, cell);
    g.drawRect(0, 0, cell, cell);
    g.beginFill(MISSING_TEXTURE_DARK, 1);
    g.drawRect(0, -half, cell, cell);
    g.drawRect(-half, 0, cell, cell);
    g.endFill();
    g.lineStyle(2, 0xffffff, 0.85);
    g.drawRect(-half, -half, size, size);
  }

  _recordRenderError(label, err) {
    this._lastRenderErrorFrame = this._renderFrameCount;
    const now = typeof performance !== "undefined" && typeof performance.now === "function"
      ? performance.now()
      : Date.now();
    const record = this._renderErrors.get(label) || { count: 0, lastLogAt: -Infinity, lastMessage: "" };
    record.count += 1;
    record.lastMessage = err?.stack || err?.message || String(err);
    this._renderErrors.set(label, record);
    globalThis.__rtsRenderErrors = {
      total: Array.from(this._renderErrors.values()).reduce((sum, item) => sum + item.count, 0),
      labels: Object.fromEntries(
        Array.from(this._renderErrors.entries()).map(([key, value]) => [key, value.count]),
      ),
      latest: { label, message: record.lastMessage },
    };
    if (record.count <= 3 || now - record.lastLogAt >= RENDER_ERROR_LOG_INTERVAL_MS) {
      record.lastLogAt = now;
      console.error(`[RTS_RENDER] skipped ${label} after render error`, err);
    }
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
   * construction (`buildProgress < 1`) → translucent body; construction and
   * deconstruction status use the HP bar layer. Producing (`prodProgress`) →
   * a top-edge progress bar.
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
   * one status bar above any entity that is damaged, selected, constructing, or
   * deconstructing.
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
   * Draw a status bar centered above the entity. HP color steps from good→mid→low;
   * construction/deconstruction can supply their own progress fraction.
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
   * Draw the build placement ghost from the feedback view: footprint-sized
   * rounded-rect tinted green (valid) or red (invalid), at the candidate tile.
   * @private
   */
  /** Draw short-lived local markers for issued move / attack commands. @private */
  /** Draw anti-tank gun setup and selected deployed field-of-fire wedges. @private */
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
   * Size scales by attacker kind (tank > anti-tank gun > MG > rifleman).
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
    if (this._missToastPool) {
      for (const t of this._missToastPool.values()) t.destroy();
      this._missToastPool.clear();
    }
    this._unseen.clear();
    this._setupVisuals.clear();
    this._assetReadiness?.clear?.();
    this._missingTextureEntityIds?.clear?.();
    this._tankMotion.clear();
    if (this._liveRigPools) {
      for (const pool of Object.values(this._liveRigPools)) {
        for (const instance of pool.values()) instance.destroy();
        pool.clear();
      }
    }
    destroyRendererTextureMap(this._livePngRigAtlasTextures);
    destroyRendererTextureMap(this._liveFrameStripTextures);
    destroyRendererTextureMap(this._visualFrameStripTextures);
    this._visualFrameStripTextureLoads?.clear?.();
    this._lineProjectileTrails.clear();

    // Long-lived single Graphics.
    this._fogGfx.destroy();
    this._feedbackGfx.destroy();
    this._observerMapAnalysisGfx.destroy();
    this._observerMapAnalysisHitLayer.destroy({ children: true });
    this._observerMapAnalysisLabels.destroy({ children: true });
    this._observerMapAnalysisTooltip.destroy();
    this._observerMapAnalysisLabelPool.clear();
    this._observerMapAnalysisHitPool.clear();
    this._smokeGfx.destroy();
    this._abilityObjectGfx.destroy();
    this._placementGfx.destroy();
    this._dragGfx.destroy();
    this._groundDecals?.destroy();
    this._groundDecals = null;
    this._trenchDecals?.destroy();
    this._trenchDecals = null;
    this._visualSamples?.destroy();
    this._visualSamples = null;

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

function destroyRendererTextureMap(map) {
  if (!map) return;
  for (const texture of map.values()) destroyRendererOwnedTexture(texture);
  map.clear?.();
}

function destroyRendererOwnedTexture(texture) {
  if (!texture?.rtsRendererOwnedTexture) return;
  if (texture.destroyed) return;
  texture.destroy?.(true);
  texture.baseTexture?.destroy?.();
}

Object.assign(Renderer.prototype, {
  buildStaticMap,
  previewStaticTerrain,
  _initGroundDecalsForMap,
  _initTrenchesForMap,
  _drawGroundDecals,
  _drawObserverMapAnalysisOverlay,
  _drawTrenches,
  _drawOccupiedTrenches,
  _drawVisualSamples,
  _ownerColors,
  _tintFor,
  _deployedWeaponSetupVisual,
  _slot,
  _shadow,
  _vehicleShadow,
  _sweepFrameStripMotion,
  _sweepSetupVisuals,
  _sweepTankMotion,
  _tankMotionVisual,
  _frameStripMovementVisual,
  _drawUnit,
  _rigRenderContextFor,
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
  _drawAttackTargetPreview,
  _drawSmokeCanisters,
  _drawArtilleryLaunches,
  _drawArtilleryTargets,
  _drawArtilleryImpacts,
  _drawPanzerfaustShots,
  _drawPanzerfaustImpacts,
  _drawAbilityObjects,
  _drawAbilityTargetPreview,
  _drawBreakthroughAuras,
  _drawSelectedUnitRanges,
  _drawSelectedMortarRanges,
  _drawSmokes,
  _drawAntiTankGunSetupPreview,
  _drawOrderPlan,
  _drawDebugPathOverlay,
  _drawRallyPoints,
  _drawResourceMiningPreview,
  _drawMuzzleFlashes,
  _drawMissToasts,
  _drawMortarLaunches,
  _drawMortarTargets,
  _drawMortarShells,
  _drawMortarImpacts,
  drawSelectionBox,
  _sweep,
});

function diagnosticSegment(label) {
  return String(label || "unknown").replace(/[^A-Za-z0-9_.:-]/g, "_").slice(0, 48) || "unknown";
}
