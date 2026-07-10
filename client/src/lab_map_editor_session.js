import { TERRAIN } from "./protocol.js";

export const LAB_MAP_HISTORY_LIMIT = 25;

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

export class LabMapEditorSession {
  constructor({ storage = globalThis.localStorage, historyLimit = LAB_MAP_HISTORY_LIMIT } = {}) {
    this.storage = storage;
    this.historyLimit = Math.max(1, Math.trunc(historyLimit) || LAB_MAP_HISTORY_LIMIT);
    this.draft = null;
    this.undoStack = [];
    this.redoStack = [];
    this.subscribers = new Set();
    this.desiredTool = null;
    this.lastAction = "";
  }

  get initialized() {
    return !!this.draft;
  }

  initializeFromStart(startPayload, { name = "Lab map" } = {}) {
    if (this.draft) return false;
    const map = startPayload?.map || {};
    const size = Number(map.width);
    if (!Number.isInteger(size) || size <= 0 || Number(map.height) !== size) return false;
    const starts = (startPayload?.players || []).map((player) => ({
      x: Number(player.startTileX),
      y: Number(player.startTileY),
    }));
    this.draft = draftFromMaterializedMap({
      name,
      description: "Map drafted in the live lab editor.",
      size,
      terrain: map.terrain,
      starts,
      expansionSites: [],
    });
    this.notify("initialized");
    return true;
  }

  initializeFromScenario(scenario, { force = false } = {}) {
    if (this.draft && !force) return false;
    const data = scenario?.map?.data;
    if (!data) return false;
    this.draft = draftFromMaterializedMap({
      name: scenario?.map?.name || scenario?.name || "Lab map",
      description: "Map drafted in the live lab editor.",
      size: data.size,
      terrain: data.terrain,
      starts: data.starts,
      expansionSites: data.expansionSites,
    });
    this.undoStack = [];
    this.redoStack = [];
    this.notify("initialized");
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

  setDesiredTool(tool) {
    this.desiredTool = tool ? clone(tool) : null;
    this.notify("tool");
  }

  saveLocal(key) {
    if (!this.draft || !this.storage?.setItem) return false;
    this.storage.setItem(storageKey(key), JSON.stringify(this.draft));
    this.lastAction = "Saved local draft";
    this.notify("saved");
    return true;
  }

  loadLocal(key) {
    if (!this.storage?.getItem) return false;
    const text = this.storage.getItem(storageKey(key));
    if (!text) return false;
    let parsed;
    try {
      parsed = JSON.parse(text);
      normalizeDraft(parsed);
    } catch {
      return false;
    }
    if (!this.draft) {
      this.draft = parsed;
      this.lastAction = "Loaded local draft";
      this.notify("loaded");
      return true;
    }
    return this.mutate("Loaded local draft", (draft) => replaceObject(draft, parsed));
  }

  materialized() {
    if (!this.draft) throw new Error("Map draft is not initialized.");
    const draft = clone(this.draft);
    normalizeDraft(draft);
    const layout = draft.layouts[0];
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
    if (!this.draft) throw new Error("Map draft is not initialized.");
    const draft = clone(this.draft);
    normalizeDraft(draft);
    return draft;
  }

  notify(reason) {
    const snapshot = { ...this.snapshot(), reason };
    for (const handler of this.subscribers) handler(snapshot);
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

export function placeDraftSite(draft, { kind, x, y }) {
  const normalizedKind = kind === "natural" ? "natural" : "main";
  const existing = draft.sites.find((site) => site.x === x && site.y === y);
  if (existing) return existing.id;
  const prefix = normalizedKind === "main" ? "main" : "natural";
  const used = new Set(draft.sites.map((site) => site.id));
  let index = 1;
  while (used.has(`${prefix}-${index}`)) index += 1;
  const id = `${prefix}-${index}`;
  draft.sites.push({ id, kind: normalizedKind, x, y });
  if (normalizedKind === "main") {
    const slot = draft.layouts[0].slots.find((candidate) => !candidate.main);
    if (slot) slot.main = id;
  }
  return id;
}

export function removeDraftSite(draft, siteId) {
  draft.sites = draft.sites.filter((site) => site.id !== siteId);
  for (const slot of draft.layouts[0].slots) {
    if (slot.main === siteId) slot.main = "";
    slot.naturals = slot.naturals.filter((id) => id !== siteId);
  }
}

function draftFromMaterializedMap({ name, description, size, terrain, starts, expansionSites }) {
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
      .filter(({ slot }) => slot.naturals.length < 3)
      .sort((a, b) => distanceSq(natural, a.main) - distanceSq(natural, b.main));
    candidates[0]?.slot.naturals.push(natural.id);
  }
  const draft = {
    version: 2,
    name: String(name || "Lab map").trim() || "Lab map",
    description: String(description || ""),
    _design: "Proof-of-concept map authored in the live lab editor.",
    terrain: rows,
    sites: [...mainSites, ...naturalSites],
    layouts: [{ id: `lab-${Math.max(1, slots.length)}p`, playerCount: slots.length, slots }],
  };
  normalizeDraft(draft);
  return draft;
}

function normalizeDraft(draft) {
  if (!draft || typeof draft !== "object") throw new Error("Map draft must be an object.");
  draft.version = 2;
  draft.name = String(draft.name || "Lab map").trim().slice(0, 80) || "Lab map";
  draft.description = String(draft.description || "").slice(0, 500);
  draft._design = String(draft._design || "Proof-of-concept map authored in the live lab editor.");
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
    throw new Error("Map draft needs a player layout.");
  }
  const layout = draft.layouts[0];
  layout.id = String(layout.id || "lab-layout");
  layout.slots = Array.isArray(layout.slots) ? layout.slots : [];
  layout.playerCount = layout.slots.length;
  for (const slot of layout.slots) {
    slot.main = String(slot.main || "");
    slot.naturals = Array.from(new Set(Array.isArray(slot.naturals) ? slot.naturals.map(String) : [])).slice(0, 3);
  }
  draft.layouts = [layout];
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

function storageKey(key) {
  return `rts.labMapDraft.${String(key || "default")}.v1`;
}

function replaceObject(target, source) {
  for (const key of Object.keys(target)) delete target[key];
  Object.assign(target, clone(source));
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}
