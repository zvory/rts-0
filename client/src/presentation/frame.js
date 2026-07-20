import { GridSnapshotCache } from "./grid_snapshot.js";
import { PRESENTATION_LAYER_IDS, createEmptyLayerRecords } from "./layers.js";
import {
  PRESENTATION_ENTITY_FIELDS,
  preparedPresentationEntityRecord,
} from "./entity_snapshot.js";

export const PRESENTATION_FRAME_VERSION = 1;
export const STATIC_MAP_PRESENTATION_VERSION = 1;

const FEEDBACK_ARRAYS = Object.freeze([
  ["commandFeedback", "command"],
  ["smokeCanisters", "smokeCanister"],
  ["mortarLaunches", "mortarLaunch"],
  ["mortarShells", "mortarShell"],
  ["mortarTargets", "mortarTarget"],
  ["mortarImpacts", "mortarImpact"],
  ["artilleryTargets", "artilleryTarget"],
  ["artilleryLaunches", "artilleryLaunch"],
  ["artilleryImpacts", "artilleryImpact"],
  ["panzerfaustShots", "panzerfaustShot"],
  ["panzerfaustImpacts", "panzerfaustImpact"],
  ["muzzleFlashes", "muzzleFlash"],
  ["missToasts", "missToast"],
]);

const FEEDBACK_SINGLETONS = Object.freeze([
  ["placement", "placement"],
  ["formationMovePreview", "formationMovePreview"],
  ["labToolPreview", "labToolPreview"],
  ["attackTargetPreview", "attackTargetPreview"],
  ["antiTankGunSetupPreview", "supportWeaponSetupPreview"],
  ["abilityTargetPreview", "abilityTargetPreview"],
  ["resourceMiningPreview", "resourceMiningPreview"],
]);

export class PresentationFrameAssembler {
  constructor({ map = null, generation = 1, entityStats = null } = {}) {
    this._generation = normalizeGeneration(generation);
    this._entityStats = entityStats || Object.freeze({});
    this._frameId = 0;
    this._assemblyCount = 0;
    this._staticRevision = 0;
    this._mapSource = null;
    this._staticMap = null;
    this._terrainCache = new GridSnapshotCache();
    this._visibleCache = new GridSnapshotCache();
    this._exploredCache = new GridSnapshotCache();
    if (map) this._replaceStaticMap(map);
  }

  get staticMap() {
    return this._staticMap;
  }

  reset({ map = null, generation = this._generation + 1 } = {}) {
    this._generation = normalizeGeneration(generation);
    this._frameId = 0;
    this._assemblyCount = 0;
    this._visibleCache.clear();
    this._exploredCache.clear();
    this._mapSource = null;
    this._staticMap = null;
    if (map) this._replaceStaticMap(map);
  }

  assemble({
    map,
    frameContext,
    projection,
    fog,
    feedback = null,
    rememberedBuildings = [],
    trenches = [],
    groundDecals = [],
    groundDecalRevision = 0,
    selectionIds = null,
    players = [],
    playerId = null,
    spectator = false,
    visualSamples = [],
    observerMapAnalysis = null,
    screenOverlay = null,
    visualTimeMs = 0,
    mode = "live",
    sourceTick = 0,
  }) {
    if (map && map !== this._mapSource) {
      if (this._mapSource) this.reset({ map, generation: this._generation + 1 });
      else this._replaceStaticMap(map);
    }
    if (!this._staticMap) throw new Error("PresentationFrameAssembler requires a static map.");

    const diagnostics = {
      droppedRecords: 0,
      droppedByCategory: {},
      assembledByLayer: Object.fromEntries(PRESENTATION_LAYER_IDS.map((id) => [id, 0])),
    };
    const layers = createEmptyLayerRecords();
    const context = buildViewerContext({ players, playerId, spectator });
    const selected = selectionIds instanceof Set ? selectionIds : new Set(selectionIds || []);
    const entities = Array.isArray(frameContext?.interpolatedEntities)
      ? frameContext.interpolatedEntities
      : [];

    for (let entityIndex = 0; entityIndex < entities.length; entityIndex += 1) {
      const entity = entities[entityIndex];
      const prepared = frameContext?.preparedEntities?.[entityIndex];
      const aboveFogReveal = entity?.shotReveal || entity?.aboveFogReveal;
      const layerId = aboveFogReveal
        ? "aboveFogReveal"
        : entity?.visionOnly
          ? "belowFogIntel"
          : "fogGatedWorld";
      safePush(layers, layerId, "entity", diagnostics, () =>
        entityRecord(entity, {
          prepared,
          selected: selected.has(entity?.id),
          context,
          tileSizePx: this._staticMap.tileSizePx,
          entityStats: this._entityStats,
          presentationKind: aboveFogReveal
            ? "shotRevealEntity"
            : entity?.visionOnly
              ? "intelEntity"
              : "entity",
        }));
    }
    for (const entity of rememberedBuildings || []) {
      safePush(layers, "rememberedWorld", "rememberedBuilding", diagnostics, () =>
        entityRecord(entity, {
          selected: false,
          context,
          tileSizePx: this._staticMap.tileSizePx,
          entityStats: this._entityStats,
          presentationKind: "rememberedBuilding",
        }));
    }
    pushTypedRecords(layers, "persistentGroundMark", "trench", trenches, diagnostics);
    pushTypedRecords(layers, "persistentGroundMark", "groundDecal", groundDecals, diagnostics);
    pushTypedRecords(layers, "persistentGroundMark", "visualSample", visualSamples, diagnostics);
    pushTypedRecords(layers, "fogGatedWorld", "smoke", feedback?.smokes, diagnostics);
    pushTypedRecords(layers, "fogGatedWorld", "abilityObject", feedback?.abilityObjects, diagnostics);

    for (const [field, type] of FEEDBACK_ARRAYS) {
      pushTypedRecords(layers, "tacticalFeedback", type, feedback?.[field], diagnostics);
    }
    for (const [field, type] of FEEDBACK_SINGLETONS) {
      if (feedback?.[field] != null) {
        safePush(layers, "tacticalFeedback", type, diagnostics, () =>
          field === "placement"
            ? placementRecord(feedback[field], this._entityStats, this._staticMap.tileSizePx)
            : detachedRecord({ type, ...feedback[field] }));
      }
    }
    if (feedback) {
      safePush(layers, "tacticalFeedback", "feedbackContext", diagnostics, () => detachedRecord({
        type: "feedbackContext",
        feedbackOwnerId: feedback.feedbackOwnerId,
        feedbackOwnerIds: feedback.feedbackOwnerIds,
        issueAsOwnerId: feedback.issueAsOwnerId,
        showUnitRangesEnabled: feedback.showUnitRangesEnabled,
        showSelectedFieldOfFireEnabled: feedback.showSelectedFieldOfFireEnabled,
        debugPathOverlaysEnabled: feedback.debugPathOverlaysEnabled,
        showAllDebugPathOverlays: feedback.showAllDebugPathOverlays,
      }));
    }
    if (observerMapAnalysis) {
      safePush(layers, "tacticalFeedback", "observerMapAnalysis", diagnostics, () =>
        detachedRecord({ type: "observerMapAnalysis", model: observerMapAnalysis }));
    }
    if (screenOverlay?.marquee) {
      safePush(layers, "screenOverlay", "marquee", diagnostics, () =>
        detachedRecord({ type: "marquee", rect: screenOverlay.marquee }));
    }

    const width = this._staticMap.terrain.width;
    const height = this._staticMap.terrain.height;
    const visible = this._visibleCache.snapshot({
      revision: fog?.visibleRevision ?? 0,
      width,
      height,
      source: fog?.visibleGrid || new Uint8Array(width * height),
    });
    const explored = this._exploredCache.snapshot({
      revision: fog?.exploredRevision ?? 0,
      width,
      height,
      source: fog?.exploredGrid || new Uint8Array(width * height),
    });
    safePush(layers, "currentFog", "fogMask", diagnostics, () => detachedRecord({
      type: "fogMask",
      width,
      height,
      visibleRevision: visible.revision,
      exploredRevision: explored.revision,
    }));

    const frozenLayers = Object.freeze(Object.fromEntries(
      PRESENTATION_LAYER_IDS.map((id) => [id, Object.freeze(layers[id])]),
    ));
    this._frameId += 1;
    this._assemblyCount += 1;
    const diagnosticsContext = detachedRecord({
      mode: mode === "fixedCapture" ? "fixedCapture" : "live",
      sourceTick: finiteNonNegativeInteger(sourceTick),
      interpolationAlpha: normalizeAlpha(frameContext?.alpha),
      assemblyOrdinal: this._assemblyCount,
      droppedRecords: diagnostics.droppedRecords,
      droppedByCategory: diagnostics.droppedByCategory,
      assembledByLayer: diagnostics.assembledByLayer,
    });
    return Object.freeze({
      version: PRESENTATION_FRAME_VERSION,
      generation: this._generation,
      frameId: this._frameId,
      groundDecalRevision: finiteNonNegativeInteger(groundDecalRevision),
      visualTimeMs: finiteNonNegativeNumber(visualTimeMs),
      projection: projectionRecord(projection),
      staticMapRevision: this._staticMap.revision,
      visible,
      explored,
      layers: frozenLayers,
      diagnosticsContext,
    });
  }

  _replaceStaticMap(map) {
    const width = finiteNonNegativeInteger(map?.width);
    const height = finiteNonNegativeInteger(map?.height);
    const tileSizePx = finitePositiveNumber(map?.tileSize);
    this._staticRevision += 1;
    this._terrainCache.clear();
    const terrain = this._terrainCache.snapshot({
      revision: this._staticRevision,
      width,
      height,
      source: map?.terrain || new Uint8Array(width * height),
    });
    const resourceSites = [];
    for (const resource of map?.resources || []) {
      try {
        resourceSites.push(detachedRecord({
          id: resource?.id,
          kind: resource?.kind,
          x: resource?.x,
          y: resource?.y,
        }));
      } catch {
        // Malformed static resource records are omitted; dynamic assembly diagnostics remain bounded.
      }
    }
    this._mapSource = map;
    this._staticMap = Object.freeze({
      version: STATIC_MAP_PRESENTATION_VERSION,
      generation: this._generation,
      revision: this._staticRevision,
      widthPx: width * tileSizePx,
      heightPx: height * tileSizePx,
      tileSizePx,
      terrain,
      resourceSites: Object.freeze(resourceSites),
    });
  }
}

function entityRecord(entity, {
  prepared = null,
  selected,
  context,
  tileSizePx,
  entityStats,
  presentationKind,
}) {
  if (!entity || (typeof entity.id !== "number" && typeof entity.id !== "string")) {
    throw new TypeError("Presented entity requires a stable id.");
  }
  if (typeof entity.kind !== "string" || !entity.kind) {
    throw new TypeError("Presented entity requires a kind.");
  }
  const x = finiteNumber(entity.x);
  const y = finiteNumber(entity.y);
  const owner = Number.isFinite(Number(entity.owner)) ? Number(entity.owner) : 0;
  const stat = entityStats?.[entity.kind] || null;
  const semanticHeight = entitySemanticHeight(stat, tileSizePx);
  const derived = {
    type: presentationKind,
    x,
    y,
    owner,
    relationship: relationshipForOwner(owner, context),
    teamColor: context.colors[owner] || "#9aa0a8",
    selected: !!selected,
    visualBounds: entityVisualBounds(stat, tileSizePx),
    anchors: {
      ground: { x, y, heightPx: 0 },
      selection: { x, y, heightPx: 0 },
      hp: { x, y, heightPx: semanticHeight },
    },
  };
  if (prepared?.source === entity) return preparedPresentationEntityRecord(prepared, derived);
  const record = { type: presentationKind };
  for (const field of PRESENTATION_ENTITY_FIELDS) {
    if (entity[field] !== undefined) record[field] = entity[field];
  }
  Object.assign(record, derived);
  return detachedRecord(record);
}

function entityVisualBounds(stat, tileSizePx) {
  const building = Number.isFinite(stat?.footW) && Number.isFinite(stat?.footH);
  const widthPx = building
    ? Math.max(8, stat.footW * tileSizePx)
    : Math.max(8, Number.isFinite(stat?.size) ? stat.size * 2 : 16);
  const depthPx = building
    ? Math.max(8, stat.footH * tileSizePx)
    : widthPx;
  return {
    class: building ? "building" : "unit",
    widthPx,
    depthPx,
    heightPx: entitySemanticHeight(stat, tileSizePx),
  };
}

function placementRecord(placement, entityStats, tileSizePx) {
  const stat = entityStats?.[placement?.building] || null;
  const footW = Number.isFinite(stat?.footW) ? Math.max(1, stat.footW) : 1;
  const footH = Number.isFinite(stat?.footH) ? Math.max(1, stat.footH) : 1;
  return detachedRecord({
    type: "placement",
    ...placement,
    footprint: { footW, footH, tileSizePx },
  });
}

function entitySemanticHeight(stat, tileSizePx) {
  if (Number.isFinite(stat?.footW) && Number.isFinite(stat?.footH)) {
    return Math.max(8, Math.max(stat.footW, stat.footH) * tileSizePx * 0.5);
  }
  return Math.max(8, Number.isFinite(stat?.size) ? stat.size : 0);
}

function buildViewerContext({ players, playerId, spectator }) {
  const teams = {};
  const colors = { 0: "#9aa0a8" };
  for (const player of players || []) {
    const id = Number(player?.id);
    if (!Number.isInteger(id) || id <= 0) continue;
    teams[id] = Number(player?.teamId) || 0;
    colors[id] = /^#[0-9a-fA-F]{6}$/.test(player?.color || "") ? player.color : "#9aa0a8";
  }
  return {
    playerId: Number.isInteger(Number(playerId)) ? Number(playerId) : null,
    spectator: !!spectator,
    teams,
    colors,
  };
}

function relationshipForOwner(owner, context) {
  if (owner === 0) return "neutral";
  if (context.spectator || context.playerId == null) return "observed";
  if (owner === context.playerId) return "own";
  const ownTeam = context.teams[context.playerId] || 0;
  const ownerTeam = context.teams[owner] || 0;
  return ownTeam !== 0 && ownTeam === ownerTeam ? "ally" : "enemy";
}

function pushTypedRecords(layers, layerId, type, records, diagnostics) {
  if (!Array.isArray(records)) return;
  for (const record of records) {
    safePush(layers, layerId, type, diagnostics, () => detachedRecord({ type, ...record }));
  }
}

function safePush(layers, layerId, category, diagnostics, build) {
  try {
    layers[layerId].push(build());
    diagnostics.assembledByLayer[layerId] += 1;
  } catch {
    diagnostics.droppedRecords += 1;
    diagnostics.droppedByCategory[category] = Math.min(
      9999,
      (diagnostics.droppedByCategory[category] || 0) + 1,
    );
  }
}

function projectionRecord(projection) {
  if (!projection || projection.version !== 1) throw new TypeError("Presentation frame requires ProjectionSnapshotV1.");
  const queryNames = [
    "project", "groundAtScreen", "projectedExtent", "viewportGroundPolygon",
    "viewportGroundBounds", "containsProjected", "snapshot", "audioListener",
  ];
  const out = {
    version: 1,
    camera: detachedRecord(projection.camera),
    viewport: detachedRecord(projection.viewport),
    mapBounds: projection.mapBounds == null ? null : detachedRecord(projection.mapBounds),
  };
  if (projection.perspective != null) out.perspective = detachedRecord(projection.perspective);
  for (const name of queryNames) {
    if (typeof projection[name] !== "function") throw new TypeError(`ProjectionSnapshotV1 is missing ${name}.`);
    out[name] = projection[name];
  }
  return Object.freeze(out);
}

export function detachedRecord(value) {
  return detach(value, new Set(), 0);
}

function detach(value, seen, depth) {
  if (value == null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number") return finiteNumber(value);
  if (typeof value !== "object") throw new TypeError("Presentation records may contain only plain data.");
  if (depth > 16 || seen.has(value)) throw new TypeError("Presentation record is too deep or cyclic.");
  if (ArrayBuffer.isView(value) || value instanceof Map || value instanceof Set) {
    throw new TypeError("Presentation records cannot expose mutable collection views.");
  }
  const prototype = Object.getPrototypeOf(value);
  if (!Array.isArray(value) && prototype !== Object.prototype && prototype !== null) {
    throw new TypeError("Presentation records must use plain objects.");
  }
  seen.add(value);
  let out;
  if (Array.isArray(value)) {
    out = value.map((entry) => detach(entry, seen, depth + 1));
  } else {
    out = {};
    for (const [key, entry] of Object.entries(value)) {
      if (entry !== undefined) out[key] = detach(entry, seen, depth + 1);
    }
  }
  seen.delete(value);
  return Object.freeze(out);
}

function normalizeGeneration(value) {
  const generation = Number(value);
  if (!Number.isInteger(generation) || generation <= 0) throw new RangeError("Generation must be a positive integer.");
  return generation;
}

function normalizeAlpha(value) {
  const alpha = Number(value);
  if (!Number.isFinite(alpha)) return 1;
  return Math.max(0, Math.min(1, alpha));
}

function finiteNumber(value) {
  const number = Number(value);
  if (!Number.isFinite(number)) throw new TypeError("Presentation number must be finite.");
  return number;
}

function finiteNonNegativeNumber(value) {
  const number = finiteNumber(value);
  if (number < 0) throw new RangeError("Presentation number must be non-negative.");
  return number;
}

function finitePositiveNumber(value) {
  const number = finiteNumber(value);
  if (number <= 0) throw new RangeError("Presentation number must be positive.");
  return number;
}

function finiteNonNegativeInteger(value) {
  const number = finiteNumber(value);
  if (!Number.isInteger(number) || number < 0) throw new RangeError("Presentation value must be a non-negative integer.");
  return number;
}
