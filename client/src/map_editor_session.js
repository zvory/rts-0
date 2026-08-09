import { PASSABLE, TERRAIN } from "./protocol.js";
import {
  createMapEditorDoodadRecords,
  createMapEditorDoodads,
  MAP_EDITOR_MAX_DOODADS,
  normalizeMapEditorDoodads,
  removeMapEditorDoodads,
} from "./map_editor_doodads.js";
import { applyMapOperation, transformRoadCharacter } from "./map_authoring/operations.js";
import { rectTiles } from "./map_authoring/geometry.js";
import {
  editForestSpans,
  forestOwnedDoodadIds,
  forestSpansFromTiles,
  forestTilesFromSpans,
  generatedForestDoodads,
  normalizeForestSpans,
} from "./map_authoring/forests.js";
import {
  AUTHORED_MAP_MAX_BASE_SITES,
  AUTHORED_MAP_MAX_DIMENSION_TILES,
  AUTHORED_MAP_MAX_OIL_PATCHES,
  AUTHORED_MAP_MAX_START_LOCATIONS,
  AUTHORED_MAP_MAX_STEEL_PATCHES,
  AUTHORED_MAP_MIN_DIMENSION_TILES,
  boundedAuthoredPatchCount,
} from "./map_authoring/limits.js";
import { CURRENT_AUTHORED_MAP_VERSION } from "./map_authoring/schema.js";
import {
  expandSymmetricPoints,
  MAP_AUTHORING_SYMMETRY,
  symmetrySupported,
  symmetryTransforms as authoringSymmetryTransforms,
  transformPoint as transformAuthoringPoint,
} from "./map_authoring/symmetry.js";

export { MAP_EDITOR_MAX_DOODADS } from "./map_editor_doodads.js";

export const MAP_EDITOR_HISTORY_LIMIT = 25;
export const MAP_EDITOR_MAX_START_LOCATIONS = AUTHORED_MAP_MAX_START_LOCATIONS;
export const MAP_EDITOR_MAX_BASE_SITES = AUTHORED_MAP_MAX_BASE_SITES;
export const MAP_EDITOR_MAX_STEEL_PATCHES = AUTHORED_MAP_MAX_STEEL_PATCHES;
export const MAP_EDITOR_MAX_OIL_PATCHES = AUTHORED_MAP_MAX_OIL_PATCHES;
export const MAP_EDITOR_DEFAULT_STEEL_PATCHES = 12;
export const MAP_EDITOR_DEFAULT_OIL_PATCHES = 3;
export const MAP_EDITOR_DEFAULT_SIZE = 126;
export const MAP_EDITOR_MIN_SIZE = AUTHORED_MAP_MIN_DIMENSION_TILES;
export const MAP_EDITOR_MAX_SIZE = AUTHORED_MAP_MAX_DIMENSION_TILES;
// Mirror the authored-map clearance contract enforced by the simulation.
export const MAP_EDITOR_MAIN_CLEARANCE_TILES = 7;
export const MAP_EDITOR_BASE_SITE_CLEARANCE_TILES = 4;
export const MAP_EDITOR_SYMMETRY = MAP_AUTHORING_SYMMETRY;
export const MAP_EDITOR_MAP_VERSION = CURRENT_AUTHORED_MAP_VERSION;
export const MAP_EDITOR_DEFAULT_SUN = Object.freeze({
  azimuthDegrees: 315,
  elevationDegrees: 35,
  warmth: 25,
});

const TERRAIN_TO_CHAR = Object.freeze({
  [TERRAIN.GRASS]: ".",
  [TERRAIN.ROCK]: "#",
  [TERRAIN.WATER]: "~",
  [TERRAIN.ROAD_BARE]: "=",
  [TERRAIN.ROAD_HORIZONTAL]: "-",
  [TERRAIN.ROAD_VERTICAL]: "|",
  [TERRAIN.ROAD_DIAGONAL_NW_SE]: "\\",
  [TERRAIN.ROAD_DIAGONAL_NE_SW]: "/",
  [TERRAIN.GRAVEL_A]: "0",
  [TERRAIN.GRAVEL_B]: "1",
  [TERRAIN.GRAVEL_C]: "2",
  [TERRAIN.DIRT_A]: "3",
  [TERRAIN.DIRT_B]: "4",
  [TERRAIN.DIRT_C]: "5",
  [TERRAIN.MUD_A]: "6",
  [TERRAIN.MUD_B]: "7",
  [TERRAIN.MUD_C]: "8",
  [TERRAIN.FROSTED_GROUND]: "9",
});
const CHAR_TO_TERRAIN = Object.freeze({
  ".": TERRAIN.GRASS,
  "#": TERRAIN.ROCK,
  "~": TERRAIN.WATER,
  "=": TERRAIN.ROAD_BARE,
  "-": TERRAIN.ROAD_HORIZONTAL,
  "|": TERRAIN.ROAD_VERTICAL,
  "\\": TERRAIN.ROAD_DIAGONAL_NW_SE,
  "/": TERRAIN.ROAD_DIAGONAL_NE_SW,
  "0": TERRAIN.GRAVEL_A,
  "1": TERRAIN.GRAVEL_B,
  "2": TERRAIN.GRAVEL_C,
  "3": TERRAIN.DIRT_A,
  "4": TERRAIN.DIRT_B,
  "5": TERRAIN.DIRT_C,
  "6": TERRAIN.MUD_A,
  "7": TERRAIN.MUD_B,
  "8": TERRAIN.MUD_C,
  "9": TERRAIN.FROSTED_GROUND,
});

export class MapEditorSession {
  constructor({ historyLimit = MAP_EDITOR_HISTORY_LIMIT } = {}) {
    this.historyLimit = Math.max(1, Math.trunc(historyLimit) || MAP_EDITOR_HISTORY_LIMIT);
    this.draft = null;
    this.undoStack = [];
    this.redoStack = [];
    this.subscribers = new Set();
    this.desiredTool = null;
    this.lastAction = "";
    this.savedFingerprint = "";
    this.terrainStroke = null;
    this.elevationStroke = null;
    this.overlayStroke = null;
    this.doodadStroke = null;
  }

  get initialized() { return !!this.draft; }

  initializeFromStart(startPayload, { name = "Map" } = {}) {
    if (this.draft) return false;
    const map = startPayload?.map || {};
    const width = Number(map.width);
    const height = Number(map.height);
    if (!Number.isInteger(width) || width <= 0 || !Number.isInteger(height) || height <= 0) return false;
    this.draft = authoredMapFromMaterialized({
      name,
      description: "Map imported from an authoritative session.",
      width,
      height,
      terrain: map.terrain,
      elevation: map.elevation,
      sun: map.sun,
      starts: (startPayload?.players || []).map((player) => ({ x: Number(player.startTileX), y: Number(player.startTileY) })),
      baseSites: [],
      doodads: map.doodads,
      concealmentTiles: map.concealmentTiles,
      noVehicleTiles: map.noVehicleTiles,
      noBuildingTiles: map.noBuildingTiles,
      noEntrenchmentTiles: map.noEntrenchmentTiles,
      damageReductionTiles: map.damageReductionTiles,
      slowMovementTiles: map.slowMovementTiles,
    });
    this.markSaved({ notify: false });
    this.notify("initialized");
    return true;
  }

  initializeFromScenario(scenario, { force = false } = {}) {
    if (this.draft && !force) return false;
    const data = scenario?.map?.data;
    if (!data) return false;
    this.draft = authoredMapFromMaterialized({
      name: scenario?.map?.name || scenario?.name || "Map",
      description: "Map imported from Lab.",
      width: data.width ?? data.size,
      height: data.height ?? data.size,
      terrain: data.terrain,
      elevation: data.elevation,
      sun: data.sun,
      starts: data.starts,
      baseSites: data.baseSites || data.expansionSites,
      doodads: data.doodads,
      concealmentTiles: data.concealmentTiles,
      noVehicleTiles: data.noVehicleTiles,
      noBuildingTiles: data.noBuildingTiles,
      noEntrenchmentTiles: data.noEntrenchmentTiles,
      damageReductionTiles: data.damageReductionTiles,
      slowMovementTiles: data.slowMovementTiles,
    });
    this.undoStack = [];
    this.redoStack = [];
    this.markSaved({ notify: false });
    this.notify("initialized");
    return true;
  }

  initializeBlank({ size, width = size ?? MAP_EDITOR_DEFAULT_SIZE, height = size ?? MAP_EDITOR_DEFAULT_SIZE, playerCount = 2, name = "Untitled map" } = {}) {
    const mapWidth = boundedMapDimension(width);
    const mapHeight = boundedMapDimension(height);
    const count = Math.max(1, Math.min(MAP_EDITOR_MAX_START_LOCATIONS, Math.trunc(Number(playerCount)) || 2));
    const startTile = (dimension, fraction) => Math.max(
      MAP_EDITOR_MAIN_CLEARANCE_TILES,
      Math.min(dimension - MAP_EDITOR_MAIN_CLEARANCE_TILES - 1, Math.floor(dimension * fraction)),
    );
    const starts = [
      { x: startTile(mapWidth, 0.25), y: startTile(mapHeight, 0.25) },
      { x: startTile(mapWidth, 0.75), y: startTile(mapHeight, 0.75) },
      { x: startTile(mapWidth, 0.75), y: startTile(mapHeight, 0.25) },
      { x: startTile(mapWidth, 0.25), y: startTile(mapHeight, 0.75) },
    ].slice(0, count);
    this.draft = authoredMapFromMaterialized({
      name,
      description: "",
      width: mapWidth,
      height: mapHeight,
      terrain: Array(mapWidth * mapHeight).fill(TERRAIN.GRASS),
      starts,
      baseSites: starts,
      doodads: [],
      concealmentTiles: [],
      noVehicleTiles: [],
      noBuildingTiles: [],
      noEntrenchmentTiles: [],
      damageReductionTiles: [],
      slowMovementTiles: [],
    });
    this.undoStack = [];
    this.redoStack = [];
    this.markSaved({ notify: false });
    this.lastAction = "Created blank map";
    this.notify("loaded");
    return true;
  }

  loadAuthoredMap(source, { expectedSize = null, expectedWidth = expectedSize, expectedHeight = expectedSize, playerCount = null } = {}) {
    const draft = clone(source);
    normalizeDraft(draft);
    if (!validEditorDimension(draft.width) || !validEditorDimension(draft.height)) {
      throw new Error(`Map dimensions must be whole numbers from ${MAP_EDITOR_MIN_SIZE} to ${MAP_EDITOR_MAX_SIZE}.`);
    }
    const requiredWidth = positiveInteger(expectedWidth);
    const requiredHeight = positiveInteger(expectedHeight);
    if ((requiredWidth && draft.width !== requiredWidth) || (requiredHeight && draft.height !== requiredHeight)) {
      throw new Error(`This session uses a ${requiredWidth || draft.width} × ${requiredHeight || draft.height} map; ${draft.name} is ${draft.width} × ${draft.height}.`);
    }
    const requiredPlayers = positiveInteger(playerCount);
    if (requiredPlayers && draft.startLocations.length !== requiredPlayers) {
      throw new Error(`${draft.name} has ${draft.startLocations.length} start locations, not ${requiredPlayers}.`);
    }
    this.draft = draft;
    this.undoStack = [];
    this.redoStack = [];
    this.lastAction = `Loaded ${draft.name}`;
    this.markSaved({ notify: false });
    this.notify("loaded");
    return true;
  }

  resize({ width, height } = {}) {
    if (!this.draft) return draftEditError("Map is not initialized.");
    const mapWidth = positiveInteger(width);
    const mapHeight = positiveInteger(height);
    if (!validEditorDimension(mapWidth) || !validEditorDimension(mapHeight)) {
      return draftEditError(`Map dimensions must be whole numbers from ${MAP_EDITOR_MIN_SIZE} to ${MAP_EDITOR_MAX_SIZE}.`);
    }
    if (mapWidth === this.draft.width && mapHeight === this.draft.height) return { ok: true, count: 0 };
    const resized = resizeDraftCentered(this.draft, mapWidth, mapHeight);
    if (!resized.ok) return resized;
    const before = clone(this.draft);
    this.draft = resized.draft;
    this.undoStack.push(before);
    if (this.undoStack.length > this.historyLimit) this.undoStack.shift();
    this.redoStack = [];
    this.lastAction = `Resized map to ${mapWidth} × ${mapHeight}`;
    this.notify("loaded");
    return { ok: true, count: 1 };
  }

  subscribe(handler) {
    this.subscribers.add(handler);
    handler(this.snapshot());
    return () => this.subscribers.delete(handler);
  }

  snapshot() {
    return {
      draft: this.draft,
      canUndo: this.undoStack.length > 0,
      canRedo: this.redoStack.length > 0,
      undoDepth: this.undoStack.length,
      redoDepth: this.redoStack.length,
      desiredTool: this.desiredTool,
      lastAction: this.lastAction,
      hasUnsavedChanges: this.hasUnsavedChanges,
    };
  }

  mutate(label, mutation) {
    if (!this.draft || typeof mutation !== "function") return false;
    const before = clone(this.draft);
    const next = clone(this.draft);
    mutation(next);
    normalizeDraft(next);
    if (JSON.stringify(before) === JSON.stringify(next)) return false;
    this.undoStack.push(before);
    if (this.undoStack.length > this.historyLimit) this.undoStack.shift();
    this.redoStack = [];
    this.draft = next;
    this.lastAction = String(label || "Edited map");
    this.notify("changed");
    return true;
  }

  undo() {
    const previous = this.undoStack.pop();
    if (!previous || !this.draft) return false;
    this.redoStack.push(clone(this.draft));
    if (this.redoStack.length > this.historyLimit) this.redoStack.shift();
    this.draft = previous;
    this.lastAction = "Undo";
    this.notify("undo");
    return true;
  }

  redo() {
    const next = this.redoStack.pop();
    if (!next || !this.draft) return false;
    this.undoStack.push(clone(this.draft));
    if (this.undoStack.length > this.historyLimit) this.undoStack.shift();
    this.draft = next;
    this.lastAction = "Redo";
    this.notify("redo");
    return true;
  }

  setDesiredTool(tool) { this.desiredTool = tool ? clone(tool) : null; this.notify("tool"); }
  get hasUnsavedChanges() { return !!this.draft && draftFingerprint(this.draft) !== this.savedFingerprint; }

  markSaved({ notify = true, draft = this.draft } = {}) {
    if (!draft) return false;
    this.savedFingerprint = draftFingerprint(draft);
    if (notify) this.notify("saved");
    return true;
  }

  beginTerrainStroke(label = "Painted terrain") {
    if (!this.draft || this.terrainStroke) return false;
    this.terrainStroke = { label, before: clone(this.draft), dirty: new Map() };
    return true;
  }

  paintTerrainTiles(tiles, terrainCode) {
    if (!this.draft || !this.terrainStroke || !Array.isArray(tiles)) return [];
    const source = tiles.map((tile) => ({
      ...tile,
      character: TERRAIN_TO_CHAR[tile?.paintTerrainCode ?? terrainCode],
    })).filter((tile) => tile.character);
    const { terrainPatch } = applyMapOperation(this.draft, {
      type: "paintTiles",
      tiles: source,
      expandSymmetry: false,
    }, {
      protectedTerrain: ({ x, y }) => protectedTerrainTile(this.draft, x, y),
    });
    const changed = terrainPatch.map(({ x, y, character }) => ({ x, y, code: CHAR_TO_TERRAIN[character] }));
    for (const change of changed) {
      this.terrainStroke.dirty.set(`${change.x},${change.y}`, change);
    }
    return changed;
  }

  paintRoad(from, to, width = 5, symmetry = MAP_EDITOR_SYMMETRY.NONE) {
    if (!this.draft || !this.terrainStroke || !from || !to) return [];
    const { terrainPatch } = applyMapOperation(this.draft, {
      type: "road",
      points: [[from.x, from.y], [to.x, to.y]],
      width: Math.max(1, Math.min(15, Math.trunc(Number(width)) || 5)),
      roughness: 0,
      symmetry,
    }, {
      protectedTerrain: ({ x, y }) => protectedTerrainTile(this.draft, x, y),
    });
    const changed = terrainPatch.map(({ x, y, character }) => ({ x, y, code: CHAR_TO_TERRAIN[character] }));
    for (const change of changed) this.terrainStroke.dirty.set(`${change.x},${change.y}`, change);
    return changed;
  }

  commitTerrainStroke() {
    const stroke = this.terrainStroke;
    this.terrainStroke = null;
    if (!stroke || stroke.dirty.size === 0) return false;
    normalizeDraft(this.draft);
    this.undoStack.push(stroke.before);
    if (this.undoStack.length > this.historyLimit) this.undoStack.shift();
    this.redoStack = [];
    this.lastAction = stroke.label;
    this.notify("terrainStroke", { dirtyTiles: [...stroke.dirty.values()] });
    return true;
  }

  cancelTerrainStroke() {
    const stroke = this.terrainStroke;
    this.terrainStroke = null;
    if (!stroke) return false;
    this.draft = stroke.before;
    this.notify("changed");
    return true;
  }

  beginElevationStroke(label = "Painted elevation") {
    if (!this.draft || this.elevationStroke) return false;
    this.elevationStroke = { label, before: clone(this.draft), dirty: new Map() };
    return true;
  }

  paintElevationTiles(tiles, elevationLevel) {
    if (!this.draft || !this.elevationStroke || !Array.isArray(tiles)) return [];
    const level = Math.max(0, Math.min(9, Math.trunc(Number(elevationLevel)) || 0));
    const width = this.draft.width;
    const height = this.draft.height;
    if (!this.draft.elevation.length && level === 0) return [];
    if (!this.draft.elevation.length) this.draft.elevation = Array.from({ length: height }, () => "0".repeat(width));
    const byRow = new Map();
    const changed = [];
    for (const candidate of tiles) {
      const x = Math.trunc(Number(candidate?.x));
      const y = Math.trunc(Number(candidate?.y));
      if (x < 0 || y < 0 || x >= width || y >= height) continue;
      const row = byRow.get(y) || [...this.draft.elevation[y]];
      if (Number(row[x]) === level) continue;
      row[x] = String(level);
      byRow.set(y, row);
      const change = { x, y, level };
      changed.push(change);
      this.elevationStroke.dirty.set(`${x},${y}`, change);
    }
    for (const [y, row] of byRow) this.draft.elevation[y] = row.join("");
    if (changed.length && !this.draft.sun) this.draft.sun = { ...MAP_EDITOR_DEFAULT_SUN };
    return changed;
  }

  commitElevationStroke() {
    const stroke = this.elevationStroke;
    this.elevationStroke = null;
    if (!stroke || stroke.dirty.size === 0) return false;
    normalizeDraft(this.draft);
    this.undoStack.push(stroke.before);
    if (this.undoStack.length > this.historyLimit) this.undoStack.shift();
    this.redoStack = [];
    this.lastAction = stroke.label;
    this.notify("elevationStroke", { dirtyTiles: [...stroke.dirty.values()] });
    return true;
  }

  cancelElevationStroke() {
    const stroke = this.elevationStroke;
    this.elevationStroke = null;
    if (!stroke) return false;
    this.draft = stroke.before;
    this.notify("changed");
    return true;
  }

  beginOverlayStroke(label = "Painted map overlay") {
    if (!this.draft || this.overlayStroke) return false;
    this.overlayStroke = { label, before: clone(this.draft), dirty: new Map(), strippedForestDoodads: false };
    return true;
  }

  paintOverlayTiles(tiles, edit = {}) {
    if (!this.draft || !this.overlayStroke || !Array.isArray(tiles)) return [];
    const { overlayPatch } = applyMapOperation(this.draft, {
      type: "overlayTiles",
      tiles,
      edit,
      expandSymmetry: false,
    });
    for (const tile of overlayPatch) this.overlayStroke.dirty.set(locationKey(tile), tile);
    return overlayPatch;
  }

  paintForestTiles(tiles, paint = true) {
    if (!this.draft || !this.overlayStroke || !Array.isArray(tiles)) return [];
    const result = editForestSpans(this.draft.forestSpans, tiles, paint, this.draft);
    if (!result.changed.length) return [];
    if (!this.overlayStroke.strippedForestDoodads) {
      const ownedIds = forestOwnedDoodadIds(this.draft.doodads, this.draft, {
        maxDoodads: MAP_EDITOR_MAX_DOODADS,
      });
      this.draft.doodads = this.draft.doodads.filter((doodad) => !ownedIds.has(Number(doodad.id)));
      this.overlayStroke.strippedForestDoodads = true;
    }
    this.draft.forestSpans = result.spans;
    for (const tile of result.changed) this.overlayStroke.dirty.set(locationKey(tile), tile);
    return result.changed;
  }

  forestTiles() {
    return this.draft ? forestTilesFromSpans(this.draft.forestSpans, this.draft) : [];
  }

  commitOverlayStroke() {
    const stroke = this.overlayStroke;
    this.overlayStroke = null;
    if (!stroke || stroke.dirty.size === 0) return false;
    normalizeDraft(this.draft);
    this.undoStack.push(stroke.before);
    if (this.undoStack.length > this.historyLimit) this.undoStack.shift();
    this.redoStack = [];
    this.lastAction = stroke.label;
    this.notify("overlayStroke", { dirtyTiles: [...stroke.dirty.values()] });
    return true;
  }

  cancelOverlayStroke() {
    const stroke = this.overlayStroke;
    this.overlayStroke = null;
    if (!stroke) return false;
    this.draft = stroke.before;
    this.notify("changed");
    return true;
  }

  beginDoodadStroke(label = "Edited doodads") {
    if (!this.draft || this.doodadStroke) return false;
    this.doodadStroke = {
      label,
      before: clone(this.draft),
      upserts: new Map(),
      removedIds: new Set(),
    };
    return true;
  }

  placeDoodads(placements, options) {
    if (!this.draft || !this.doodadStroke) return [];
    const bounded = (placements || []).filter((point) => draftContainsWorldPoint(this.draft, point));
    const added = createMapEditorDoodads(this.draft, bounded, options);
    this.recordDoodadUpserts(added);
    return added;
  }

  placeDoodadRecords(placements) {
    if (!this.draft || !this.doodadStroke) return [];
    const bounded = (placements || []).filter((point) => draftContainsWorldPoint(this.draft, point));
    const added = createMapEditorDoodadRecords(this.draft, bounded);
    this.recordDoodadUpserts(added);
    return added;
  }

  recordDoodadUpserts(added) {
    for (const record of added) {
      this.doodadStroke.removedIds.delete(record.id);
      this.doodadStroke.upserts.set(record.id, clone(record));
    }
  }

  removeDoodads(ids) {
    if (!this.draft || !this.doodadStroke) return [];
    const removed = removeMapEditorDoodads(this.draft, ids);
    for (const id of removed) {
      this.doodadStroke.upserts.delete(id);
      this.doodadStroke.removedIds.add(id);
    }
    return removed;
  }

  commitDoodadStroke() {
    const stroke = this.doodadStroke;
    this.doodadStroke = null;
    if (!stroke) return false;
    normalizeDraft(this.draft);
    if (JSON.stringify(stroke.before) === JSON.stringify(this.draft)) return false;
    this.undoStack.push(stroke.before);
    if (this.undoStack.length > this.historyLimit) this.undoStack.shift();
    this.redoStack = [];
    this.lastAction = stroke.label;
    this.notify("doodadStroke", {
      doodadPatch: {
        upserts: [...stroke.upserts.values()].sort((left, right) => left.id - right.id),
        removedIds: [...stroke.removedIds].sort((left, right) => left - right),
      },
    });
    return true;
  }

  cancelDoodadStroke() {
    const stroke = this.doodadStroke;
    this.doodadStroke = null;
    if (!stroke) return false;
    this.draft = stroke.before;
    this.notify("changed");
    return true;
  }

  mapOverlay() {
    if (!this.draft) return null;
    const starts = this.draft.startLocations.map((location, index) => ({ ...location, index }));
    const startKeys = new Set(starts.map(locationKey));
    const bases = this.draft.baseSites
      .map((location, index) => ({ ...location, index }))
      .filter((site) => !startKeys.has(locationKey(site)));
    return { starts, bases };
  }

  materialized() {
    if (!this.draft) throw new Error("Map is not initialized.");
    const draft = clone(this.draft);
    normalizeDraft(draft);
    const forestTiles = forestTilesFromSpans(draft.forestSpans, draft);
    return {
      name: draft.name,
      width: draft.width,
      height: draft.height,
      terrain: draft.terrain.flatMap((row) => [...row].map((ch) => CHAR_TO_TERRAIN[ch])),
      elevation: draft.elevation?.length
        ? draft.elevation.flatMap((row) => [...row].map(Number))
        : new Array(draft.width * draft.height).fill(0),
      sun: draft.sun ? { ...draft.sun } : null,
      starts: draft.startLocations.map(copyLocation),
      baseSites: draft.baseSites.map(copyBaseSite),
      doodads: draft.doodads.map(copyDoodad),
      concealmentTiles: mergedOverlayTiles(draft.concealmentTiles, forestTiles),
      noVehicleTiles: mergedOverlayTiles(draft.noVehicleTiles, forestTiles),
      noBuildingTiles: mergedOverlayTiles(draft.noBuildingTiles, forestTiles),
      noEntrenchmentTiles: normalizeOverlayTiles(draft.noEntrenchmentTiles, draft),
      damageReductionTiles: mergedOverlayTiles(draft.damageReductionTiles, forestTiles),
      slowMovementTiles: mergedOverlayTiles(draft.slowMovementTiles, forestTiles),
    };
  }

  exportMap() {
    if (!this.draft) throw new Error("Map is not initialized.");
    const draft = clone(this.draft);
    normalizeDraft(draft);
    // Keep legacy flat authored maps byte-shape compatible. Elevation stays optional, while a sun
    // record independently opts any map into directional lighting and atmosphere.
    if (!draft.elevation.length) delete draft.elevation;
    if (!draft.sun) delete draft.sun;
    return draft;
  }

  notify(reason, detail = {}) {
    const snapshot = { ...this.snapshot(), reason, ...detail };
    for (const handler of this.subscribers) handler(snapshot);
  }
}

export function symmetricMapTiles(dimensions, tiles, symmetry = MAP_EDITOR_SYMMETRY.NONE) {
  return symmetricTiles(dimensions, tiles, symmetry, ({ x, y }) => ({ x, y }));
}

/**
 * Expand terrain paint across the selected symmetry while keeping marked-road directions
 * geometrically correct in each reflected or rotated copy.
 */
export function symmetricTerrainTiles(dimensions, tiles, terrainCode, symmetry = MAP_EDITOR_SYMMETRY.NONE) {
  return symmetricTiles(dimensions, tiles, symmetry, ({ x, y }, transform) => ({
    x,
    y,
    paintTerrainCode: transformTerrainCode(terrainCode, transform),
  }));
}

function symmetricTiles(dimensions, tiles, symmetry, decorate = (tile) => tile) {
  return expandSymmetricPoints(dimensions, tiles, symmetry, { decorate });
}

export function mapEditorRectTiles(first, last, dimensions) {
  return rectTiles(dimensions, first, last);
}

export function moveSymmetricDraftLocation(draft, {
  kind = "base", locationIndex = 0, tile, symmetry = MAP_EDITOR_SYMMETRY.NONE,
} = {}) {
  const collection = locationCollection(draft, kind);
  const radius = locationRadius(kind);
  const source = collection?.[Math.trunc(Number(locationIndex))];
  const target = normalizedDraftTile(draft, tile, radius);
  if (!source || !target) return draftEditError("Choose a valid location and map tile.");
  if (sameLocation(source, target)) return { ok: true, count: 0 };
  const transforms = symmetryTransforms(draftDimensions(draft), symmetry);
  const plannedSources = new Set();
  const plans = [];
  for (const transform of transforms) {
    const transformedFrom = transformMapTile(source, draftDimensions(draft), transform);
    const index = transformedLocationIndex(collection, transformedFrom, plannedSources, symmetry);
    if (index < 0 || plannedSources.has(index)) continue;
    const from = collection[index];
    const to = transformMapTile(target, draftDimensions(draft), transform);
    if (!draftLocationTileWithinClearance(draft, to, radius)) {
      return draftEditError("That location's symmetric copies do not fit within the map edge clearance.");
    }
    plannedSources.add(index);
    plans.push({
      index,
      from,
      to,
      baseIndex: draft.baseSites.findIndex((candidate) => sameLocation(candidate, from)),
      startIndex: draft.startLocations.findIndex((candidate) => sameLocation(candidate, from)),
    });
  }
  if (!plans.length) return { ok: true, count: 0 };
  const plannedCoordinates = new Set(plans.map((plan) => locationKey(plan.from)));
  const targets = new Set();
  for (const plan of plans) {
    const key = locationKey(plan.to);
    if (targets.has(key)) return draftEditError("That symmetric move would place multiple bases on the same tile.");
    targets.add(key);
    const occupied = draft.baseSites.find((candidate) => sameLocation(candidate, plan.to));
    if (occupied && !plannedCoordinates.has(locationKey(occupied))) {
      return draftEditError("A base already uses that tile.");
    }
  }
  for (const plan of plans) {
    if (kind === "start") {
      draft.startLocations[plan.index] = copyLocation(plan.to);
      if (plan.baseIndex >= 0) draft.baseSites[plan.baseIndex] = movedBaseSite(draft.baseSites[plan.baseIndex], plan.to);
    } else {
      draft.baseSites[plan.index] = movedBaseSite(draft.baseSites[plan.index], plan.to);
      if (plan.startIndex >= 0) draft.startLocations[plan.startIndex] = copyLocation(plan.to);
    }
  }
  return { ok: true, count: plans.length };
}

export function addSymmetricDraftLocations(draft, {
  kind = "base", tile, symmetry = MAP_EDITOR_SYMMETRY.NONE,
} = {}) {
  const radius = locationRadius(kind);
  const target = normalizedDraftTile(draft, tile, radius);
  if (!target) return draftEditError("Choose a valid map tile.");
  const locations = symmetricDraftLocationTiles(draft, target, symmetry, radius);
  if (!locations) {
    return draftEditError("That location's symmetric copies do not fit within the map edge clearance.");
  }
  if (kind === "start") {
    const additions = locations.filter((location) => (
      !draft.startLocations.some((start) => sameLocation(start, location))
    ));
    if (draft.startLocations.length + additions.length > MAP_EDITOR_MAX_START_LOCATIONS) {
      return draftEditError(`A map supports at most ${MAP_EDITOR_MAX_START_LOCATIONS} start locations.`);
    }
    const missingBases = additions.filter((location) => (
      !draft.baseSites.some((site) => sameLocation(site, location))
    ));
    if (draft.baseSites.length + missingBases.length > MAP_EDITOR_MAX_BASE_SITES) {
      return draftEditError(`A map supports at most ${MAP_EDITOR_MAX_BASE_SITES} base sites.`);
    }
    for (const location of additions) {
      if (!draft.baseSites.some((site) => sameLocation(site, location))) {
        draft.baseSites.push(newBaseSite(location));
      }
      draft.startLocations.push(copyLocation(location));
    }
    return { ok: true, count: additions.length };
  }
  if (draft.baseSites.length + locations.length > MAP_EDITOR_MAX_BASE_SITES) {
    return draftEditError(`A map supports at most ${MAP_EDITOR_MAX_BASE_SITES} base sites.`);
  }
  if (locations.some((location) => draft.baseSites.some((site) => sameLocation(site, location)))) {
    return draftEditError("A base already uses that tile.");
  }
  for (const location of locations) draft.baseSites.push(newBaseSite(location));
  return { ok: true, count: locations.length };
}

export function removeDraftLocation(draft, { kind = "base", locationIndex = 0 } = {}) {
  const collection = locationCollection(draft, kind);
  const index = Math.trunc(Number(locationIndex));
  const location = collection?.[index];
  if (!location) return draftEditError("That map location is no longer present.");
  if (kind === "start") {
    draft.startLocations.splice(index, 1);
    return { ok: true };
  }
  if (draft.startLocations.some((start) => sameLocation(start, location))) {
    return draftEditError("Remove the matching start location before removing this base site.");
  }
  draft.baseSites.splice(index, 1);
  return { ok: true };
}

export function paintDraftRect(draft, rect, terrainCode) {
  const ch = TERRAIN_TO_CHAR[terrainCode];
  if (!ch || !Array.isArray(draft?.terrain) || !draft.terrain.length) return;
  const { width, height } = draftDimensions(draft);
  const x0 = clampTile(Math.min(rect.x0, rect.x1), width);
  const x1 = clampTile(Math.max(rect.x0, rect.x1), width);
  const y0 = clampTile(Math.min(rect.y0, rect.y1), height);
  const y1 = clampTile(Math.max(rect.y0, rect.y1), height);
  for (let y = y0; y <= y1; y++) {
    const chars = [...draft.terrain[y]];
    for (let x = x0; x <= x1; x++) chars[x] = ch;
    draft.terrain[y] = chars.join("");
  }
}

export function protectDraftBaseTerrain(draft) {
  if (!Array.isArray(draft?.terrain)) return;
  const starts = new Set((draft.startLocations || []).map(locationKey));
  for (const site of draft.baseSites || []) {
    const radius = starts.has(locationKey(site)) ? MAP_EDITOR_MAIN_CLEARANCE_TILES : MAP_EDITOR_BASE_SITE_CLEARANCE_TILES;
    const { width, height } = draftDimensions(draft);
    for (let y = clampTile(site.y - radius, height); y <= clampTile(site.y + radius, height); y++) {
      const chars = [...draft.terrain[y]];
      for (let x = clampTile(site.x - radius, width); x <= clampTile(site.x + radius, width); x++) {
        if (PASSABLE[CHAR_TO_TERRAIN[chars[x]]] !== true) chars[x] = TERRAIN_TO_CHAR[TERRAIN.GRASS];
      }
      draft.terrain[y] = chars.join("");
    }
  }
}

export function authoredMapFromMaterialized({
  name,
  description,
  size,
  width = size,
  height = size,
  terrain,
  elevation = [],
  sun = null,
  starts,
  baseSites,
  doodads = [],
  concealmentTiles = [],
  noVehicleTiles = [],
  noBuildingTiles = [],
  noEntrenchmentTiles = [],
  damageReductionTiles = [],
  slowMovementTiles = [],
}) {
  const mapWidth = Math.max(1, Math.trunc(Number(width)) || 1);
  const mapHeight = Math.max(1, Math.trunc(Number(height)) || 1);
  const codes = Array.from(terrain || []);
  const terrainRows = Array.from({ length: mapHeight }, (_, y) => (
    Array.from({ length: mapWidth }, (_, x) => TERRAIN_TO_CHAR[codes[y * mapWidth + x]] || ".").join("")
  ));
  const elevationCodes = Array.from(elevation || [], (level) => Number(level));
  const validElevation = elevationCodes.length === mapWidth * mapHeight
    && elevationCodes.every((level) => Number.isInteger(level) && level >= 0 && level <= 9);
  const elevationRows = validElevation && elevationCodes.some((level) => level !== 0)
    ? Array.from({ length: mapHeight }, (_, y) => (
      elevationCodes.slice(y * mapWidth, (y + 1) * mapWidth).join("")
    ))
    : [];
  const dimensions = { width: mapWidth, height: mapHeight };
  const startLocations = normalizeLocations(starts, dimensions);
  const bases = normalizeBaseSiteRecords(baseSites, dimensions);
  for (const start of startLocations) if (!bases.some((site) => sameLocation(site, start))) bases.push(newBaseSite(start));
  const draft = {
    version: MAP_EDITOR_MAP_VERSION,
    name: String(name || "Map").trim() || "Map",
    description: String(description || ""),
    _design: "Flat map locations: startLocations choose player starts; every baseSites entry defines its own steel and oil patch counts.",
    width: mapWidth,
    height: mapHeight,
    terrain: terrainRows,
    elevation: elevationRows,
    sun: sun ? { ...sun } : null,
    startLocations,
    baseSites: bases,
    doodads: normalizeDraftDoodads(doodads, dimensions),
    // A materialized map no longer carries the provenance needed to distinguish a compact forest
    // from four independently overlapping gameplay layers. Preserve its exact semantics instead
    // of guessing and unexpectedly generating trees when a Lab map returns to the editor.
    forestSpans: [],
    concealmentTiles: normalizeOverlayTiles(concealmentTiles, dimensions),
    noVehicleTiles: normalizeOverlayTiles(noVehicleTiles, dimensions),
    noBuildingTiles: normalizeOverlayTiles(noBuildingTiles, dimensions),
    noEntrenchmentTiles: normalizeOverlayTiles(noEntrenchmentTiles, dimensions),
    damageReductionTiles: normalizeOverlayTiles(damageReductionTiles, dimensions),
    slowMovementTiles: normalizeOverlayTiles(slowMovementTiles, dimensions),
  };
  normalizeDraft(draft);
  return draft;
}

export function materializedMapsEqual(left, right) {
  if (!left || !right || left.name !== right.name || left.width !== right.width || left.height !== right.height) return false;
  if (!sameFlatArray(left.terrain, right.terrain)) return false;
  if (!sameFlatArray(left.elevation || [], right.elevation || [])) return false;
  if (JSON.stringify(left.sun || null) !== JSON.stringify(right.sun || null)) return false;
  return sameLocationSet(left.starts, right.starts)
    && sameBaseSiteSet(left.baseSites, right.baseSites)
    && sameDoodadSet(left.doodads, right.doodads)
    && sameLocationSet(left.concealmentTiles, right.concealmentTiles)
    && sameLocationSet(left.noVehicleTiles, right.noVehicleTiles)
    && sameLocationSet(left.noBuildingTiles, right.noBuildingTiles)
    && sameLocationSet(left.noEntrenchmentTiles, right.noEntrenchmentTiles)
    && sameLocationSet(left.damageReductionTiles, right.damageReductionTiles)
    && sameLocationSet(left.slowMovementTiles, right.slowMovementTiles);
}

function normalizeDraft(draft) {
  if (!draft || typeof draft !== "object") throw new Error("Map data is invalid.");
  if (Number(draft.version) !== MAP_EDITOR_MAP_VERSION) {
    throw new Error(`Map schema version ${draft.version ?? "missing"} is unsupported; expected version ${MAP_EDITOR_MAP_VERSION}.`);
  }
  if (!Array.isArray(draft.forestSpans)) {
    throw new Error("Map forestSpans must be an array.");
  }
  if (!Array.isArray(draft.noBuildingTiles)) {
    throw new Error("Map noBuildingTiles must be an array.");
  }
  if (!Array.isArray(draft.noEntrenchmentTiles)) {
    throw new Error("Map noEntrenchmentTiles must be an array.");
  }
  if (!positiveInteger(draft.width) || !positiveInteger(draft.height)) {
    const inferred = inferredDraftDimensions(draft);
    draft.width = inferred.width;
    draft.height = inferred.height;
  }
  const height = positiveInteger(draft.height);
  const width = positiveInteger(draft.width);
  if (!width || !height || !Array.isArray(draft.terrain) || draft.terrain.length !== height || draft.terrain.some((row) => typeof row !== "string" || [...row].length !== width)) {
    throw new Error("Map terrain rows must match its width and height.");
  }
  draft.name = String(draft.name || "Map").trim() || "Map";
  draft.description = String(draft.description || "");
  draft._design = String(draft._design || "Flat map locations.");
  draft.terrain = draft.terrain.map((row) => [...row].map((ch) => CHAR_TO_TERRAIN[ch] === undefined ? "." : ch).join(""));
  if (draft.elevation == null) draft.elevation = [];
  if (!Array.isArray(draft.elevation)
    || (draft.elevation.length !== 0 && (draft.elevation.length !== height
      || draft.elevation.some((row) => typeof row !== "string" || !/^[0-9]+$/.test(row) || [...row].length !== width)))) {
    throw new Error("Map elevation rows must be empty or match its width and height using digits 0-9.");
  }
  const elevationValues = draft.elevation.flatMap((row) => [...row]);
  const hasRelief = elevationValues.length > 0
    && elevationValues.some((level) => level !== elevationValues[0]);
  if (hasRelief && !draft.sun) throw new Error("Maps with varying elevation must specify sun conditions.");
  draft.sun = normalizeMapSun(draft.sun);
  const dimensions = { width, height };
  draft.width = width;
  draft.height = height;
  draft.startLocations = normalizeLocations(draft.startLocations, dimensions).slice(0, MAP_EDITOR_MAX_START_LOCATIONS);
  draft.baseSites = normalizeBaseSites(draft.baseSites, draft.startLocations, dimensions);
  draft.forestSpans = normalizeForestSpans(draft.forestSpans, dimensions);
  draft.doodads = normalizeDraftDoodads(draft.doodads, dimensions);
  draft.concealmentTiles = normalizeOverlayTiles(draft.concealmentTiles, dimensions);
  draft.noVehicleTiles = normalizeOverlayTiles(draft.noVehicleTiles, dimensions);
  draft.noBuildingTiles = normalizeOverlayTiles(draft.noBuildingTiles, dimensions);
  draft.noEntrenchmentTiles = normalizeOverlayTiles(draft.noEntrenchmentTiles, dimensions);
  draft.damageReductionTiles = normalizeOverlayTiles(draft.damageReductionTiles, dimensions);
  draft.slowMovementTiles = normalizeOverlayTiles(draft.slowMovementTiles, dimensions);
  reconcileForestDoodads(draft);
}

function resizeDraftCentered(source, width, height) {
  const current = draftDimensions(source);
  const offsetX = Math.floor((width - current.width) / 2);
  const offsetY = Math.floor((height - current.height) / 2);
  const terrain = Array.from({ length: height }, () => Array(width).fill(TERRAIN_TO_CHAR[TERRAIN.GRASS]));
  const elevation = source.elevation?.length
    ? Array.from({ length: height }, () => Array(width).fill("0"))
    : null;
  for (let y = 0; y < current.height; y++) {
    const targetY = y + offsetY;
    if (targetY < 0 || targetY >= height) continue;
    const row = [...source.terrain[y]];
    for (let x = 0; x < current.width; x++) {
      const targetX = x + offsetX;
      if (targetX >= 0 && targetX < width) terrain[targetY][targetX] = row[x];
      if (elevation && targetX >= 0 && targetX < width) elevation[targetY][targetX] = source.elevation[y][x];
    }
  }
  const shift = (location) => ({ ...location, x: location.x + offsetX, y: location.y + offsetY });
  const startLocations = source.startLocations.map(shift);
  const baseSites = source.baseSites.map(shift);
  const forestDoodadIds = forestOwnedDoodadIds(source.doodads, source, {
    maxDoodads: MAP_EDITOR_MAX_DOODADS,
  });
  const doodads = (source.doodads || []).filter((doodad) => !forestDoodadIds.has(Number(doodad.id))).map((doodad) => ({
    ...copyDoodad(doodad),
    x: doodad.x + offsetX * 32,
    y: doodad.y + offsetY * 32,
  }));
  const shiftTile = (tile) => ({ x: tile.x + offsetX, y: tile.y + offsetY });
  const forestTiles = forestTilesFromSpans(source.forestSpans, current).map(shiftTile);
  const draft = {
    ...clone(source),
    width,
    height,
    terrain: terrain.map((row) => row.join("")),
    elevation: elevation ? elevation.map((row) => row.join("")) : [],
    startLocations,
    baseSites,
    doodads,
    forestSpans: forestSpansFromTiles(forestTiles, { width, height }),
    concealmentTiles: (source.concealmentTiles || []).map(shiftTile),
    noVehicleTiles: (source.noVehicleTiles || []).map(shiftTile),
    noBuildingTiles: (source.noBuildingTiles || []).map(shiftTile),
    noEntrenchmentTiles: (source.noEntrenchmentTiles || []).map(shiftTile),
    damageReductionTiles: (source.damageReductionTiles || []).map(shiftTile),
    slowMovementTiles: (source.slowMovementTiles || []).map(shiftTile),
  };
  if (startLocations.some((location) => !draftLocationTileWithinClearance(draft, location, MAP_EDITOR_MAIN_CLEARANCE_TILES))) {
    return draftEditError("Resize would move a start location inside the map edge clearance.");
  }
  const startKeys = new Set(startLocations.map(locationKey));
  if (baseSites.some((location) => !draftLocationTileWithinClearance(
    draft,
    location,
    startKeys.has(locationKey(location)) ? MAP_EDITOR_MAIN_CLEARANCE_TILES : MAP_EDITOR_BASE_SITE_CLEARANCE_TILES,
  ))) return draftEditError("Resize would move a base site inside the map edge clearance.");
  normalizeDraft(draft);
  protectDraftBaseTerrain(draft);
  return { ok: true, draft };
}

function locationCollection(draft, kind) { return kind === "start" ? draft?.startLocations : draft?.baseSites; }
function locationRadius(kind) { return kind === "start" ? MAP_EDITOR_MAIN_CLEARANCE_TILES : MAP_EDITOR_BASE_SITE_CLEARANCE_TILES; }
function protectedTerrainTile(draft, x, y) {
  const starts = new Set((draft.startLocations || []).map(locationKey));
  return (draft.baseSites || []).some((site) => {
    const radius = starts.has(locationKey(site)) ? MAP_EDITOR_MAIN_CLEARANCE_TILES : MAP_EDITOR_BASE_SITE_CLEARANCE_TILES;
    return Math.abs(site.x - x) <= radius && Math.abs(site.y - y) <= radius;
  });
}
function normalizeLocations(locations, dimensions) {
  const out = [];
  const seen = new Set();
  for (const location of Array.isArray(locations) ? locations : []) {
    const valid = validMapTile(location, dimensions);
    if (valid && !seen.has(locationKey(valid))) { seen.add(locationKey(valid)); out.push(valid); }
  }
  return out;
}

function normalizeOverlayTiles(locations, dimensions) {
  return normalizeLocations(locations, dimensions)
    .sort((left, right) => left.x - right.x || left.y - right.y);
}

function mergedOverlayTiles(explicit, forest) {
  const byKey = new Map([...explicit, ...forest].map((tile) => [locationKey(tile), copyLocation(tile)]));
  return [...byKey.values()].sort((left, right) => left.x - right.x || left.y - right.y);
}

function reconcileForestDoodads(draft) {
  const ownedIds = forestOwnedDoodadIds(draft.doodads, draft, {
    maxDoodads: MAP_EDITOR_MAX_DOODADS,
  });
  const manual = (draft.doodads || []).filter((doodad) => !ownedIds.has(Number(doodad.id)));
  const manualIds = new Set(manual.map((doodad) => Number(doodad.id)));
  const available = Math.max(0, MAP_EDITOR_MAX_DOODADS - manual.length);
  draft.doodads = normalizeDraftDoodads([
    ...manual,
    ...generatedForestDoodads(draft.forestSpans, draft, { max: available })
      .filter((doodad) => !manualIds.has(doodad.id)),
  ], draft);
}

function normalizeBaseSites(baseSites, startLocations, dimensions) {
  const normalized = normalizeBaseSiteRecords(baseSites, dimensions);
  const startKeys = new Set(startLocations.map(locationKey));
  if (
    normalized.length <= MAP_EDITOR_MAX_BASE_SITES
    && startLocations.every((start) => normalized.some((site) => sameLocation(site, start)))
  ) return normalized;

  const retainedStarts = normalized.filter((site) => startKeys.has(locationKey(site)));
  const missingStarts = startLocations.filter((start) => !normalized.some((site) => sameLocation(site, start)));
  const availableBaseSlots = MAP_EDITOR_MAX_BASE_SITES - retainedStarts.length - missingStarts.length;
  const retainedBases = normalized
    .filter((site) => !startKeys.has(locationKey(site)))
    .slice(0, Math.max(0, availableBaseSlots));
  return [...retainedStarts, ...retainedBases, ...missingStarts.map(newBaseSite)];
}
function normalizeBaseSiteRecords(baseSites, dimensions) {
  const out = [];
  const seen = new Set();
  for (const site of Array.isArray(baseSites) ? baseSites : []) {
    const valid = validMapTile(site, dimensions);
    if (!valid || seen.has(locationKey(valid))) continue;
    seen.add(locationKey(valid));
    out.push({
      ...valid,
      steelPatches: boundedPatchCount(site?.steelPatches, MAP_EDITOR_MAX_STEEL_PATCHES, MAP_EDITOR_DEFAULT_STEEL_PATCHES),
      oilPatches: boundedPatchCount(site?.oilPatches, MAP_EDITOR_MAX_OIL_PATCHES, MAP_EDITOR_DEFAULT_OIL_PATCHES),
    });
  }
  return out;
}
function normalizedDraftTile(draft, tile, radius) {
  const dimensions = draftDimensions(draft);
  const valid = validMapTile(tile, dimensions);
  if (!valid || dimensions.width <= radius * 2 || dimensions.height <= radius * 2) return null;
  return {
    x: Math.max(radius, Math.min(dimensions.width - radius - 1, valid.x)),
    y: Math.max(radius, Math.min(dimensions.height - radius - 1, valid.y)),
  };
}
function symmetricDraftLocationTiles(draft, tile, symmetry, radius) {
  const locations = [];
  const seen = new Set();
  const dimensions = draftDimensions(draft);
  for (const transform of symmetryTransforms(dimensions, symmetry)) {
    const transformed = transformMapTile(tile, dimensions, transform);
    if (!draftLocationTileWithinClearance(draft, transformed, radius)) return null;
    const key = locationKey(transformed);
    if (!seen.has(key)) {
      seen.add(key);
      locations.push(transformed);
    }
  }
  return locations;
}
function draftLocationTileWithinClearance(draft, tile, radius) {
  const { width, height } = draftDimensions(draft);
  const valid = validMapTile(tile, { width, height });
  return !!valid
    && valid.x >= radius
    && valid.y >= radius
    && valid.x < width - radius
    && valid.y < height - radius;
}
function transformedLocationIndex(collection, target, excluded, symmetry) {
  if (!target) return -1;
  const exact = collection.findIndex((candidate, index) => (
    !excluded.has(index) && sameLocation(candidate, target)
  ));
  if (exact >= 0 || symmetry !== MAP_EDITOR_SYMMETRY.THREE_WAY) return exact;

  // Snapping a 120-degree rotation to square-grid tiles can move a rotated copy by one
  // tile when that copy is used as the next rotation's source. Match that bounded drift so
  // every member of a generated three-location group remains a valid drag handle.
  let nearest = -1;
  let nearestDistance = Infinity;
  for (let index = 0; index < collection.length; index++) {
    if (excluded.has(index)) continue;
    const candidate = collection[index];
    const dx = candidate.x - target.x;
    const dy = candidate.y - target.y;
    const distance = dx * dx + dy * dy;
    if (distance <= 2 && distance < nearestDistance) {
      nearest = index;
      nearestDistance = distance;
    }
  }
  return nearest;
}
function validMapTile(tile, dimensions) {
  const map = normalizeDimensions(dimensions);
  if (!map) return null;
  const x = Math.trunc(Number(tile?.x)); const y = Math.trunc(Number(tile?.y));
  return Number.isInteger(x) && Number.isInteger(y) && x >= 0 && y >= 0 && x < map.width && y < map.height ? { x, y } : null;
}
function transformMapTile(tile, dimensions, transform) {
  return transformAuthoringPoint(tile, dimensions, transform);
}

export function mapEditorSymmetrySupported(dimensions, symmetry) {
  return symmetrySupported(dimensions, symmetry);
}

function symmetryTransforms(dimensions, symmetry) {
  return authoringSymmetryTransforms(dimensions, symmetry);
}
function transformTerrainCode(code, transform) {
  const character = TERRAIN_TO_CHAR[code];
  return CHAR_TO_TERRAIN[transformRoadCharacter(character, transform)] ?? code;
}
function locationKey(location) { return `${location?.x},${location?.y}`; }
function sameLocation(a, b) { return !!a && !!b && a.x === b.x && a.y === b.y; }
function copyLocation(location) { return { x: Number(location?.x), y: Number(location?.y) }; }
function copyBaseSite(site) {
  return {
    ...copyLocation(site),
    steelPatches: boundedPatchCount(site?.steelPatches, MAP_EDITOR_MAX_STEEL_PATCHES, MAP_EDITOR_DEFAULT_STEEL_PATCHES),
    oilPatches: boundedPatchCount(site?.oilPatches, MAP_EDITOR_MAX_OIL_PATCHES, MAP_EDITOR_DEFAULT_OIL_PATCHES),
  };
}
function copyDoodad(record) {
  const copy = { id: record.id, typeId: record.typeId, x: record.x, y: record.y };
  if (record.color) copy.color = record.color;
  return copy;
}
function newBaseSite(location) {
  return {
    ...copyLocation(location),
    steelPatches: MAP_EDITOR_DEFAULT_STEEL_PATCHES,
    oilPatches: MAP_EDITOR_DEFAULT_OIL_PATCHES,
  };
}
function movedBaseSite(site, location) { return { ...copyBaseSite(site), ...copyLocation(location) }; }
function boundedPatchCount(value, max, fallback) {
  return boundedAuthoredPatchCount(value, max, fallback);
}
function sameLocationSet(left, right) {
  const a = new Set((left || []).map(locationKey)); const b = new Set((right || []).map(locationKey));
  return a.size === b.size && [...a].every((key) => b.has(key));
}
function sameBaseSiteSet(left, right) {
  const record = (site) => `${locationKey(site)}:${site?.steelPatches}:${site?.oilPatches}`;
  const a = new Set((left || []).map(record)); const b = new Set((right || []).map(record));
  return a.size === b.size && [...a].every((key) => b.has(key));
}
function sameDoodadSet(left, right) {
  const record = (doodad) => `${doodad?.id}:${doodad?.typeId}:${doodad?.x}:${doodad?.y}:${doodad?.color || ""}`;
  const a = new Set((left || []).map(record)); const b = new Set((right || []).map(record));
  return a.size === b.size && [...a].every((key) => b.has(key));
}
function sameFlatArray(left, right) { return Array.isArray(left) && Array.isArray(right) && left.length === right.length && left.every((value, index) => value === right[index]); }
function normalizeMapSun(value) {
  if (value == null) return null;
  const sun = {
    azimuthDegrees: Number(value.azimuthDegrees),
    elevationDegrees: Number(value.elevationDegrees),
    warmth: Number(value.warmth),
  };
  if (!Number.isInteger(sun.azimuthDegrees) || sun.azimuthDegrees < 0 || sun.azimuthDegrees > 359
    || !Number.isInteger(sun.elevationDegrees) || sun.elevationDegrees < 1 || sun.elevationDegrees > 89
    || !Number.isInteger(sun.warmth) || sun.warmth < 0 || sun.warmth > 100) {
    throw new Error("Map sun conditions require azimuth 0-359, elevation 1-89, and warmth 0-100.");
  }
  return sun;
}
function draftFingerprint(draft) { return JSON.stringify(draft); }
function clone(value) { return structuredCloneSafe(value); }
function structuredCloneSafe(value) { return typeof structuredClone === "function" ? structuredClone(value) : JSON.parse(JSON.stringify(value)); }
function positiveInteger(value) { const number = Number(value); return Number.isInteger(number) && number > 0 ? number : 0; }
function validEditorDimension(value) { return value >= MAP_EDITOR_MIN_SIZE && value <= MAP_EDITOR_MAX_SIZE; }
function boundedMapDimension(value) {
  return Math.max(MAP_EDITOR_MIN_SIZE, Math.min(MAP_EDITOR_MAX_SIZE, Math.trunc(Number(value)) || MAP_EDITOR_DEFAULT_SIZE));
}
function inferredDraftDimensions(draft) {
  const height = positiveInteger(draft?.height) || (Array.isArray(draft?.terrain) ? draft.terrain.length : 0);
  const inferredWidth = Array.isArray(draft?.terrain) && typeof draft.terrain[0] === "string"
    ? [...draft.terrain[0]].length
    : 0;
  return {
    width: positiveInteger(draft?.width) || inferredWidth,
    height,
  };
}
function draftDimensions(draft) { return normalizeDimensions({ width: draft?.width, height: draft?.height }) || { width: 0, height: 0 }; }
function normalizeDraftDoodads(records, dimensions) {
  const map = normalizeDimensions(dimensions);
  if (!map) return [];
  return normalizeMapEditorDoodads(records, {
    width: map.width * 32,
    height: map.height * 32,
  }, { max: MAP_EDITOR_MAX_DOODADS });
}
function draftContainsWorldPoint(draft, point) {
  const map = draftDimensions(draft);
  const x = Math.round(Number(point?.x));
  const y = Math.round(Number(point?.y));
  return Number.isFinite(x) && Number.isFinite(y)
    && x >= 0 && y >= 0 && x < map.width * 32 && y < map.height * 32;
}
function normalizeDimensions(value) {
  if (typeof value === "number") {
    const size = positiveInteger(value);
    return size ? { width: size, height: size } : null;
  }
  const width = positiveInteger(value?.width);
  const height = positiveInteger(value?.height);
  return width && height ? { width, height } : null;
}
function clampTile(value, size) { return Math.max(0, Math.min(size - 1, Math.trunc(value))); }
function draftEditError(error) { return { ok: false, error }; }
