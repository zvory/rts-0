import { TERRAIN } from "./protocol.js";

export const MAP_EDITOR_HISTORY_LIMIT = 25;
export const MAP_EDITOR_MAX_NATURALS_PER_PLAYER = 3;
// Mirror the authored-map clearance contract enforced by
// server/crates/sim/src/game/map.rs. The Lab runtime's smaller spawn-reset
// footprint is not sufficient for a map that must pass authored-map validation.
export const MAP_EDITOR_MAIN_CLEARANCE_TILES = 7;
export const MAP_EDITOR_NATURAL_CLEARANCE_TILES = 4;
export const MAP_EDITOR_SYMMETRY = Object.freeze({
  NONE: "none",
  HORIZONTAL: "horizontal",
  VERTICAL: "vertical",
  RADIAL: "radial",
  DIAGONAL_MAIN: "diagonalMain",
  DIAGONAL_ANTI: "diagonalAnti",
});

const TERRAIN_TO_CHAR = Object.freeze({
  [TERRAIN.GRASS]: ".",
  [TERRAIN.ROCK]: "#",
  [TERRAIN.WATER]: "~",
});

const CHAR_TO_TERRAIN = Object.freeze({
  ".": TERRAIN.GRASS,
  "#": TERRAIN.ROCK,
  "~": TERRAIN.WATER,
});

const SYMMETRY_TRANSFORMS = Object.freeze({
  [MAP_EDITOR_SYMMETRY.NONE]: ["identity"],
  [MAP_EDITOR_SYMMETRY.HORIZONTAL]: ["identity", "horizontal"],
  [MAP_EDITOR_SYMMETRY.VERTICAL]: ["identity", "vertical"],
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
    this.selectedLayoutId = "";
    this.lastAction = "";
    this.savedFingerprint = "";
    this.terrainStroke = null;
  }

  get initialized() {
    return !!this.draft;
  }

  get activeLayout() {
    return layoutById(this.draft, this.selectedLayoutId);
  }

  initializeFromStart(startPayload, { name = "Map" } = {}) {
    if (this.draft) return false;
    const map = startPayload?.map || {};
    const size = Number(map.width);
    if (!Number.isInteger(size) || size <= 0 || Number(map.height) !== size) return false;
    const starts = (startPayload?.players || []).map((player) => ({
      x: Number(player.startTileX),
      y: Number(player.startTileY),
    }));
    this.draft = authoredMapFromMaterialized({
      name,
      description: "Map imported from an authoritative session.",
      size,
      terrain: map.terrain,
      starts,
      expansionSites: [],
    });
    this.ensureSelectedLayout();
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
      expansionSites: data.expansionSites,
    });
    this.ensureSelectedLayout();
    this.undoStack = [];
    this.redoStack = [];
    this.markSaved({ notify: false });
    this.notify("initialized");
    return true;
  }

  initializeBlank({ size = 126, playerCount = 2, name = "Untitled map" } = {}) {
    const mapSize = Math.max(16, Math.min(126, Math.trunc(Number(size)) || 126));
    const count = Math.max(1, Math.min(4, Math.trunc(Number(playerCount)) || 2));
    const corners = [
      { x: Math.floor(mapSize * 0.25), y: Math.floor(mapSize * 0.25) },
      { x: Math.floor(mapSize * 0.75), y: Math.floor(mapSize * 0.75) },
      { x: Math.floor(mapSize * 0.75), y: Math.floor(mapSize * 0.25) },
      { x: Math.floor(mapSize * 0.25), y: Math.floor(mapSize * 0.75) },
    ].slice(0, count);
    this.draft = authoredMapFromMaterialized({
      name,
      description: "",
      size: mapSize,
      terrain: Array(mapSize * mapSize).fill(TERRAIN.GRASS),
      starts: corners,
      expansionSites: [],
    });
    this.undoStack = [];
    this.redoStack = [];
    this.ensureSelectedLayout();
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
      throw new Error(
        `This session uses a ${requiredSize} × ${requiredSize} map; ${draft.name} is ${draft.terrain.length} × ${draft.terrain.length}.`,
      );
    }
    const requiredPlayers = positiveInteger(playerCount);
    const compatibleLayouts = requiredPlayers
      ? draft.layouts.filter((layout) => layout.slots.length === requiredPlayers)
      : draft.layouts;
    if (compatibleLayouts.length === 0) {
      throw new Error(`${draft.name} has no ${requiredPlayers}-player layout.`);
    }

    this.draft = draft;
    this.selectedLayoutId = compatibleLayouts[0].id;
    this.undoStack = [];
    this.redoStack = [];
    this.lastAction = `Loaded ${draft.name}`;
    this.notify("loaded");
    return true;
  }

  selectLayout(layoutId) {
    const layout = this.draft?.layouts?.find((candidate) => candidate.id === layoutId) || null;
    if (!layout || layout.id === this.selectedLayoutId) return false;
    this.selectedLayoutId = layout.id;
    this.lastAction = `Selected ${layout.id} layout`;
    this.notify("layout");
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
      selectedLayoutId: this.selectedLayoutId,
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
    this.ensureSelectedLayout();
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
    this.ensureSelectedLayout();
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
    this.ensureSelectedLayout();
    this.lastAction = "Redo";
    this.notify("redo");
    return true;
  }

  setDesiredTool(tool) {
    this.desiredTool = tool ? clone(tool) : null;
    this.notify("tool");
  }

  get hasUnsavedChanges() {
    return !!this.draft && draftFingerprint(this.draft, this.selectedLayoutId) !== this.savedFingerprint;
  }

  markSaved({
    notify = true,
    draft = this.draft,
    selectedLayoutId = this.selectedLayoutId,
  } = {}) {
    if (!draft) return false;
    this.savedFingerprint = draftFingerprint(draft, selectedLayoutId);
    if (notify) this.notify("saved");
    return true;
  }

  beginTerrainStroke(label = "Painted terrain") {
    if (!this.draft || this.terrainStroke) return false;
    this.terrainStroke = {
      label,
      before: clone(this.draft),
      dirty: new Map(),
    };
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

  addLayout(playerCount = 2) {
    if (!this.draft) return false;
    const count = Math.max(1, Math.min(4, Math.trunc(Number(playerCount)) || 2));
    const source = this.activeLayout?.slots || [];
    const idBase = `${count}p`;
    let suffix = 1;
    let id = idBase;
    const ids = new Set(this.draft.layouts.map((layout) => layout.id));
    while (ids.has(id)) id = `${idBase}-${++suffix}`;
    return this.mutate(`Added ${count}-player layout`, (draft) => {
      const slots = Array.from({ length: count }, (_, index) => ({
        main: source[index]?.main || "",
        naturals: [...(source[index]?.naturals || [])],
      }));
      draft.layouts.push({ id, playerCount: count, slots });
      this.selectedLayoutId = id;
    });
  }

  removeSelectedLayout() {
    if (!this.draft || this.draft.layouts.length <= 1) return false;
    const selected = this.selectedLayoutId;
    return this.mutate(`Removed ${selected} layout`, (draft) => {
      draft.layouts = draft.layouts.filter((layout) => layout.id !== selected);
      removeUnreferencedDraftSites(draft);
    });
  }

  /** A player-centred read model for the active authored layout and map overlay. */
  playerSlots() {
    return draftPlayerSlots(this.draft, this.selectedLayoutId);
  }

  /** Persistent, browser-local markers for authored starts and natural bases. */
  mapOverlay() {
    if (!this.draft) return null;
    return {
      players: this.playerSlots().map((slot) => ({
        playerIndex: slot.playerIndex,
        start: slot.start ? { x: slot.start.x, y: slot.start.y } : null,
        naturals: slot.naturals.map((site) => ({ x: site.x, y: site.y })),
      })),
    };
  }

  saveLocal(key) {
    if (!this.draft || !this.storage?.setItem) return false;
    try {
      this.storage.setItem(storageKey(key), JSON.stringify({
        schemaVersion: 2,
        draft: this.draft,
        selectedLayoutId: this.selectedLayoutId,
      }));
    } catch {
      return false;
    }
    this.lastAction = "Saved local map";
    this.markSaved();
    return true;
  }

  loadLocal(key) {
    if (!this.storage?.getItem) return false;
    let text;
    try {
      text = this.storage.getItem(storageKey(key));
    } catch {
      return false;
    }
    if (!text) return false;
    let parsed;
    let selectedLayoutId = "";
    try {
      parsed = JSON.parse(text);
      if (parsed?.schemaVersion === 2 && parsed?.draft) {
        selectedLayoutId = String(parsed.selectedLayoutId || "");
        parsed = parsed.draft;
      }
      normalizeDraft(parsed);
    } catch {
      return false;
    }
    if (!this.draft) {
      this.draft = parsed;
      this.selectedLayoutId = selectedLayoutId;
      this.ensureSelectedLayout();
      this.lastAction = "Loaded local map";
      this.markSaved({ notify: false });
      this.notify("loaded");
      return true;
    }
    this.mutate("Loaded local map", (draft) => replaceObject(draft, parsed));
    if (selectedLayoutId) this.selectLayout(selectedLayoutId);
    this.markSaved({ notify: false });
    return true;
  }

  materialized() {
    if (!this.draft) throw new Error("Map is not initialized.");
    const draft = clone(this.draft);
    normalizeDraft(draft);
    const layout = layoutById(draft, this.selectedLayoutId);
    if (!layout) throw new Error("Map needs a player layout.");
    const byId = new Map(draft.sites.map((site) => [site.id, site]));
    const starts = layout.slots.map((slot) => tileForSite(byId, slot.main, "main"));
    const expansionSites = [];
    const usedNaturals = new Set();
    for (const slot of layout.slots) {
      for (const id of slot.naturals) {
        if (usedNaturals.has(id)) continue;
        usedNaturals.add(id);
        expansionSites.push(tileForSite(byId, id, "natural"));
      }
    }
    return {
      name: draft.name,
      size: draft.terrain.length,
      terrain: draft.terrain.flatMap((row) => [...row].map((ch) => CHAR_TO_TERRAIN[ch])),
      starts,
      expansionSites,
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

  ensureSelectedLayout() {
    this.selectedLayoutId = this.activeLayout?.id || this.draft?.layouts?.[0]?.id || "";
  }
}

/** Expand tile positions through a map-wide symmetry group, with stable de-duplication. */
export function symmetricMapTiles(size, tiles, symmetry = MAP_EDITOR_SYMMETRY.NONE) {
  const mapSize = positiveInteger(size);
  if (!mapSize || !Array.isArray(tiles)) return [];
  const transforms = SYMMETRY_TRANSFORMS[normalizeMapEditorSymmetry(symmetry)];
  const seen = new Set();
  const expanded = [];
  for (const tile of tiles) {
    const source = validMapTile(tile, mapSize);
    if (!source) continue;
    for (const transform of transforms) {
      const transformed = transformMapTile(source, mapSize, transform);
      if (!transformed) continue;
      const key = `${transformed.x},${transformed.y}`;
      if (seen.has(key)) continue;
      seen.add(key);
      expanded.push(transformed);
    }
  }
  return expanded;
}

/** Return every inclusive tile in a drag rectangle, bounded to the square authored map. */
export function mapEditorRectTiles(first, last, size) {
  const mapSize = positiveInteger(size);
  const start = validMapTile(first, mapSize);
  const end = validMapTile(last, mapSize);
  if (!start || !end) return [];
  const x0 = Math.min(start.x, end.x);
  const x1 = Math.max(start.x, end.x);
  const y0 = Math.min(start.y, end.y);
  const y1 = Math.max(start.y, end.y);
  const tiles = [];
  for (let y = y0; y <= y1; y++) {
    for (let x = x0; x <= x1; x++) tiles.push({ x, y });
  }
  return tiles;
}

/**
 * Move an authored start or natural and every already-matching counterpart in
 * the selected layout. The move is atomic, including a swap of counterpart tiles.
 */
export function moveSymmetricDraftBase(draft, {
  kind,
  playerIndex,
  naturalId = "",
  tile,
  layoutId = "",
  symmetry = MAP_EDITOR_SYMMETRY.NONE,
} = {}) {
  const expectedKind = kind === "natural" ? "natural" : "main";
  const radius = expectedKind === "main"
    ? MAP_EDITOR_MAIN_CLEARANCE_TILES
    : MAP_EDITOR_NATURAL_CLEARANCE_TILES;
  const target = normalizedDraftTile(draft, tile, radius);
  const layout = layoutById(draft, layoutId);
  const source = draftBaseBinding(draft, layout, expectedKind, playerIndex, naturalId);
  if (!target || !source) return draftEditError("Choose a valid base and map tile.");

  const mapSize = draft.terrain.length;
  const bindings = draftBaseBindings(draft, layout, expectedKind);
  const plans = [];
  const seenSiteIds = new Set();
  for (const transform of SYMMETRY_TRANSFORMS[normalizeMapEditorSymmetry(symmetry)]) {
    const from = transformMapTile(source.site, mapSize, transform);
    const mirroredTarget = transformMapTile(target, mapSize, transform);
    const binding = bindings.find((candidate) => (
      candidate.site.x === from?.x && candidate.site.y === from?.y
    ));
    if (!binding || !mirroredTarget || seenSiteIds.has(binding.site.id)) continue;
    seenSiteIds.add(binding.site.id);
    if (binding.site.x === mirroredTarget.x && binding.site.y === mirroredTarget.y) continue;
    plans.push({ binding, target: mirroredTarget });
  }
  if (plans.length === 0) return { ok: true, count: 0 };

  const plannedSiteIds = new Set(plans.map(({ binding }) => binding.site.id));
  const plannedTargets = new Set();
  for (const plan of plans) {
    const targetKey = `${plan.target.x},${plan.target.y}`;
    if (plannedTargets.has(targetKey)) {
      return draftEditError("That symmetric move would place multiple bases on the same tile.");
    }
    plannedTargets.add(targetKey);
    const occupied = siteAt(draft, plan.target.x, plan.target.y);
    if (occupied && !plannedSiteIds.has(occupied.id)) {
      return draftEditError("A start or natural base already uses that tile.");
    }
    if (occupied && draftSiteReferenceCount(draft, occupied.id) > 1) {
      return draftEditError("A shared base from another layout already uses that tile.");
    }
  }

  for (const plan of plans) {
    const site = detachDraftBaseBinding(draft, layout, plan.binding);
    if (!site) return draftEditError("That base is no longer part of this layout.");
    site.x = plan.target.x;
    site.y = plan.target.y;
  }
  return { ok: true, count: plans.length };
}

/** Add a natural for the selected player and any already-matching symmetric players. */
export function addSymmetricDraftNaturals(draft, {
  playerIndex,
  tile,
  layoutId = "",
  symmetry = MAP_EDITOR_SYMMETRY.NONE,
} = {}) {
  const layout = layoutById(draft, layoutId);
  const target = normalizedDraftTile(draft, tile, MAP_EDITOR_NATURAL_CLEARANCE_TILES);
  const source = draftBaseBinding(draft, layout, "main", playerIndex);
  if (!target || !source) return draftEditError("Choose a valid player and map tile.");

  const mapSize = draft.terrain.length;
  const plans = [];
  const seenPlayers = new Set();
  for (const transform of SYMMETRY_TRANSFORMS[normalizeMapEditorSymmetry(symmetry)]) {
    const from = transformMapTile(source.site, mapSize, transform);
    const mirroredTarget = transformMapTile(target, mapSize, transform);
    const counterpart = draftBaseBindings(draft, layout, "main")
      .find((candidate) => candidate.site.x === from?.x && candidate.site.y === from?.y);
    if (!counterpart || !mirroredTarget || seenPlayers.has(counterpart.playerIndex)) continue;
    seenPlayers.add(counterpart.playerIndex);
    plans.push({ playerIndex: counterpart.playerIndex, target: mirroredTarget });
  }
  if (plans.length === 0) return draftEditError("That player is no longer part of this layout.");

  const targetKeys = new Set();
  for (const plan of plans) {
    const slot = layout.slots[plan.playerIndex];
    const key = `${plan.target.x},${plan.target.y}`;
    if (slot.naturals.length >= MAP_EDITOR_MAX_NATURALS_PER_PLAYER) {
      return draftEditError(`Player ${plan.playerIndex + 1} already has ${MAP_EDITOR_MAX_NATURALS_PER_PLAYER} natural bases.`);
    }
    if (targetKeys.has(key) || siteAt(draft, plan.target.x, plan.target.y)) {
      return draftEditError("A start or natural base already uses that tile.");
    }
    targetKeys.add(key);
  }
  for (const plan of plans) {
    const result = addDraftPlayerNatural(draft, plan.playerIndex, plan.target, layout.id);
    if (!result.ok) return result;
  }
  return { ok: true, count: plans.length };
}

export function paintDraftRect(draft, rect, terrainCode) {
  const ch = TERRAIN_TO_CHAR[terrainCode];
  if (!ch || !Array.isArray(draft?.terrain) || draft.terrain.length === 0) return;
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
  for (const site of draft.sites || []) {
    const radius = siteClearanceRadius(site.kind);
    paintDraftRect(draft, {
      x0: site.x - radius,
      y0: site.y - radius,
      x1: site.x + radius,
      y1: site.y + radius,
    }, TERRAIN.GRASS);
  }
}

/** Compatibility helper for authored-map import tooling; the draft UI is player-centred. */
export function placeDraftSite(draft, { kind, x, y, layoutId = "" }) {
  const normalizedKind = kind === "natural" ? "natural" : "main";
  const existing = siteAt(draft, x, y);
  if (existing) return existing.id;
  const id = uniqueDraftSiteId(draft, normalizedKind);
  draft.sites.push({ id, kind: normalizedKind, x, y });
  if (normalizedKind === "main") {
    const slot = layoutById(draft, layoutId)?.slots.find((candidate) => !candidate.main);
    if (slot) slot.main = id;
  }
  return id;
}

/** Move one player's start instead of exposing an anonymous "main" site. */
export function moveDraftPlayerStart(draft, playerIndex, tile, layoutId = "") {
  const slot = draftSlotAt(draft, playerIndex, layoutId);
  const target = normalizedDraftTile(draft, tile, MAP_EDITOR_MAIN_CLEARANCE_TILES);
  if (!slot || !target) return draftEditError("Choose a valid player and map tile.");
  const current = siteById(draft, slot.main);
  const occupied = siteAt(draft, target.x, target.y);
  if (occupied && occupied.id !== current?.id) {
    return draftEditError("A start or natural base already uses that tile.");
  }
  if (current?.kind === "main" && current.x === target.x && current.y === target.y) {
    return { ok: true, id: current.id };
  }
  if (current?.kind === "main" && draftSiteReferenceCount(draft, current.id) > 1) {
    const id = uniqueDraftSiteId(draft, "main");
    draft.sites.push({ id, kind: "main", x: target.x, y: target.y });
    slot.main = id;
    return { ok: true, id };
  }
  if (current?.kind === "main") {
    current.x = target.x;
    current.y = target.y;
    return { ok: true, id: current.id };
  }
  const id = uniqueDraftSiteId(draft, "main");
  draft.sites.push({ id, kind: "main", x: target.x, y: target.y });
  slot.main = id;
  return { ok: true, id };
}

/** Add a natural directly to one player's setup, capped at three per player. */
export function addDraftPlayerNatural(draft, playerIndex, tile, layoutId = "") {
  const slot = draftSlotAt(draft, playerIndex, layoutId);
  const target = normalizedDraftTile(draft, tile, MAP_EDITOR_NATURAL_CLEARANCE_TILES);
  if (!slot || !target) return draftEditError("Choose a valid player and map tile.");
  if (slot.naturals.length >= MAP_EDITOR_MAX_NATURALS_PER_PLAYER) {
    return draftEditError(`Player ${playerIndex + 1} already has ${MAP_EDITOR_MAX_NATURALS_PER_PLAYER} natural bases.`);
  }
  const occupied = siteAt(draft, target.x, target.y);
  if (occupied) {
    return draftEditError("A start or natural base already uses that tile.");
  }
  const id = uniqueDraftSiteId(draft, "natural");
  draft.sites.push({ id, kind: "natural", x: target.x, y: target.y });
  slot.naturals.push(id);
  return { ok: true, id };
}

/** Move a named natural that already belongs to the selected player. */
export function moveDraftPlayerNatural(draft, playerIndex, naturalId, tile, layoutId = "") {
  const slot = draftSlotAt(draft, playerIndex, layoutId);
  const target = normalizedDraftTile(draft, tile, MAP_EDITOR_NATURAL_CLEARANCE_TILES);
  const natural = siteById(draft, naturalId);
  if (!slot || !target || natural?.kind !== "natural" || !slot.naturals.includes(naturalId)) {
    return draftEditError("That natural base is no longer part of this player's setup.");
  }
  const occupied = siteAt(draft, target.x, target.y);
  if (occupied && occupied.id !== natural.id) {
    return draftEditError("A start or natural base already uses that tile.");
  }
  if (natural.x === target.x && natural.y === target.y) {
    return { ok: true, id: natural.id };
  }
  if (draftSiteReferenceCount(draft, natural.id) > 1) {
    const id = uniqueDraftSiteId(draft, "natural");
    draft.sites.push({ id, kind: "natural", x: target.x, y: target.y });
    slot.naturals = slot.naturals.map((candidate) => candidate === natural.id ? id : candidate);
    return { ok: true, id };
  }
  natural.x = target.x;
  natural.y = target.y;
  return { ok: true, id: natural.id };
}

/** Remove a natural while keeping all player starts intact. */
export function removeDraftPlayerNatural(draft, playerIndex, naturalId, layoutId = "") {
  const slot = draftSlotAt(draft, playerIndex, layoutId);
  const natural = siteById(draft, naturalId);
  if (!slot || natural?.kind !== "natural" || !slot.naturals.includes(naturalId)) {
    return draftEditError("That natural base is no longer part of this player's setup.");
  }
  slot.naturals = slot.naturals.filter((id) => id !== naturalId);
  if (draftSiteReferenceCount(draft, naturalId) === 0) {
    draft.sites = draft.sites.filter((site) => site.id !== naturalId);
  }
  return { ok: true, id: naturalId };
}

export function removeDraftSite(draft, siteId) {
  draft.sites = draft.sites.filter((site) => site.id !== siteId);
  for (const layout of draft.layouts || []) {
    for (const slot of layout.slots || []) {
      if (slot.main === siteId) slot.main = "";
      slot.naturals = slot.naturals.filter((id) => id !== siteId);
    }
  }
}

export function authoredMapFromMaterialized({ name, description, size, terrain, starts, expansionSites }) {
  const mapSize = Math.max(1, Math.trunc(Number(size)) || 1);
  const codes = Array.from(terrain || []);
  const rows = Array.from({ length: mapSize }, (_, y) => (
    Array.from({ length: mapSize }, (_, x) => TERRAIN_TO_CHAR[codes[y * mapSize + x]] || ".").join("")
  ));
  const mainSites = normalizeTiles(starts).map((tile, index) => ({
    id: `main-${index + 1}`,
    kind: "main",
    x: tile.x,
    y: tile.y,
  }));
  const naturalSites = normalizeTiles(expansionSites).map((tile, index) => ({
    id: `natural-${index + 1}`,
    kind: "natural",
    x: tile.x,
    y: tile.y,
  }));
  const slots = mainSites.map((site) => ({ main: site.id, naturals: [] }));
  for (const natural of naturalSites) {
    const candidates = slots
      .map((slot, index) => ({ slot, index, main: mainSites[index] }))
      .filter(({ slot }) => slot.naturals.length < MAP_EDITOR_MAX_NATURALS_PER_PLAYER)
      .sort((a, b) => distanceSq(natural, a.main) - distanceSq(natural, b.main));
    candidates[0]?.slot.naturals.push(natural.id);
  }
  const draft = {
    version: 2,
    name: String(name || "Map").trim() || "Map",
    description: String(description || ""),
    _design: "Authored in the dedicated Map Editor.",
    terrain: rows,
    sites: [...mainSites, ...naturalSites],
    layouts: [{ id: `lab-${Math.max(1, slots.length)}p`, playerCount: slots.length, slots }],
  };
  normalizeDraft(draft);
  return draft;
}

/** Compare a Lab round trip while treating expansion-site order as non-semantic editor metadata. */
export function materializedMapsEqual(left, right) {
  if (!left || !right || left.name !== right.name || left.size !== right.size) return false;
  if (!sameFlatArray(left.terrain, right.terrain)) return false;
  if (!sameOrderedTiles(left.starts, right.starts)) return false;
  return sameTileMultiset(left.expansionSites, right.expansionSites);
}

function normalizeDraft(draft) {
  if (!draft || typeof draft !== "object") throw new Error("Map must be an object.");
  draft.version = 2;
  draft.name = String(draft.name || "Map").trim().slice(0, 80) || "Map";
  draft.description = String(draft.description || "").slice(0, 500);
  draft._design = String(draft._design || "Authored in the dedicated Map Editor.");
  if (!Array.isArray(draft.terrain) || draft.terrain.length === 0) throw new Error("Map terrain is empty.");
  const size = draft.terrain.length;
  draft.terrain = draft.terrain.map((row) => {
    const text = String(row);
    if (text.length !== size || [...text].some((ch) => !(ch in CHAR_TO_TERRAIN))) {
      throw new Error("Map terrain must be square and contain only ., #, and ~.");
    }
    return text;
  });
  draft.sites = Array.isArray(draft.sites) ? draft.sites : [];
  const ids = new Set();
  const coords = new Set();
  for (const site of draft.sites) {
    site.id = String(site.id || "").trim();
    site.kind = site.kind === "natural" ? "natural" : "main";
    site.x = clampTile(site.x, size);
    site.y = clampTile(site.y, size);
    if (!site.id || ids.has(site.id)) throw new Error("Base site ids must be unique.");
    const coord = `${site.x},${site.y}`;
    if (coords.has(coord)) throw new Error("Base sites cannot share a tile.");
    ids.add(site.id);
    coords.add(coord);
  }
  if (!Array.isArray(draft.layouts) || draft.layouts.length === 0) {
    throw new Error("Map needs a player layout.");
  }
  const layoutIds = new Set();
  draft.layouts = draft.layouts.map((layout, index) => {
    if (!layout || typeof layout !== "object") throw new Error("Map layouts must be objects.");
    layout.id = String(layout.id || `lab-layout-${index + 1}`).trim();
    if (!layout.id || layoutIds.has(layout.id)) throw new Error("Map layout ids must be unique.");
    layoutIds.add(layout.id);
    layout.slots = Array.isArray(layout.slots) ? layout.slots : [];
    layout.playerCount = layout.slots.length;
    for (const slot of layout.slots) {
      slot.main = String(slot.main || "");
      const legacyNatural = slot.natural == null ? [] : [String(slot.natural)];
      slot.naturals = Array.from(new Set([
        ...legacyNatural,
        ...(Array.isArray(slot.naturals) ? slot.naturals.map(String) : []),
      ]))
        .slice(0, MAP_EDITOR_MAX_NATURALS_PER_PLAYER);
      delete slot.natural;
    }
    return layout;
  });
}

function tileForSite(byId, id, expectedKind) {
  const site = byId.get(id);
  if (!site || site.kind !== expectedKind) {
    throw new Error(`Every player slot needs a valid ${expectedKind} site.`);
  }
  return { x: site.x, y: site.y };
}

function normalizeTiles(tiles) {
  return Array.isArray(tiles)
    ? tiles.filter((tile) => Number.isFinite(Number(tile?.x)) && Number.isFinite(Number(tile?.y)))
      .map((tile) => ({ x: Math.trunc(Number(tile.x)), y: Math.trunc(Number(tile.y)) }))
    : [];
}

function sameFlatArray(left, right) {
  return Array.isArray(left)
    && Array.isArray(right)
    && left.length === right.length
    && left.every((value, index) => value === right[index]);
}

function sameOrderedTiles(left, right) {
  return Array.isArray(left)
    && Array.isArray(right)
    && left.length === right.length
    && left.every((tile, index) => tile?.x === right[index]?.x && tile?.y === right[index]?.y);
}

function sameTileMultiset(left, right) {
  if (!Array.isArray(left) || !Array.isArray(right) || left.length !== right.length) return false;
  const keys = (tiles) => tiles.map((tile) => `${tile?.x},${tile?.y}`).sort();
  return sameFlatArray(keys(left), keys(right));
}

function distanceSq(a, b) {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return dx * dx + dy * dy;
}

function normalizeMapEditorSymmetry(symmetry) {
  return Object.hasOwn(SYMMETRY_TRANSFORMS, symmetry)
    ? symmetry
    : MAP_EDITOR_SYMMETRY.NONE;
}

function validMapTile(tile, size) {
  const x = Math.trunc(Number(tile?.x));
  const y = Math.trunc(Number(tile?.y));
  return Number.isInteger(x) && Number.isInteger(y) && x >= 0 && y >= 0 && x < size && y < size
    ? { x, y }
    : null;
}

function transformMapTile(tile, size, transform) {
  const source = validMapTile(tile, size);
  if (!source) return null;
  if (transform === "horizontal") return { x: source.x, y: size - 1 - source.y };
  if (transform === "vertical") return { x: size - 1 - source.x, y: source.y };
  if (transform === "diagonalMain") return { x: source.y, y: source.x };
  if (transform === "diagonalAnti") return { x: size - 1 - source.y, y: size - 1 - source.x };
  if (transform === "rotate90") return { x: size - 1 - source.y, y: source.x };
  if (transform === "rotate180") return { x: size - 1 - source.x, y: size - 1 - source.y };
  if (transform === "rotate270") return { x: source.y, y: size - 1 - source.x };
  return source;
}

function draftBaseBindings(draft, layout, kind) {
  if (!layout?.slots) return [];
  const bindings = [];
  for (const [playerIndex, slot] of layout.slots.entries()) {
    const ids = kind === "main" ? [slot.main] : slot.naturals || [];
    for (const siteId of ids) {
      const site = siteById(draft, siteId);
      if (site?.kind === kind) bindings.push({ kind, playerIndex, siteId, site });
    }
  }
  return bindings;
}

function draftBaseBinding(draft, layout, kind, playerIndex, naturalId = "") {
  const index = Number(playerIndex);
  const slot = layout?.slots?.[index];
  if (!slot) return null;
  const siteId = kind === "main" ? slot.main : naturalId;
  if (kind === "natural" && !slot.naturals?.includes(siteId)) return null;
  const site = siteById(draft, siteId);
  return site?.kind === kind ? { kind, playerIndex: index, siteId, site } : null;
}

function detachDraftBaseBinding(draft, layout, binding) {
  const site = siteById(draft, binding.siteId);
  const slot = layout?.slots?.[binding.playerIndex];
  if (!site || !slot) return null;
  if (draftSiteReferenceCount(draft, site.id) <= 1) return site;
  const id = uniqueDraftSiteId(draft, binding.kind);
  const detached = { id, kind: binding.kind, x: site.x, y: site.y };
  draft.sites.push(detached);
  if (binding.kind === "main") {
    slot.main = id;
  } else {
    slot.naturals = slot.naturals.map((candidate) => candidate === site.id ? id : candidate);
  }
  return detached;
}

function clampTile(value, size) {
  return Math.max(0, Math.min(size - 1, Math.trunc(Number(value)) || 0));
}

function draftPlayerSlots(draft, layoutId) {
  const slots = layoutById(draft, layoutId)?.slots;
  if (!Array.isArray(slots)) return [];
  return slots.map((slot, playerIndex) => ({
    playerIndex,
    start: cloneSiteForPlayer(siteById(draft, slot.main), "main"),
    naturals: (slot.naturals || [])
      .map((id) => cloneSiteForPlayer(siteById(draft, id), "natural"))
      .filter(Boolean),
  }));
}

function cloneSiteForPlayer(site, expectedKind) {
  if (!site || site.kind !== expectedKind) return null;
  return { id: site.id, kind: site.kind, x: site.x, y: site.y };
}

function draftSlotAt(draft, playerIndex, layoutId) {
  const index = Number(playerIndex);
  const slots = layoutById(draft, layoutId)?.slots;
  return Number.isInteger(index) && index >= 0 && Array.isArray(slots) ? slots[index] || null : null;
}

function normalizedDraftTile(draft, tile, radius = 0) {
  const size = Array.isArray(draft?.terrain) ? draft.terrain.length : 0;
  if (size <= radius * 2 || !Number.isFinite(Number(tile?.x)) || !Number.isFinite(Number(tile?.y))) {
    return null;
  }
  return {
    x: clampTileToRadius(tile.x, size, radius),
    y: clampTileToRadius(tile.y, size, radius),
  };
}

function siteById(draft, id) {
  return draft?.sites?.find((site) => site.id === id) || null;
}

function siteAt(draft, x, y) {
  return draft?.sites?.find((site) => site.x === x && site.y === y) || null;
}

function draftSiteReferenceCount(draft, siteId) {
  let count = 0;
  for (const layout of draft?.layouts || []) {
    for (const slot of layout.slots || []) {
      if (slot.main === siteId) count += 1;
      count += (slot.naturals || []).filter((id) => id === siteId).length;
    }
  }
  return count;
}

function removeUnreferencedDraftSites(draft) {
  const referenced = new Set();
  for (const layout of draft?.layouts || []) {
    for (const slot of layout.slots || []) {
      if (slot.main) referenced.add(slot.main);
      for (const id of slot.naturals || []) referenced.add(id);
    }
  }
  draft.sites = (draft?.sites || []).filter((site) => referenced.has(site.id));
}

function uniqueDraftSiteId(draft, prefix) {
  const used = new Set((draft?.sites || []).map((site) => site.id));
  let index = 1;
  while (used.has(`${prefix}-${index}`)) index += 1;
  return `${prefix}-${index}`;
}

function draftEditError(error) {
  return { ok: false, error };
}

function draftFingerprint(draft, selectedLayoutId) {
  return JSON.stringify({ draft: draft || null, selectedLayoutId: selectedLayoutId || "" });
}

function positiveInteger(value) {
  const number = Math.trunc(Number(value));
  return Number.isInteger(number) && number > 0 ? number : 0;
}

function layoutById(draft, layoutId) {
  const layouts = Array.isArray(draft?.layouts) ? draft.layouts : [];
  return layouts.find((layout) => layout.id === layoutId) || layouts[0] || null;
}

function storageKey(key) {
  return `rts.mapEditor.${String(key || "default")}.v2`;
}

function defaultStorage() {
  try {
    return globalThis.localStorage;
  } catch {
    return null;
  }
}

function protectedTerrainTile(draft, x, y) {
  return (draft?.sites || []).some((site) => {
    const radius = siteClearanceRadius(site.kind);
    return Math.abs(site.x - x) <= radius && Math.abs(site.y - y) <= radius;
  });
}

function siteClearanceRadius(kind) {
  return kind === "natural"
    ? MAP_EDITOR_NATURAL_CLEARANCE_TILES
    : MAP_EDITOR_MAIN_CLEARANCE_TILES;
}

function clampTileToRadius(value, size, radius) {
  return Math.max(radius, Math.min(size - radius - 1, Math.trunc(Number(value)) || 0));
}

function replaceObject(target, source) {
  for (const key of Object.keys(target)) delete target[key];
  Object.assign(target, clone(source));
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}
