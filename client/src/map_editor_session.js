import { TERRAIN } from "./protocol.js";

export const MAP_EDITOR_HISTORY_LIMIT = 25;
export const MAP_EDITOR_MAX_NATURALS_PER_PLAYER = 3;

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

export class MapEditorSession {
  constructor({ storage = globalThis.localStorage, historyLimit = MAP_EDITOR_HISTORY_LIMIT } = {}) {
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
    this.storage.setItem(storageKey(key), JSON.stringify({
      schemaVersion: 2,
      draft: this.draft,
      selectedLayoutId: this.selectedLayoutId,
    }));
    this.lastAction = "Saved local map";
    this.markSaved();
    return true;
  }

  loadLocal(key) {
    if (!this.storage?.getItem) return false;
    const text = this.storage.getItem(storageKey(key));
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
    const changed = this.mutate("Loaded local map", (draft) => replaceObject(draft, parsed));
    if (selectedLayoutId) this.selectLayout(selectedLayoutId);
    this.markSaved({ notify: false });
    return changed;
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
    const radius = site.kind === "natural" ? 0 : 3;
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
  const target = normalizedDraftTile(draft, tile);
  if (!slot || !target) return draftEditError("Choose a valid player and map tile.");
  const current = siteById(draft, slot.main);
  const occupied = siteAt(draft, target.x, target.y);
  if (occupied && occupied.id !== current?.id) {
    return draftEditError("A start or natural base already uses that tile.");
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
  const target = normalizedDraftTile(draft, tile);
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
  const target = normalizedDraftTile(draft, tile);
  const natural = siteById(draft, naturalId);
  if (!slot || !target || natural?.kind !== "natural" || !slot.naturals.includes(naturalId)) {
    return draftEditError("That natural base is no longer part of this player's setup.");
  }
  const occupied = siteAt(draft, target.x, target.y);
  if (occupied && occupied.id !== natural.id) {
    return draftEditError("A start or natural base already uses that tile.");
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
  removeDraftSite(draft, naturalId);
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

function distanceSq(a, b) {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return dx * dx + dy * dy;
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

function normalizedDraftTile(draft, tile) {
  const size = Array.isArray(draft?.terrain) ? draft.terrain.length : 0;
  if (!size || !Number.isFinite(Number(tile?.x)) || !Number.isFinite(Number(tile?.y))) return null;
  return { x: clampTile(tile.x, size), y: clampTile(tile.y, size) };
}

function siteById(draft, id) {
  return draft?.sites?.find((site) => site.id === id) || null;
}

function siteAt(draft, x, y) {
  return draft?.sites?.find((site) => site.x === x && site.y === y) || null;
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

function protectedTerrainTile(draft, x, y) {
  return (draft?.sites || []).some((site) => {
    const radius = site.kind === "natural" ? 0 : 3;
    return Math.abs(site.x - x) <= radius && Math.abs(site.y - y) <= radius;
  });
}

function replaceObject(target, source) {
  for (const key of Object.keys(target)) delete target[key];
  Object.assign(target, clone(source));
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}
