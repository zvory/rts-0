import {
  defaultMapAuthoringLayerVisibility,
  normalizeMapAuthoringLayerVisibility,
  validateMapAuthoringLayerVisibility,
} from "./map_authoring/layers.js";

export const MAP_EDITOR_PRESENTATION_VERSION = 3;

export function createMapEditorPresentation({
  generation = 1,
  frameId,
  camera,
  terrainUpdate = null,
  doodadUpdate = null,
  overlay = null,
  layerVisibility = defaultMapAuthoringLayerVisibility(),
  visualTimeMs = 0,
}) {
  const record = {
    version: MAP_EDITOR_PRESENTATION_VERSION,
    generation: positiveInteger(generation, "generation"),
    frameId: positiveInteger(frameId, "frameId"),
    camera: plain(camera),
    terrainUpdate: terrainUpdate == null ? null : plain(terrainUpdate),
    doodadUpdate: doodadUpdate == null ? null : plain(doodadUpdate),
    overlay: overlay == null ? null : plain(overlay),
    layerVisibility: normalizeMapAuthoringLayerVisibility(layerVisibility),
    visualTimeMs: finiteNonNegative(visualTimeMs, "visualTimeMs"),
  };
  validateMapEditorPresentation(record);
  return Object.freeze(record);
}

export function validateMapEditorPresentation(record) {
  if (record?.version !== MAP_EDITOR_PRESENTATION_VERSION) throw new RangeError("Map Editor presentation version is unsupported");
  positiveInteger(record.generation, "generation");
  positiveInteger(record.frameId, "frameId");
  for (const [name, value] of Object.entries(record.camera || {})) {
    if (!["x", "y", "zoom"].includes(name) || !Number.isFinite(value)) throw new TypeError("Map Editor camera must be finite plain data");
  }
  if (!(record.camera?.zoom > 0)) throw new RangeError("Map Editor camera zoom must be positive");
  finiteNonNegative(record.visualTimeMs, "visualTimeMs");
  validateMapAuthoringLayerVisibility(record.layerVisibility);
  const update = record.terrainUpdate;
  if (update) {
    positiveInteger(update.revision, "terrain revision");
    if (update.kind === "replace") {
      positiveInteger(update.width, "terrain width");
      positiveInteger(update.height, "terrain height");
      positiveInteger(update.tileSize, "terrain tileSize");
      if (!Array.isArray(update.terrain) || update.terrain.length !== update.width * update.height) {
        throw new RangeError("Map Editor replacement terrain shape does not match its payload");
      }
      if (!Array.isArray(update.elevation) || update.elevation.length !== update.width * update.height
        || update.elevation.some((level) => !Number.isInteger(level) || level < 0 || level > 9)) {
        throw new RangeError("Map Editor replacement elevation shape does not match its payload");
      }
      validateSun(update.sun);
    } else if (update.kind === "patch") {
      if (!Array.isArray(update.changes)) throw new TypeError("Map Editor terrain patch requires changes");
    } else throw new TypeError("Map Editor terrain update kind is unsupported");
  }
  const doodadUpdate = record.doodadUpdate;
  if (doodadUpdate) {
    positiveInteger(doodadUpdate.revision, "doodad revision");
    if (doodadUpdate.kind === "replace") {
      validateDoodads(doodadUpdate.doodads, "replacement doodads");
    } else if (doodadUpdate.kind === "patch") {
      validateDoodads(doodadUpdate.upserts, "doodad upserts");
      if (!Array.isArray(doodadUpdate.removedIds)) throw new TypeError("Map Editor doodad patch requires removedIds");
      const removed = new Set();
      for (const id of doodadUpdate.removedIds) {
        positiveInteger(id, "removed doodad id");
        if (removed.has(id)) throw new RangeError("Map Editor doodad patch has duplicate removed ids");
        removed.add(id);
      }
    } else throw new TypeError("Map Editor doodad update kind is unsupported");
  }
  structuredClone(record);
  return record;
}

function validateSun(sun) {
  if (sun == null) return;
  if (typeof sun !== "object" || Array.isArray(sun)
    || !Number.isInteger(sun.azimuthDegrees) || sun.azimuthDegrees < 0 || sun.azimuthDegrees > 359
    || !Number.isInteger(sun.elevationDegrees) || sun.elevationDegrees < 1 || sun.elevationDegrees > 89
    || !Number.isInteger(sun.warmth) || sun.warmth < 0 || sun.warmth > 100) {
    throw new RangeError("Map Editor sun requires direction 0-359, height 1-89, and warmth 0-100");
  }
}

function validateDoodads(records, label) {
  if (!Array.isArray(records)) throw new TypeError(`Map Editor ${label} must be an array`);
  const ids = new Set();
  for (const record of records) {
    positiveInteger(record?.id, "doodad id");
    if (ids.has(record.id)) throw new RangeError(`Map Editor ${label} has duplicate ids`);
    ids.add(record.id);
    if (typeof record.typeId !== "string" || !record.typeId) throw new TypeError("Map Editor doodad typeId is required");
    if (!Number.isInteger(record.x) || !Number.isInteger(record.y)) throw new TypeError("Map Editor doodad coordinates must be integers");
    if (record.color !== undefined && !/^#[0-9a-f]{6}$/.test(record.color)) {
      throw new TypeError("Map Editor doodad color must be canonical lowercase hex");
    }
  }
}

function plain(value) {
  return structuredClone(value);
}

function positiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value <= 0) throw new RangeError(`${label} must be a positive integer`);
  return value;
}

function finiteNonNegative(value, label) {
  if (!Number.isFinite(value) || value < 0) throw new RangeError(`${label} must be finite and non-negative`);
  return value;
}
