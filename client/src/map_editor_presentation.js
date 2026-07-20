export const MAP_EDITOR_PRESENTATION_VERSION = 1;

export function createMapEditorPresentation({
  generation = 1,
  frameId,
  camera,
  terrainUpdate = null,
  overlay = null,
}) {
  const record = {
    version: MAP_EDITOR_PRESENTATION_VERSION,
    generation: positiveInteger(generation, "generation"),
    frameId: positiveInteger(frameId, "frameId"),
    camera: plain(camera),
    terrainUpdate: terrainUpdate == null ? null : plain(terrainUpdate),
    overlay: overlay == null ? null : plain(overlay),
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
    } else if (update.kind === "patch") {
      if (!Array.isArray(update.changes)) throw new TypeError("Map Editor terrain patch requires changes");
    } else throw new TypeError("Map Editor terrain update kind is unsupported");
  }
  structuredClone(record);
  return record;
}

function plain(value) {
  return structuredClone(value);
}

function positiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value <= 0) throw new RangeError(`${label} must be a positive integer`);
  return value;
}
