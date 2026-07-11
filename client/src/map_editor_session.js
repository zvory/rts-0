import { TERRAIN } from "./protocol.js";

export const MAP_EDITOR_HISTORY_LIMIT = 25;
export const MAP_EDITOR_MAX_START_LOCATIONS = 4;
export const MAP_EDITOR_MAX_BASE_SITES = 32;
// Mirror the authored-map clearance contract enforced by the simulation.
export const MAP_EDITOR_MAIN_CLEARANCE_TILES = 7;
export const MAP_EDITOR_BASE_SITE_CLEARANCE_TILES = 4;
export const MAP_EDITOR_SYMMETRY = Object.freeze({
  NONE: "none",
  HORIZONTAL: "horizontal",
  VERTICAL: "vertical",
  HALF_TURN: "halfTurn",
  RADIAL: "radial",
  DIAGONAL_MAIN: "diagonalMain",
  DIAGONAL_ANTI: "diagonalAnti",
});

const TERRAIN_TO_CHAR = Object.freeze({
  [TERRAIN.GRASS]: ".",
  [TERRAIN.ROCK]: "#",
  [TERRAIN.WATER]: "~",
});
const CHAR_TO_TERRAIN = Object.freeze({ ".": TERRAIN.GRASS, "#": TERRAIN.ROCK, "~": TERRAIN.WATER });
const SYMMETRY_TRANSFORMS = Object.freeze({
  [MAP_EDITOR_SYMMETRY.NONE]: ["identity"],
  [MAP_EDITOR_SYMMETRY.HORIZONTAL]: ["identity", "horizontal"],
  [MAP_EDITOR_SYMMETRY.VERTICAL]: ["identity", "vertical"],
  [MAP_EDITOR_SYMMETRY.HALF_TURN]: ["identity", "rotate180"],
  [MAP_EDITOR_SYMMETRY.RADIAL]: ["identity", "rotate90", "rotate180", "rotate270"],
  [MAP_EDITOR_SYMMETRY.DIAGONAL_MAIN]: ["identity", "diagonalMain"],
  [MAP_EDITOR_SYMMETRY.DIAGONAL_ANTI]: ["identity", "diagonalAnti"],
});

export class MapEditorSession {
  constructor({ storage = defaultStorage(), historyLimit = MAP_EDITOR_HISTORY_LIMIT } = {}) {
    this.storage = storage;
    this.historyLimit = Math.max(1, Math.trunc(historyLimit) || MAP_EDITOR_HISTORY_LIMIT);
    this.draft = null;
    this.undoStack = [];
    this.redoStack = [];
    this.subscribers = new Set();
    this.desiredTool = null;
    this.lastAction = "";
    this.savedFingerprint = "";
    this.terrainStroke = null;
  }

  get initialized() { return !!this.draft; }

  initializeFromStart(startPayload, { name = "Map" } = {}) {
    if (this.draft) return false;
    const map = startPayload?.map || {};
    const size = Number(map.width);
    if (!Number.isInteger(size) || size <= 0 || Number(map.height) !== size) return false;
    this.draft = authoredMapFromMaterialized({
      name,
      description: "Map imported from an authoritative session.",
      size,
      terrain: map.terrain,
      starts: (startPayload?.players || []).map((player) => ({ x: Number(player.startTileX), y: Number(player.startTileY) })),
      baseSites: [],
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
      size: data.size,
      terrain: data.terrain,
      starts: data.starts,
      baseSites: data.baseSites || data.expansionSites,
    });
    this.undoStack = [];
    this.redoStack = [];
    this.markSaved({ notify: false });
    this.notify("initialized");
    return true;
  }

  initializeBlank({ size = 126, playerCount = 2, name = "Untitled map" } = {}) {
    const mapSize = Math.max(16, Math.min(126, Math.trunc(Number(size)) || 126));
    const count = Math.max(1, Math.min(MAP_EDITOR_MAX_START_LOCATIONS, Math.trunc(Number(playerCount)) || 2));
    const startTile = (fraction) => Math.max(
      MAP_EDITOR_MAIN_CLEARANCE_TILES,
      Math.min(mapSize - MAP_EDITOR_MAIN_CLEARANCE_TILES - 1, Math.floor(mapSize * fraction)),
    );
    const starts = [
      { x: startTile(0.25), y: startTile(0.25) },
      { x: startTile(0.75), y: startTile(0.75) },
      { x: startTile(0.75), y: startTile(0.25) },
      { x: startTile(0.25), y: startTile(0.75) },
    ].slice(0, count);
    this.draft = authoredMapFromMaterialized({
      name,
      description: "",
      size: mapSize,
      terrain: Array(mapSize * mapSize).fill(TERRAIN.GRASS),
      starts,
      baseSites: starts,
    });
    this.undoStack = [];
    this.redoStack = [];
    this.markSaved({ notify: false });
    this.lastAction = "Created blank map";
    this.notify("loaded");
    return true;
  }

  loadAuthoredMap(source, { expectedSize = null, playerCount = null } = {}) {
    const draft = clone(source);
    normalizeDraft(draft);
    const requiredSize = positiveInteger(expectedSize);
    if (requiredSize && draft.terrain.length !== requiredSize) {
      throw new Error(`This session uses a ${requiredSize} × ${requiredSize} map; ${draft.name} is ${draft.terrain.length} × ${draft.terrain.length}.`);
    }
    const requiredPlayers = positiveInteger(playerCount);
    if (requiredPlayers && draft.startLocations.length !== requiredPlayers) {
      throw new Error(`${draft.name} has ${draft.startLocations.length} start locations, not ${requiredPlayers}.`);
    }
    this.draft = draft;
    this.undoStack = [];
    this.redoStack = [];
    this.lastAction = `Loaded ${draft.name}`;
    this.notify("loaded");
    return true;
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
    const ch = TERRAIN_TO_CHAR[terrainCode];
    if (!this.draft || !this.terrainStroke || !ch || !Array.isArray(tiles)) return [];
    const size = this.draft.terrain.length;
    const byRow = new Map();
    const changed = [];
    for (const tile of tiles) {
      const x = Math.trunc(Number(tile?.x));
      const y = Math.trunc(Number(tile?.y));
      if (x < 0 || y < 0 || x >= size || y >= size || protectedTerrainTile(this.draft, x, y)) continue;
      const row = byRow.get(y) || [...this.draft.terrain[y]];
      if (row[x] === ch) continue;
      row[x] = ch;
      byRow.set(y, row);
      const change = { x, y, code: terrainCode };
      this.terrainStroke.dirty.set(`${x},${y}`, change);
      changed.push(change);
    }
    for (const [y, row] of byRow) this.draft.terrain[y] = row.join("");
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

  mapOverlay() {
    if (!this.draft) return null;
    const starts = this.draft.startLocations.map((location, index) => ({ ...location, index }));
    const startKeys = new Set(starts.map(locationKey));
    const bases = this.draft.baseSites
      .map((location, index) => ({ ...location, index }))
      .filter((site) => !startKeys.has(locationKey(site)));
    return { starts, bases };
  }

  saveLocal(key) {
    if (!this.draft || !this.storage?.setItem) return false;
    try { this.storage.setItem(storageKey(key), JSON.stringify({ schemaVersion: 3, draft: this.draft })); } catch { return false; }
    this.lastAction = "Saved local map";
    this.markSaved();
    return true;
  }

  loadLocal(key) {
    if (!this.storage?.getItem) return false;
    let parsed;
    try {
      const text = this.storage.getItem(storageKey(key)) || this.storage.getItem(legacyStorageKey(key));
      if (!text) return false;
      parsed = JSON.parse(text);
      if (parsed?.draft) parsed = parsed.draft;
      normalizeDraft(parsed);
    } catch { return false; }
    if (!this.draft) {
      this.draft = parsed;
      this.lastAction = "Loaded local map";
      this.markSaved({ notify: false });
      this.notify("loaded");
      return true;
    }
    this.mutate("Loaded local map", (draft) => replaceObject(draft, parsed));
    this.markSaved({ notify: false });
    return true;
  }

  materialized() {
    if (!this.draft) throw new Error("Map is not initialized.");
    const draft = clone(this.draft);
    normalizeDraft(draft);
    return {
      name: draft.name,
      size: draft.terrain.length,
      terrain: draft.terrain.flatMap((row) => [...row].map((ch) => CHAR_TO_TERRAIN[ch])),
      starts: draft.startLocations.map(copyLocation),
      baseSites: draft.baseSites.map(copyLocation),
    };
  }

  exportMap() {
    if (!this.draft) throw new Error("Map is not initialized.");
    const draft = clone(this.draft);
    normalizeDraft(draft);
    return draft;
  }

  notify(reason, detail = {}) {
    const snapshot = { ...this.snapshot(), reason, ...detail };
    for (const handler of this.subscribers) handler(snapshot);
  }
}

export function symmetricMapTiles(size, tiles, symmetry = MAP_EDITOR_SYMMETRY.NONE) {
  const mapSize = positiveInteger(size);
  if (!mapSize || !Array.isArray(tiles)) return [];
  const expanded = [];
  const seen = new Set();
  for (const tile of tiles) {
    const source = validMapTile(tile, mapSize);
    if (!source) continue;
    for (const transform of SYMMETRY_TRANSFORMS[normalizeMapEditorSymmetry(symmetry)]) {
      const transformed = transformMapTile(source, mapSize, transform);
      if (!transformed || seen.has(locationKey(transformed))) continue;
      seen.add(locationKey(transformed));
      expanded.push(transformed);
    }
  }
  return expanded;
}

export function mapEditorRectTiles(first, last, size) {
  const mapSize = positiveInteger(size);
  const start = validMapTile(first, mapSize);
  const end = validMapTile(last, mapSize);
  if (!start || !end) return [];
  const tiles = [];
  for (let y = Math.min(start.y, end.y); y <= Math.max(start.y, end.y); y++) {
    for (let x = Math.min(start.x, end.x); x <= Math.max(start.x, end.x); x++) tiles.push({ x, y });
  }
  return tiles;
}

export function moveSymmetricDraftLocation(draft, {
  kind = "base", locationIndex = 0, tile, symmetry = MAP_EDITOR_SYMMETRY.NONE,
} = {}) {
  const collection = locationCollection(draft, kind);
  const radius = locationRadius(kind);
  const source = collection?.[Math.trunc(Number(locationIndex))];
  const target = normalizedDraftTile(draft, tile, radius);
  if (!source || !target) return draftEditError("Choose a valid location and map tile.");
  const transforms = SYMMETRY_TRANSFORMS[normalizeMapEditorSymmetry(symmetry)];
  const plannedSources = new Set();
  const plans = [];
  for (const transform of transforms) {
    const from = transformMapTile(source, draft.terrain.length, transform);
    const to = transformMapTile(target, draft.terrain.length, transform);
    const index = collection.findIndex((candidate) => sameLocation(candidate, from));
    if (index < 0 || !to || plannedSources.has(index)) continue;
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
      if (plan.baseIndex >= 0) draft.baseSites[plan.baseIndex] = copyLocation(plan.to);
    } else {
      draft.baseSites[plan.index] = copyLocation(plan.to);
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
  const locations = symmetricMapTiles(draft.terrain.length, [target], symmetry);
  const limit = kind === "start" ? MAP_EDITOR_MAX_START_LOCATIONS : MAP_EDITOR_MAX_BASE_SITES;
  const current = kind === "start" ? draft.startLocations.length : draft.baseSites.length;
  if (current + locations.length > limit) return draftEditError(`A map supports at most ${limit} ${kind === "start" ? "start locations" : "base sites"}.`);
  if (locations.some((location) => draft.baseSites.some((site) => sameLocation(site, location)))) {
    return draftEditError("A base already uses that tile.");
  }
  for (const location of locations) {
    draft.baseSites.push(copyLocation(location));
    if (kind === "start") draft.startLocations.push(copyLocation(location));
  }
  return { ok: true, count: locations.length };
}

export function removeDraftLocation(draft, { kind = "base", locationIndex = 0 } = {}) {
  const collection = locationCollection(draft, kind);
  const index = Math.trunc(Number(locationIndex));
  const location = collection?.[index];
  if (!location) return draftEditError("That map location is no longer present.");
  if (kind === "start") {
    if (draft.startLocations.length <= 1) return draftEditError("A map needs at least one start location.");
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
  const size = draft.terrain.length;
  const x0 = clampTile(Math.min(rect.x0, rect.x1), size);
  const x1 = clampTile(Math.max(rect.x0, rect.x1), size);
  const y0 = clampTile(Math.min(rect.y0, rect.y1), size);
  const y1 = clampTile(Math.max(rect.y0, rect.y1), size);
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
    paintDraftRect(draft, { x0: site.x - radius, y0: site.y - radius, x1: site.x + radius, y1: site.y + radius }, TERRAIN.GRASS);
  }
}

export function authoredMapFromMaterialized({ name, description, size, terrain, starts, baseSites }) {
  const mapSize = Math.max(1, Math.trunc(Number(size)) || 1);
  const codes = Array.from(terrain || []);
  const terrainRows = Array.from({ length: mapSize }, (_, y) => (
    Array.from({ length: mapSize }, (_, x) => TERRAIN_TO_CHAR[codes[y * mapSize + x]] || ".").join("")
  ));
  const startLocations = normalizeLocations(starts, mapSize);
  const bases = normalizeLocations(baseSites, mapSize);
  for (const start of startLocations) if (!bases.some((site) => sameLocation(site, start))) bases.push(copyLocation(start));
  const draft = {
    version: 3,
    name: String(name || "Map").trim() || "Map",
    description: String(description || ""),
    _design: "Flat map locations: startLocations choose player starts; every baseSites entry always spawns its resource cluster.",
    terrain: terrainRows,
    startLocations,
    baseSites: bases,
  };
  normalizeDraft(draft);
  return draft;
}

export function materializedMapsEqual(left, right) {
  if (!left || !right || left.name !== right.name || left.size !== right.size) return false;
  if (!sameFlatArray(left.terrain, right.terrain)) return false;
  return sameLocationSet(left.starts, right.starts) && sameLocationSet(left.baseSites, right.baseSites);
}

function normalizeDraft(draft) {
  if (!draft || typeof draft !== "object") throw new Error("Map data is invalid.");
  if (Number(draft.version) !== 3) replaceObject(draft, migrateLegacyDraft(draft));
  const size = draft.terrain?.length;
  if (!positiveInteger(size) || !Array.isArray(draft.terrain) || draft.terrain.some((row) => typeof row !== "string" || [...row].length !== size)) {
    throw new Error("Map terrain must be a square grid.");
  }
  draft.version = 3;
  draft.name = String(draft.name || "Map").trim() || "Map";
  draft.description = String(draft.description || "");
  draft._design = String(draft._design || "Flat map locations.");
  draft.terrain = draft.terrain.map((row) => [...row].map((ch) => CHAR_TO_TERRAIN[ch] === undefined ? "." : ch).join(""));
  draft.startLocations = normalizeLocations(draft.startLocations, size).slice(0, MAP_EDITOR_MAX_START_LOCATIONS);
  draft.baseSites = normalizeBaseSites(draft.baseSites, draft.startLocations, size);
  if (!draft.startLocations.length) throw new Error("Map needs at least one start location.");
  protectDraftBaseTerrain(draft);
}

function migrateLegacyDraft(source) {
  const sites = Array.isArray(source?.sites) ? source.sites : [];
  const byId = new Map(sites.map((site) => [site.id, site]));
  const starts = [];
  for (const layout of source?.layouts || []) for (const slot of layout?.slots || []) {
    const site = byId.get(slot.main);
    if (site && !starts.some((candidate) => sameLocation(candidate, site))) starts.push(copyLocation(site));
  }
  if (!starts.length) for (const site of sites.filter((site) => site.kind === "main")) starts.push(copyLocation(site));
  return {
    version: 3,
    name: source?.name || "Map",
    description: source?.description || "",
    _design: "Migrated from layout-based map data. Flat map locations are now authoritative.",
    terrain: source?.terrain || [],
    startLocations: starts,
    baseSites: sites.map(copyLocation),
  };
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
function normalizeLocations(locations, size) {
  const out = [];
  const seen = new Set();
  for (const location of Array.isArray(locations) ? locations : []) {
    const valid = validMapTile(location, size);
    if (valid && !seen.has(locationKey(valid))) { seen.add(locationKey(valid)); out.push(valid); }
  }
  return out;
}
function normalizeBaseSites(baseSites, startLocations, size) {
  const normalized = normalizeLocations(baseSites, size);
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
  return [...retainedStarts, ...retainedBases, ...missingStarts.map(copyLocation)];
}
function normalizedDraftTile(draft, tile, radius) {
  const size = draft?.terrain?.length || 0;
  const valid = validMapTile(tile, size);
  if (!valid || size <= radius * 2) return null;
  return { x: Math.max(radius, Math.min(size - radius - 1, valid.x)), y: Math.max(radius, Math.min(size - radius - 1, valid.y)) };
}
function validMapTile(tile, size) {
  const x = Math.trunc(Number(tile?.x)); const y = Math.trunc(Number(tile?.y));
  return Number.isInteger(x) && Number.isInteger(y) && x >= 0 && y >= 0 && x < size && y < size ? { x, y } : null;
}
function transformMapTile(tile, size, transform) {
  if (!tile) return null;
  const max = size - 1;
  if (transform === "horizontal") return { x: tile.x, y: max - tile.y };
  if (transform === "vertical") return { x: max - tile.x, y: tile.y };
  if (transform === "rotate90") return { x: max - tile.y, y: tile.x };
  if (transform === "rotate180") return { x: max - tile.x, y: max - tile.y };
  if (transform === "rotate270") return { x: tile.y, y: max - tile.x };
  if (transform === "diagonalMain") return { x: tile.y, y: tile.x };
  if (transform === "diagonalAnti") return { x: max - tile.y, y: max - tile.x };
  return copyLocation(tile);
}
function normalizeMapEditorSymmetry(value) { return SYMMETRY_TRANSFORMS[value] ? value : MAP_EDITOR_SYMMETRY.NONE; }
function locationKey(location) { return `${location?.x},${location?.y}`; }
function sameLocation(a, b) { return !!a && !!b && a.x === b.x && a.y === b.y; }
function copyLocation(location) { return { x: Number(location?.x), y: Number(location?.y) }; }
function sameLocationSet(left, right) {
  const a = new Set((left || []).map(locationKey)); const b = new Set((right || []).map(locationKey));
  return a.size === b.size && [...a].every((key) => b.has(key));
}
function sameFlatArray(left, right) { return Array.isArray(left) && Array.isArray(right) && left.length === right.length && left.every((value, index) => value === right[index]); }
function draftFingerprint(draft) { return JSON.stringify(draft); }
function clone(value) { return structuredCloneSafe(value); }
function structuredCloneSafe(value) { return typeof structuredClone === "function" ? structuredClone(value) : JSON.parse(JSON.stringify(value)); }
function replaceObject(target, source) { for (const key of Object.keys(target)) delete target[key]; Object.assign(target, clone(source)); }
function positiveInteger(value) { const number = Math.trunc(Number(value)); return Number.isInteger(number) && number > 0 ? number : 0; }
function clampTile(value, size) { return Math.max(0, Math.min(size - 1, Math.trunc(value))); }
function draftEditError(error) { return { ok: false, error }; }
function storageKey(key) { return `rts.map-editor.v3.${String(key || "default")}`; }
function legacyStorageKey(key) { return `rts.mapEditor.${String(key || "default")}.v2`; }
function defaultStorage() { try { return globalThis.localStorage || null; } catch { return null; } }
