import { PLAYER_PALETTE } from "./config.js";
import { TERRAIN } from "./protocol.js";
import { LabPanelWindowChrome } from "./lab_panel_window.js";
import {
  LAB_MAP_MAX_NATURALS_PER_PLAYER,
  addDraftPlayerNatural,
  moveDraftPlayerNatural,
  moveDraftPlayerStart,
  paintDraftRect,
  protectDraftBaseTerrain,
  removeDraftPlayerNatural,
} from "./lab_map_editor_session.js";

const MAP_PANEL_STORAGE_KEY = "rts.labPanel.mapEditor.window.v1";
const MAP_CATALOG_URL = "/maps/catalog";
const FALLBACK_MAPS = Object.freeze([
  {
    file: "default-handcrafted.json",
    name: "Default",
    description: "Four-player three-base map with safer in-base naturals and contested naturals.",
  },
  {
    file: "low-econ.json",
    name: "Low Econ",
    description: "Four-player map with one natural expansion per spawn.",
  },
  {
    file: "no-terrain.json",
    name: "No Terrain",
    description: "All-grass map with the standard spawn layouts.",
  },
]);

export class LabMapEditorPanel {
  constructor({
    root,
    session,
    labClient,
    match,
    startPayload,
    mapName = "Lab map",
    applyLabMapReset = null,
    setLabMapDraftOverlay = null,
    setLabMapDraftTerrainPreview = null,
    fetchImpl = globalThis.fetch?.bind(globalThis),
  }) {
    this.root = root;
    this.session = session;
    this.labClient = labClient;
    this.match = match;
    this.startPayload = startPayload;
    this.mapName = mapName;
    this.applyLabMapReset = applyLabMapReset;
    this.setLabMapDraftOverlay = setLabMapDraftOverlay;
    this.setLabMapDraftTerrainPreview = setLabMapDraftTerrainPreview;
    this.fetchImpl = fetchImpl;
    this.destroyed = false;
    this.selectedTerrain = TERRAIN.ROCK;
    this.selectedPlayerIndex = 0;
    this.lastStatus = session.lastAction
      ? `${session.lastAction}. Restart the test when you are ready to try this draft.`
      : "Edit a map draft here. The current Lab test will not change until you restart it with this draft.";
    this.lastStatusError = false;
    this.applyPending = false;
    this.terrainPreviewSignature = null;
    this.mapCatalog = FALLBACK_MAPS.slice();
    this.mapCatalogError = "";
    this.mapCatalogLoading = false;
    this.mapCatalogRequest = null;
    this.mapLoadPending = false;
    this.selectedMapFile = this.mapCatalog[0]?.file || "";

    this.el = document.createElement("aside");
    this.el.className = "lab-panel lab-map-window";
    this.el.setAttribute("aria-label", "Lab map editor");
    this.root.appendChild(this.el);
    this.chrome = new LabPanelWindowChrome(this.el, {
      storageKey: MAP_PANEL_STORAGE_KEY,
      windowObj: globalThis.window,
    });

    this.onKeyDown = (event) => this.handleKeyDown(event);
    globalThis.window?.addEventListener?.("keydown", this.onKeyDown);
    this.unsubscribe = this.session.subscribe(() => {
      this.syncDraftOverlay();
      this.syncDraftTerrainPreview();
      this.render();
    });
    this.session.initializeFromStart(startPayload, { name: mapName });
    this.ensureCompatibleLayout();
    this.hydrateInitialMap();
    this.restoreDesiredTool();
    void this.loadMapCatalog();
  }

  async hydrateInitialMap() {
    if (this.session.undoStack.length > 0 || this.session.lastAction) return;
    const result = await this.labClient.exportScenario("Lab map editor bootstrap");
    if (this.destroyed || !result?.ok || !result?.outcome?.scenario) return;
    if (this.session.undoStack.length > 0 || this.session.lastAction) return;
    if (this.session.initializeFromScenario(result.outcome.scenario, { force: true })) {
      this.ensureCompatibleLayout();
      this.lastStatus = "Loaded the current Lab map as a draft.";
      this.render();
    }
  }

  render() {
    if (this.destroyed || !this.el) return;
    this.el.replaceChildren();
    this.el.appendChild(this.chrome.renderHeader({ kicker: "Map editor", collapseLabel: "map editor" }));
    const body = document.createElement("div");
    body.className = "lab-panel-body lab-map-editor";
    if (!this.session.draft) {
      body.appendChild(readout("Loading current map…"));
    } else {
      body.append(
        this.renderMapLoader(),
        this.renderHistory(),
        this.renderMetadata(),
        this.renderTerrainTools(),
        this.renderPlayerSetup(),
        this.renderDraftActions(),
        this.renderStatus(),
      );
    }
    this.el.append(body, this.chrome.renderResizeHandle());
  }

  renderMapLoader() {
    const fieldset = group("Load map");
    const selected = this.mapCatalog.find((entry) => entry.file === this.selectedMapFile)
      || this.mapCatalog[0]
      || null;
    const label = document.createElement("label");
    label.className = "lab-field";
    const text = document.createElement("span");
    text.textContent = "Built-in map";
    const select = document.createElement("select");
    for (const entry of this.mapCatalog) {
      const option = document.createElement("option");
      option.value = entry.file;
      option.textContent = entry.name;
      option.title = entry.description;
      select.appendChild(option);
    }
    select.value = selected?.file || "";
    select.disabled = this.mapLoadPending;
    select.addEventListener("change", () => {
      this.selectedMapFile = select.value;
      this.render();
    });
    label.append(text, select);
    fieldset.append(
      label,
      button("Load selected map", () => {
        this.selectedMapFile = select.value;
        void this.loadCatalogMap(select.value);
      }, { disabled: !selected || this.mapLoadPending || this.applyPending }),
      button(this.mapCatalogLoading ? "Refreshing maps…" : "Refresh maps", () => {
        void this.loadMapCatalog();
      }, { disabled: this.mapCatalogLoading || this.mapLoadPending }),
      readout(selected?.description || "No built-in maps are available."),
      readout(`Loading a built-in map replaces only this draft. Its ${this.labPlayerCount()}-player layout will be selected; restart the test to use it.`),
    );
    if (this.mapCatalogError) fieldset.appendChild(readout(this.mapCatalogError));
    return fieldset;
  }

  renderHistory() {
    const row = document.createElement("section");
    row.className = "lab-map-history";
    row.append(
      button("Undo", () => this.undo(), { disabled: !this.session.undoStack.length, title: "Ctrl/Cmd-Z" }),
      button("Redo", () => this.redo(), { disabled: !this.session.redoStack.length, title: "Ctrl/Cmd-Shift-Z or Ctrl-Y" }),
      readout(`${this.session.undoStack.length}/25 states`),
    );
    return row;
  }

  renderMetadata() {
    const draft = this.session.draft;
    const fieldset = group("Map details");
    fieldset.append(
      textField("Name", draft.name, (value) => {
        this.session.mutate("Renamed map", (next) => { next.name = value; });
      }),
      textField("Description", draft.description, (value) => {
        this.session.mutate("Changed map description", (next) => { next.description = value; });
      }),
    );
    return fieldset;
  }

  renderTerrainTools() {
    const fieldset = group("Terrain paint");
    const palette = document.createElement("div");
    palette.className = "lab-map-palette";
    for (const [code, label] of [
      [TERRAIN.GRASS, "Grass"],
      [TERRAIN.ROCK, "Stone"],
      [TERRAIN.WATER, "Water"],
    ]) {
      palette.appendChild(terrainPaletteButton(code, label, () => {
        this.selectedTerrain = code;
        this.armTerrainTool();
      }, { active: this.selectedTerrain === code && this.desiredToolKind() === "terrain" }));
    }
    fieldset.append(
      palette,
      readout("Click or drag to paint one tile at a time. Protected base circles remain grass."),
    );
    return fieldset;
  }

  renderPlayerSetup() {
    const fieldset = group("Player starts and natural bases");
    const layouts = this.compatibleLayouts();
    const layout = layouts.find((candidate) => candidate.id === this.session.selectedLayoutId) || null;
    if (!layout) {
      fieldset.appendChild(readout(`No ${this.labPlayerCount()}-player layout is available for this Lab test.`));
      return fieldset;
    }
    if (layouts.length > 1) {
      fieldset.append(layoutSelectField("Test layout", layouts, layout.id, (layoutId) => {
        if (!this.session.selectLayout(layoutId)) return;
        this.selectedPlayerIndex = 0;
        this.setStatus(`Selected ${layoutId} for the draft. The test has not changed.`);
      }));
    }
    const players = this.session.playerSlots();
    if (!players.length) {
      fieldset.appendChild(readout("The selected layout does not yet contain player slots."));
      return fieldset;
    }
    this.selectedPlayerIndex = Math.max(0, Math.min(players.length - 1, this.selectedPlayerIndex));
    const selected = players[this.selectedPlayerIndex];
    const playerPicker = document.createElement("div");
    playerPicker.className = "lab-map-player-picker";
    for (const player of players) {
      const start = player.start ? `${player.start.x}, ${player.start.y}` : "not placed";
      const pick = button(
        `Player ${player.playerIndex + 1} · start ${start} · ${player.naturals.length} natural${player.naturals.length === 1 ? "" : "s"}`,
        () => {
          this.selectedPlayerIndex = player.playerIndex;
          this.render();
        },
        { active: player.playerIndex === this.selectedPlayerIndex },
      );
      pick.dataset.playerIndex = String(player.playerIndex);
      pick.style.setProperty(
        "--lab-map-player-color",
        PLAYER_PALETTE[player.playerIndex % PLAYER_PALETTE.length] || "#9aa0a8",
      );
      playerPicker.appendChild(pick);
    }
    const playerNumber = selected.playerIndex + 1;
    const startText = selected.start
      ? `Start: ${selected.start.x}, ${selected.start.y}`
      : "Start: not placed";
    const naturals = document.createElement("div");
    naturals.className = "lab-map-natural-list";
    for (const [index, natural] of selected.naturals.entries()) {
      const row = document.createElement("div");
      row.className = "lab-map-natural";
      const naturalLabel = document.createElement("span");
      naturalLabel.textContent = `Natural ${index + 1}: ${natural.x}, ${natural.y}`;
      row.append(
        naturalLabel,
        button("Move", () => this.armPlayerNaturalTool(natural.id)),
        button("Remove", () => this.removePlayerNatural(natural.id)),
      );
      naturals.appendChild(row);
    }
    fieldset.append(
      playerPicker,
      readout(`Player ${playerNumber} ${startText}. Click a map tool, then click the map. Coloured markers show this draft on the map.`),
      button(`Move Player ${playerNumber} start`, () => this.armPlayerStartTool(), {
        active: this.desiredToolKind() === "start" && this.session.desiredTool?.playerIndex === selected.playerIndex,
      }),
      button(`Add natural for Player ${playerNumber}`, () => this.armPlayerNaturalTool(), {
        active: this.desiredToolKind() === "natural" && this.session.desiredTool?.playerIndex === selected.playerIndex && !this.session.desiredTool?.naturalId,
        disabled: selected.naturals.length >= LAB_MAP_MAX_NATURALS_PER_PLAYER,
      }),
      naturals,
      readout(`Each player can have up to ${LAB_MAP_MAX_NATURALS_PER_PLAYER} natural bases. Starts and natural bases are part of the draft, not live units.`),
    );
    return fieldset;
  }

  renderDraftActions() {
    const fieldset = group("Draft and test");
    const pending = this.session.hasUnappliedChanges;
    fieldset.append(
      readout(pending
        ? "This draft has changes that are not in the current Lab test."
        : "This draft matches the current Lab test."),
      button("Restart test with this draft", () => this.restartTestWithDraft(), {
        disabled: this.applyPending || this.mapLoadPending,
        title: "Replace the current Lab test with a fresh test using this map draft",
      }),
      readout("Restarting the test clears its current units, orders, resources, and elapsed time."),
      button("Save draft on this device", () => this.saveLocal()),
      button("Load saved draft", () => this.loadLocal()),
      button("Export map JSON", () => this.exportJson()),
    );
    return fieldset;
  }

  renderStatus() {
    const status = document.createElement("p");
    status.className = "lab-result";
    status.dataset.state = this.lastStatusError ? "error" : "ok";
    status.setAttribute("aria-live", "polite");
    status.textContent = this.lastStatus;
    return status;
  }

  armTerrainTool() {
    this.session.setDesiredTool({ kind: "terrain", terrain: this.selectedTerrain });
    return this.match?.armLabTool?.({
      kind: "editMapTerrain",
      label: `Paint ${terrainLabel(this.selectedTerrain)} terrain`,
      payload: { terrain: this.selectedTerrain },
      keepArmedOnWorldClick: true,
      paintOnDrag: true,
    }, {
      onWorldClick: (event) => this.paintWorldClick(event),
    });
  }

  armPlayerStartTool() {
    const playerIndex = this.selectedPlayerIndex;
    this.session.setDesiredTool({ kind: "start", playerIndex });
    return this.match?.armLabTool?.({
      kind: "editMapPlayerStart",
      label: `Move Player ${playerIndex + 1} start`,
      payload: { playerIndex },
    }, { onWorldClick: (event) => this.placePlayerStart(event) });
  }

  armPlayerNaturalTool(naturalId = "") {
    const playerIndex = this.selectedPlayerIndex;
    this.session.setDesiredTool({ kind: "natural", playerIndex, naturalId });
    const moving = !!naturalId;
    return this.match?.armLabTool?.({
      kind: "editMapPlayerNatural",
      label: moving ? `Move Player ${playerIndex + 1} natural base` : `Add natural for Player ${playerIndex + 1}`,
      payload: { playerIndex, naturalId },
    }, { onWorldClick: (event) => this.placePlayerNatural(event) });
  }

  restoreDesiredTool() {
    const desired = this.session.desiredTool;
    if (!desired) return;
    if (desired.kind === "terrain") {
      this.selectedTerrain = desired.terrain;
      this.armTerrainTool();
    } else if (desired.kind === "start") {
      this.selectedPlayerIndex = desired.playerIndex;
      this.armPlayerStartTool();
    } else if (desired.kind === "natural") {
      this.selectedPlayerIndex = desired.playerIndex;
      this.armPlayerNaturalTool(desired.naturalId);
    }
  }

  desiredToolKind() {
    return this.session.desiredTool?.kind || "";
  }

  paintWorldClick(event) {
    const tile = this.worldTile(event?.x, event?.y);
    if (!tile) return;
    this.commitDraft("Painted terrain tile", (draft) => {
      paintDraftRect(draft, {
        x0: tile.x,
        y0: tile.y,
        x1: tile.x,
        y1: tile.y,
      }, event.tool.payload.terrain);
      protectDraftBaseTerrain(draft);
    });
  }

  placePlayerStart(event) {
    const tile = this.worldTile(event?.x, event?.y, { start: true });
    if (!tile) return;
    const playerIndex = Number(event?.tool?.payload?.playerIndex);
    this.commitDraft(`Moved Player ${playerIndex + 1} start`, (draft) => {
      const result = moveDraftPlayerStart(draft, playerIndex, tile, this.session.selectedLayoutId);
      if (result.ok) protectDraftBaseTerrain(draft);
      return result;
    });
  }

  placePlayerNatural(event) {
    const tile = this.worldTile(event?.x, event?.y);
    if (!tile) return;
    const playerIndex = Number(event?.tool?.payload?.playerIndex);
    const naturalId = String(event?.tool?.payload?.naturalId || "");
    this.commitDraft(
      naturalId ? `Moved Player ${playerIndex + 1} natural base` : `Added natural for Player ${playerIndex + 1}`,
      (draft) => {
        const result = naturalId
          ? moveDraftPlayerNatural(draft, playerIndex, naturalId, tile, this.session.selectedLayoutId)
          : addDraftPlayerNatural(draft, playerIndex, tile, this.session.selectedLayoutId);
        if (result.ok) protectDraftBaseTerrain(draft);
        return result;
      },
    );
  }

  removePlayerNatural(naturalId) {
    const playerIndex = this.selectedPlayerIndex;
    this.commitDraft(`Removed Player ${playerIndex + 1} natural base`, (draft) => (
      removeDraftPlayerNatural(draft, playerIndex, naturalId, this.session.selectedLayoutId)
    ));
  }

  worldTile(x, y, { start = false } = {}) {
    const tileSize = Number(this.startPayload?.map?.tileSize);
    const size = this.session.draft?.terrain?.length || 0;
    if (!Number.isFinite(x) || !Number.isFinite(y) || !tileSize || !size) return null;
    const radius = start ? 3 : 0;
    if (size <= radius * 2) return null;
    return {
      x: Math.max(radius, Math.min(size - radius - 1, Math.floor(x / tileSize))),
      y: Math.max(radius, Math.min(size - radius - 1, Math.floor(y / tileSize))),
    };
  }

  commitDraft(label, mutation) {
    let result = null;
    const changed = this.session.mutate(label, (draft) => {
      result = mutation(draft);
    });
    if (!changed) {
      this.setStatus(result?.error || "No draft tiles changed. Start footprints and natural centers must remain grass.", !!result?.error);
      return false;
    }
    this.setStatus(`${label}. Restart the test when you are ready to try this draft.`);
    return true;
  }

  async restartTestWithDraft() {
    if (this.applyPending) return null;
    let materialized;
    let submittedDraft;
    let submittedLayoutId;
    try {
      materialized = this.session.materialized();
      submittedDraft = this.session.exportMap();
      submittedLayoutId = this.session.selectedLayoutId;
    } catch (error) {
      this.setStatus(error.message || String(error), true);
      return null;
    }
    this.applyPending = true;
    this.setStatus("Restarting the Lab test with this draft…");
    const result = await this.labClient.applyMapDraft(materialized);
    this.applyPending = false;
    if (!result?.ok) {
      this.setStatus(result?.error || "Map restart failed.", true);
      return result;
    }
    if (!this.applyLabMapReset?.(result.outcome)) {
      this.setStatus("The test restarted, but the local map display could not refresh in place.", true);
      return result;
    }
    // The author may keep editing while the authoritative restart is in flight. Record the
    // submitted draft as the tested baseline, then republish any newer local preview because
    // applyLabMapReset just rebuilt the world terrain from the server result.
    this.terrainPreviewSignature = null;
    this.session.markCurrentDraftAsTested({
      draft: submittedDraft,
      selectedLayoutId: submittedLayoutId,
    });
    this.setStatus("Test restarted with this map draft. Keep editing the draft without changing the test.");
    return result;
  }

  loadMapCatalog() {
    if (this.mapCatalogRequest) return this.mapCatalogRequest;
    if (!this.fetchImpl) {
      this.mapCatalogError = "Map catalog unavailable; standard maps are still available.";
      this.render();
      return Promise.resolve(false);
    }
    this.mapCatalogLoading = true;
    this.mapCatalogError = "";
    this.render();
    this.mapCatalogRequest = (async () => {
      try {
        const response = await this.fetchImpl(MAP_CATALOG_URL, { cache: "no-store" });
        if (!response?.ok) throw new Error(`HTTP ${response?.status || "network"}`);
        const data = await response.json();
        const catalog = normalizeMapCatalog(data?.maps);
        if (catalog.length === 0) throw new Error("no compatible maps");
        this.mapCatalog = catalog;
        if (!catalog.some((entry) => entry.file === this.selectedMapFile)) {
          this.selectedMapFile = catalog[0].file;
        }
        return true;
      } catch {
        this.mapCatalogError = "Map catalog unavailable; standard maps are still available.";
        return false;
      } finally {
        this.mapCatalogLoading = false;
        this.mapCatalogRequest = null;
        this.render();
      }
    })();
    return this.mapCatalogRequest;
  }

  async loadCatalogMap(file) {
    if (this.mapLoadPending) return false;
    const entry = this.mapCatalog.find((candidate) => candidate.file === file);
    if (!entry || !this.fetchImpl) {
      this.setStatus("That built-in map is unavailable.", true);
      return false;
    }
    this.mapLoadPending = true;
    this.setStatus(`Loading ${entry.name}…`);
    try {
      const response = await this.fetchImpl(`/maps/${encodeURIComponent(entry.file)}`, { cache: "no-store" });
      if (!response?.ok) throw new Error(`HTTP ${response?.status || "network"}`);
      const map = await response.json();
      this.session.loadAuthoredMap(map, {
        expectedSize: this.mapSize(),
        playerCount: this.labPlayerCount(),
      });
      this.selectedPlayerIndex = 0;
      this.setStatus(`Loaded ${entry.name} as a map draft. Restart the test to use it.`);
      return true;
    } catch (error) {
      this.setStatus(`Map load failed: ${error.message || String(error)}`, true);
      return false;
    } finally {
      this.mapLoadPending = false;
      this.render();
    }
  }

  undo() {
    if (!this.session.undo()) return;
    this.setStatus("Undid the last draft edit. The test has not changed.");
  }

  redo() {
    if (!this.session.redo()) return;
    this.setStatus("Redid the draft edit. The test has not changed.");
  }

  saveLocal() {
    const ok = this.session.saveLocal(this.mapStorageKey());
    this.setStatus(ok ? "Saved this map draft on this device." : "Saving a local draft is unavailable.", !ok);
  }

  loadLocal() {
    const ok = this.session.loadLocal(this.mapStorageKey());
    if (!ok) {
      this.setStatus("No compatible saved map draft was found.", true);
      return;
    }
    if (!this.ensureCompatibleLayout()) {
      this.setStatus(`The saved draft has no ${this.labPlayerCount()}-player layout for this Lab test.`, true);
      return;
    }
    this.selectedPlayerIndex = 0;
    this.setStatus("Loaded a saved map draft. Restart the test to use it.");
  }

  exportJson() {
    try {
      const draft = this.session.exportMap();
      const blob = new Blob([`${JSON.stringify(draft, null, 2)}\n`], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `${slug(draft.name)}.json`;
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      URL.revokeObjectURL(url);
      this.setStatus(`Exported ${anchor.download}.`);
    } catch (error) {
      this.setStatus(error.message || String(error), true);
    }
  }

  mapStorageKey() {
    return this.startPayload?.lab?.room || this.mapName || "default";
  }

  mapSize() {
    const size = Number(this.startPayload?.map?.width);
    return Number.isInteger(size) && size > 0 ? size : null;
  }

  labPlayerCount() {
    const players = this.startPayload?.players;
    return Array.isArray(players) && players.length > 0 ? players.length : 0;
  }

  compatibleLayouts() {
    const layouts = this.session.draft?.layouts || [];
    const playerCount = this.labPlayerCount();
    return playerCount > 0 ? layouts.filter((layout) => layout.slots.length === playerCount) : layouts;
  }

  ensureCompatibleLayout() {
    const layouts = this.compatibleLayouts();
    if (layouts.length === 0) return false;
    if (!layouts.some((layout) => layout.id === this.session.selectedLayoutId)) {
      this.session.selectLayout(layouts[0].id);
    }
    return true;
  }

  handleKeyDown(event) {
    if (this.destroyed || event.defaultPrevented || isTextEntry(event.target)) return;
    if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
    const key = String(event.key || "").toLowerCase();
    const redo = key === "y" || (key === "z" && event.shiftKey);
    const undo = key === "z" && !event.shiftKey;
    if (!undo && !redo) return;
    event.preventDefault();
    redo ? this.redo() : this.undo();
  }

  setStatus(message, error = false) {
    this.lastStatus = String(message || "");
    this.lastStatusError = !!error;
    this.render();
  }

  syncDraftOverlay() {
    this.setLabMapDraftOverlay?.(this.session.mapOverlay());
  }

  syncDraftTerrainPreview() {
    const previewing = !!this.session.draft && this.session.hasUnappliedChanges;
    const signature = previewing
      ? `draft:${this.session.draft.terrain.join("|")}`
      : "authoritative";
    if (signature === this.terrainPreviewSignature) return;
    this.terrainPreviewSignature = signature;
    if (!previewing) {
      this.setLabMapDraftTerrainPreview?.(null);
      return;
    }
    try {
      this.setLabMapDraftTerrainPreview?.(this.session.materialized());
    } catch {
      this.setLabMapDraftTerrainPreview?.(null);
    }
  }

  applyLabToolChange(change) {
    const kind = change?.tool?.kind || "";
    if (change?.type === "armed" && !["editMapTerrain", "editMapPlayerStart", "editMapPlayerNatural"].includes(kind)) {
      this.session.setDesiredTool(null);
    }
    if (change?.type === "cancelled" && !["panelDestroy", "teardown"].includes(change.reason)) {
      this.session.setDesiredTool(null);
    }
    this.render();
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    globalThis.window?.removeEventListener?.("keydown", this.onKeyDown);
    this.unsubscribe?.();
    this.setLabMapDraftOverlay?.(null);
    this.setLabMapDraftTerrainPreview?.(null);
    this.chrome.destroy();
    this.el.remove();
  }
}

function group(title) {
  const fieldset = document.createElement("fieldset");
  fieldset.className = "lab-tool-group lab-map-group";
  const legend = document.createElement("legend");
  legend.textContent = title;
  fieldset.appendChild(legend);
  return fieldset;
}

function button(label, onClick, { disabled = false, active = false, title = "" } = {}) {
  const el = document.createElement("button");
  el.type = "button";
  el.className = "lab-btn";
  el.textContent = label;
  el.disabled = !!disabled;
  el.dataset.active = active ? "true" : "false";
  if (title) el.title = title;
  el.addEventListener("click", onClick);
  return el;
}

function terrainPaletteButton(code, label, onClick, { active = false } = {}) {
  const el = button("", onClick, {
    active,
    title: `Paint ${label.toLowerCase()} terrain`,
  });
  el.className = "lab-btn lab-map-terrain-option";
  el.dataset.terrain = terrainName(code);
  el.setAttribute("aria-label", `Paint ${label.toLowerCase()} terrain`);

  const icon = document.createElement("span");
  icon.className = "lab-terrain-icon";
  icon.dataset.terrain = terrainName(code);
  icon.setAttribute("aria-hidden", "true");
  const text = document.createElement("span");
  text.className = "lab-terrain-label";
  text.textContent = label;
  el.append(icon, text);
  return el;
}

function terrainName(code) {
  if (code === TERRAIN.ROCK) return "stone";
  if (code === TERRAIN.WATER) return "water";
  return "grass";
}

function terrainLabel(code) {
  if (code === TERRAIN.ROCK) return "Stone";
  if (code === TERRAIN.WATER) return "Water";
  return "Grass";
}

function textField(labelText, value, onChange) {
  const label = document.createElement("label");
  label.className = "lab-field";
  label.dataset.wide = "true";
  const text = document.createElement("span");
  text.textContent = labelText;
  const input = document.createElement("input");
  input.value = value;
  input.addEventListener("change", () => onChange(input.value));
  label.append(text, input);
  return label;
}

function layoutSelectField(labelText, layouts, selected, onChange) {
  const label = document.createElement("label");
  label.className = "lab-field";
  const text = document.createElement("span");
  text.textContent = labelText;
  const select = document.createElement("select");
  for (const layout of layouts) {
    const option = document.createElement("option");
    option.value = layout.id;
    option.textContent = `${layout.id} (${layout.slots.length} players)`;
    option.selected = layout.id === selected;
    select.appendChild(option);
  }
  select.value = selected;
  select.addEventListener("change", () => onChange(select.value));
  label.append(text, select);
  return label;
}

function normalizeMapCatalog(entries) {
  if (!Array.isArray(entries)) return [];
  const files = new Set();
  return entries.flatMap((entry) => {
    const file = String(entry?.file || "").trim();
    if (!safeMapFile(file) || files.has(file)) return [];
    files.add(file);
    const name = String(entry?.name || file.replace(/\.json$/i, "")).trim() || file;
    const description = String(entry?.description || name).trim() || name;
    return [{ file, name, description }];
  });
}

function safeMapFile(file) {
  return /^[a-z0-9][a-z0-9._-]*\.json$/i.test(file) && !file.includes("..");
}

function readout(text) {
  const node = document.createElement("p");
  node.className = "lab-readout";
  node.textContent = text;
  return node;
}

function isTextEntry(target) {
  const tag = String(target?.tagName || "").toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select" || !!target?.isContentEditable;
}

function slug(value) {
  return String(value || "lab-map")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64) || "lab-map";
}
