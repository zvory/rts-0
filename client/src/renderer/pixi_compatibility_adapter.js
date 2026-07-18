import { isResource } from "../protocol.js";
import { Renderer } from "./index.js";

export const PIXI_LEGACY_READ_ALLOWLIST = Object.freeze([
  Object.freeze({ id: "state.resources.oil", reviewTrigger: "a Pixi DTO closure needs the low-oil cue" }),
  Object.freeze({ id: "state._curById", reviewTrigger: "a pose DTO or effect needs normalized current poses" }),
  Object.freeze({ id: "state._prevById", reviewTrigger: "a pose DTO or effect needs normalized previous poses" }),
  Object.freeze({ id: "state.weaponRecoil", reviewTrigger: "an effect needs normalized recoil data" }),
  Object.freeze({ id: "state.weaponRecoilPhase", reviewTrigger: "an effect needs normalized recoil data" }),
  Object.freeze({ id: "state.weaponRecoilKind", reviewTrigger: "authored recoil art is weapon-specific" }),
  Object.freeze({ id: "match.renderClock", reviewTrigger: "the existing Pixi capture path changes" }),
  Object.freeze({ id: "match.frameProfiler", reviewTrigger: "measurement needs backend-neutral metrics" }),
  Object.freeze({ id: "match.visualProfile.unitOverrides", reviewTrigger: "a playtest needs representative visuals" }),
  Object.freeze({ id: "match.visualProfile.frameStripOverrides", reviewTrigger: "a playtest needs representative visuals" }),
  Object.freeze({ id: "match.presentationAssembler.staticMap", reviewTrigger: "Babylon staging needs a shared static-map DTO" }),
]);

const FEEDBACK_ARRAY_TYPES = Object.freeze({
  command: "commandFeedback",
  smokeCanister: "smokeCanisters",
  mortarLaunch: "mortarLaunches",
  mortarShell: "mortarShells",
  mortarTarget: "mortarTargets",
  mortarImpact: "mortarImpacts",
  artilleryTarget: "artilleryTargets",
  artilleryLaunch: "artilleryLaunches",
  artilleryImpact: "artilleryImpacts",
  panzerfaustShot: "panzerfaustShots",
  panzerfaustImpact: "panzerfaustImpacts",
  muzzleFlash: "muzzleFlashes",
  missToast: "missToasts",
});

const FEEDBACK_SINGLETON_TYPES = Object.freeze({
  placement: "placement",
  labToolPreview: "labToolPreview",
  attackTargetPreview: "attackTargetPreview",
  supportWeaponSetupPreview: "antiTankGunSetupPreview",
  abilityTargetPreview: "abilityTargetPreview",
  resourceMiningPreview: "resourceMiningPreview",
});

/**
 * Pixi-only bridge from PresentationFrameV1 to the existing Pixi drawing helpers.
 * Match knows only render(frame); the mutable legacy sources below are sampled once
 * for each new immutable frame and are never exposed to another backend.
 */
export class PixiPresentationAdapter {
  constructor(canvasParent, sources, { renderer = null } = {}) {
    this.id = "pixi";
    this._sources = sources || {};
    this._renderer = renderer || new Renderer(canvasParent, { renderClock: this._sources.renderClock });
    this._lastFrame = null;
    this._lastView = null;
    this._staticMapRevision = null;
    this._decalFrameKey = null;
    this._groundDecalsAwaitingPresent = false;
    this._destroyed = false;
  }

  get app() {
    return this._renderer.app;
  }

  get _renderFrameCount() {
    return this._renderer._renderFrameCount;
  }

  render(frame) {
    const profiler = this._sources?.profiler?.() || null;
    const time = (label, fn) => profiler ? profiler.time(label, fn) : fn();
    try {
      if (!frame || frame.version !== 1) throw new TypeError("Pixi requires PresentationFrameV1.");
      time("renderer.update", () => {
        this._ensureStaticMap(frame);
        const repeated = frame === this._lastFrame;
        const view = repeated ? this._lastView : this._buildView(frame);
        if (!repeated) {
          this._lastFrame = frame;
          this._lastView = view;
        }
        const frameKey = `${frame.generation}:${frame.frameId}`;
        const groundDecals = frameKey === this._decalFrameKey || this._groundDecalsAwaitingPresent
          ? []
          : view.groundDecals;
        this._renderer.render(view.state, view.camera, view.fog, view.alpha, {
          frameViews: view.frameViews,
          profiler: view.profiler,
          visualSamples: view.visualSamples,
          visualUnitOverrides: view.visualUnitOverrides,
          visualFrameStripOverrides: view.visualFrameStripOverrides,
          observerMapAnalysis: view.observerMapAnalysis,
          feedbackView: view.feedback,
          reconciledGroundDecals: groundDecals,
          onGroundDecalsStaged: () => {
            this._groundDecalsAwaitingPresent = true;
          },
        });
        this._renderer.drawSelectionBox(view.marquee);
        this._decalFrameKey = frameKey;
      });
      time("renderer.present", () => this._present());
      this._groundDecalsAwaitingPresent = false;
      return Object.freeze({ presented: true });
    } catch (err) {
      this._renderer?._recordRenderError?.("pixiPresentationFrame", err);
      return Object.freeze({ presented: false });
    }
  }

  resize(widthCssPx, heightCssPx) {
    this._renderer.resize(widthCssPx, heightCssPx);
  }

  enterFixedCapture(renderClock) {
    this._renderer.enterFixedCapture(renderClock);
  }

  exitFixedCapture(renderClock) {
    this._renderer.exitFixedCapture(renderClock);
  }

  captureReadiness(query) {
    return this._renderer.captureReadiness(query);
  }

  groundDecalDiagnostics() {
    return this._renderer.groundDecalDiagnostics();
  }

  trenchDiagnostics() {
    return this._renderer.trenchDiagnostics();
  }

  visualSampleDiagnostics() {
    return this._renderer.visualSampleDiagnostics();
  }

  visualUnitOverrideDiagnostics() {
    return this._renderer.visualUnitOverrideDiagnostics();
  }

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    this._lastFrame = null;
    this._lastView = null;
    this._groundDecalsAwaitingPresent = false;
    this._sources = null;
    this._renderer.destroy();
  }

  _present() {
    this._renderer.present();
  }

  _ensureStaticMap(frame) {
    if (this._staticMapRevision === frame.staticMapRevision) return;
    const staticMap = this._sources?.staticMap?.();
    if (!staticMap || staticMap.revision !== frame.staticMapRevision) {
      throw new Error("Pixi static-map revision is unavailable for this presentation frame.");
    }
    this._renderer.buildStaticMap(materializeStaticMap(staticMap));
    this._staticMapRevision = staticMap.revision;
  }

  _buildView(frame) {
    const layers = frame.layers;
    const visibleEntities = recordsOfType(layers.fogGatedWorld, "entity");
    const intelEntities = recordsOfType(layers.belowFogIntel, "intelEntity")
      .map((entity) => ({ ...entity, visionOnly: true }));
    const shotReveals = recordsOfType(layers.aboveFogReveal, "shotRevealEntity")
      .map((entity) => ({ ...entity, shotReveal: true }));
    const entities = [...visibleEntities, ...intelEntities, ...shotReveals];
    const rememberedBuildings = recordsOfType(layers.rememberedWorld, "rememberedBuilding");
    const trenches = recordsOfType(layers.persistentGroundMark, "trench");
    const groundDecals = recordsOfType(layers.persistentGroundMark, "groundDecal");
    const visualSamples = recordsOfType(layers.persistentGroundMark, "visualSample");
    const smokes = recordsOfType(layers.fogGatedWorld, "smoke");
    const abilityObjects = recordsOfType(layers.fogGatedWorld, "abilityObject");
    const feedback = buildFeedbackView(frame, entities, {
      smokes,
      abilityObjects,
      resourceSites: this._renderer._map?.resources || [],
    });
    const sourceState = this._sources?.state?.() || null;
    const profiler = this._sources?.profiler?.() || null;
    const visualProfile = this._sources?.visualProfile?.() || null;
    const legacy = snapshotLegacyState(sourceState, entities, frame.visualTimeMs, profiler);
    const map = this._renderer._map;
    const state = buildStateFacade(frame, entities, rememberedBuildings, trenches, feedback, legacy, map);
    return {
      state,
      camera: buildCameraFacade(frame.projection),
      fog: buildFogFacade(frame),
      alpha: frame.diagnosticsContext.interpolationAlpha,
      frameViews: Object.freeze({
        interpolatedEntities: entities,
        selectedEntities: feedback.selectedEntities(),
      }),
      profiler,
      visualSamples,
      visualUnitOverrides: visualProfile?.unitOverrides || null,
      visualFrameStripOverrides: visualProfile?.frameStripOverrides || null,
      observerMapAnalysis: recordOfType(layers.tacticalFeedback, "observerMapAnalysis")?.model || null,
      feedback,
      marquee: marqueeForFrame(layers.screenOverlay),
      groundDecals,
    };
  }
}

function materializeStaticMap(staticMap) {
  const width = staticMap.terrain.width;
  const height = staticMap.terrain.height;
  const terrain = new Uint8Array(width * height);
  staticMap.terrain.copyInto(terrain);
  return {
    width,
    height,
    tileSize: staticMap.tileSizePx,
    terrain,
    resources: staticMap.resourceSites.map((resource) => ({ ...resource })),
  };
}

function buildCameraFacade(projection) {
  const scale = positiveNumber(projection?.camera?.framingScale, 1);
  const focus = projection?.camera?.focus || { x: 0, y: 0 };
  const width = finiteNumber(projection?.viewport?.widthCssPx, 0);
  const height = finiteNumber(projection?.viewport?.heightCssPx, 0);
  return Object.freeze({
    x: finiteNumber(focus.x, 0) - width / (2 * scale),
    y: finiteNumber(focus.y, 0) - height / (2 * scale),
    zoom: scale,
    snapshot: projection.snapshot,
    projectedExtent: projection.projectedExtent,
  });
}

function buildFogFacade(frame) {
  const width = frame.visible.width;
  const height = frame.visible.height;
  const visible = new Uint8Array(width * height);
  const explored = new Uint8Array(width * height);
  frame.visible.copyInto(visible);
  frame.explored.copyInto(explored);
  return Object.freeze({
    width,
    height,
    revision: Math.max(frame.visible.revision, frame.explored.revision),
    visibleRevision: frame.visible.revision,
    exploredRevision: frame.explored.revision,
    revealAll: false,
    isVisible(tx, ty) {
      return gridValue(visible, width, height, tx, ty) === 1;
    },
    isExplored(tx, ty) {
      return gridValue(explored, width, height, tx, ty) === 1;
    },
  });
}

function buildFeedbackView(frame, entities, { smokes, abilityObjects, resourceSites }) {
  const arrays = Object.fromEntries(Object.values(FEEDBACK_ARRAY_TYPES).map((name) => [name, []]));
  const singletons = Object.fromEntries(Object.values(FEEDBACK_SINGLETON_TYPES).map((name) => [name, null]));
  let context = null;
  for (const record of frame.layers.tacticalFeedback) {
    const arrayName = FEEDBACK_ARRAY_TYPES[record.type];
    if (arrayName) arrays[arrayName].push(record);
    const singletonName = FEEDBACK_SINGLETON_TYPES[record.type];
    if (singletonName) singletons[singletonName] = record;
    if (record.type === "feedbackContext") context = record;
  }
  const entityLookup = new Map(entities.map((entity) => [entity.id, entity]));
  const selected = entities.filter((entity) => entity.selected);
  const resourceEntities = mergedResources(
    resourceSites,
    entities.filter((entity) => isResource(entity.kind)),
  );
  const feedbackOwnerIds = context?.feedbackOwnerIds || [];
  const feedbackOwnerSet = new Set(feedbackOwnerIds);
  const relationship = relationshipResolver(entities);
  const view = {
    ...arrays,
    ...singletons,
    playerId: context?.feedbackOwnerId ?? null,
    feedbackOwnerId: context?.feedbackOwnerId ?? null,
    feedbackOwnerIds,
    issueAsOwnerId: context?.issueAsOwnerId ?? null,
    showUnitRangesEnabled: context?.showUnitRangesEnabled !== false,
    showSelectedFieldOfFireEnabled: !!context?.showSelectedFieldOfFireEnabled,
    debugPathOverlaysEnabled: !!context?.debugPathOverlaysEnabled,
    showAllDebugPathOverlays: !!context?.showAllDebugPathOverlays,
    abilityObjects,
    smokes,
    map: { resources: resourceEntities },
    selectedEntities: () => selected,
    entityById: (id) => entityLookup.get(id),
    canControlOwner: (owner) => feedbackOwnerSet.has(Number(owner)),
    isFeedbackOwner: (owner) => feedbackOwnerSet.has(Number(owner)),
    isOwnOwner: (owner) => relationship(owner) === "own" || feedbackOwnerSet.has(Number(owner)),
    isAllyOwner: (owner) => relationship(owner) === "ally",
  };
  for (const name of Object.values(FEEDBACK_ARRAY_TYPES)) {
    const method = `live${name[0].toUpperCase()}${name.slice(1)}`;
    view[method] = () => arrays[name];
  }
  return Object.freeze(view);
}

function mergedResources(staticResources, dynamicResources) {
  const byId = new Map();
  for (const resource of staticResources || []) byId.set(resource.id, resource);
  for (const resource of dynamicResources || []) byId.set(resource.id, resource);
  return [...byId.values()];
}

function buildStateFacade(frame, entities, rememberedBuildings, trenches, feedback, legacy, map) {
  const selection = new Set(entities.filter((entity) => entity.selected).map((entity) => entity.id));
  const relationship = relationshipResolver([...entities, ...rememberedBuildings]);
  const colors = new Map();
  for (const entity of [...entities, ...rememberedBuildings]) {
    if (Number.isInteger(Number(entity.owner)) && typeof entity.teamColor === "string") {
      colors.set(Number(entity.owner), entity.teamColor);
    }
  }
  const players = [...colors].map(([id, color]) => ({ id, color }));
  return {
    playerId: feedback.playerId,
    players,
    resources: { oil: legacy.oil },
    selection,
    rememberedBuildings,
    trenches,
    map,
    tick: frame.diagnosticsContext.sourceTick,
    _curById: legacy.currentById,
    _prevById: legacy.previousById,
    weaponRecoil: (id) => legacy.recoilById.get(id) ?? 0,
    weaponRecoilPhase: (id) => legacy.recoilPhaseById.get(id) ?? 0,
    weaponRecoilKind: (id) => legacy.recoilKindById.get(id),
    isFeedbackOwner: feedback.isFeedbackOwner,
    isOwnOwner: (owner) => relationship(owner) === "own" || feedback.isFeedbackOwner(owner),
    isAllyOwner: (owner) => relationship(owner) === "ally",
    isNeutralOwner: (owner) => Number(owner) === 0,
  };
}

function snapshotLegacyState(state, entities, now, profiler) {
  const currentById = new Map();
  const previousById = new Map();
  const recoilById = new Map();
  const recoilPhaseById = new Map();
  const recoilKindById = new Map();
  for (const entity of entities) {
    copyPose(currentById, entity.id, state?._curById?.get?.(entity.id));
    copyPose(previousById, entity.id, state?._prevById?.get?.(entity.id));
    try {
      if (typeof state?.weaponRecoil === "function") {
        recoilById.set(entity.id, finiteNumber(state.weaponRecoil(entity.id, entity.kind, now), 0));
      }
      if (typeof state?.weaponRecoilPhase === "function") {
        recoilPhaseById.set(entity.id, finiteNumber(state.weaponRecoilPhase(entity.id, entity.kind, now), 0));
      }
      if (typeof state?.weaponRecoilKind === "function") {
        const weaponKind = state.weaponRecoilKind(entity.id);
        if (weaponKind) recoilKindById.set(entity.id, weaponKind);
      }
    } catch {
      profiler?.recordDiagnosticCounter?.("pixiCompatibility.legacyRead.failed", 1);
    }
  }
  return {
    oil: Number.isFinite(state?.resources?.oil) ? state.resources.oil : null,
    currentById,
    previousById,
    recoilById,
    recoilPhaseById,
    recoilKindById,
  };
}

function copyPose(target, id, pose) {
  if (!pose || !Number.isFinite(pose.x) || !Number.isFinite(pose.y)) return;
  target.set(id, Object.freeze({ x: pose.x, y: pose.y }));
}

function relationshipResolver(entities) {
  const byOwner = new Map();
  for (const entity of entities) {
    const owner = Number(entity?.owner);
    if (Number.isInteger(owner) && !byOwner.has(owner)) byOwner.set(owner, entity.relationship);
  }
  return (owner) => byOwner.get(Number(owner)) || (Number(owner) === 0 ? "neutral" : "enemy");
}

function recordsOfType(records, type) {
  return Array.isArray(records) ? records.filter((record) => record?.type === type) : [];
}

function recordOfType(records, type) {
  return Array.isArray(records) ? records.find((record) => record?.type === type) : null;
}

function marqueeForFrame(records) {
  const rect = recordOfType(records, "marquee")?.rect;
  if (!rect) return null;
  return {
    x: finiteNumber(rect.x, 0),
    y: finiteNumber(rect.y, 0),
    w: finiteNumber(rect.w ?? rect.width, 0),
    h: finiteNumber(rect.h ?? rect.height, 0),
  };
}

function gridValue(grid, width, height, tx, ty) {
  if (!Number.isInteger(tx) || !Number.isInteger(ty) || tx < 0 || ty < 0 || tx >= width || ty >= height) return 0;
  return grid[ty * width + tx];
}

function finiteNumber(value, fallback) {
  return Number.isFinite(Number(value)) ? Number(value) : fallback;
}

function positiveNumber(value, fallback) {
  const number = finiteNumber(value, fallback);
  return number > 0 ? number : fallback;
}
