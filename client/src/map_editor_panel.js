import { PLAYER_PALETTE } from "./config.js";
import { TERRAIN } from "./protocol.js";
import {
  MAP_EDITOR_HISTORY_LIMIT,
  MAP_EDITOR_MAX_NATURALS_PER_PLAYER,
  MAP_EDITOR_SYMMETRY,
  removeDraftPlayerNatural,
} from "./map_editor_session.js";

const MAP_CATALOG_URL = "/maps/catalog";

export class MapEditorPanel {
  constructor({
    root,
    session,
    viewport,
    workspaceId = "default",
    onOpenLab,
    fetchImpl = globalThis.fetch?.bind(globalThis),
  }) {
    this.root = root;
    this.session = session;
    this.viewport = viewport;
    this.workspaceId = workspaceId;
    this.onOpenLab = onOpenLab;
    this.fetchImpl = fetchImpl;
    this.catalog = [];
    this.catalogError = "";
    this.selectedMapFile = "";
    this.selectedPlayerIndex = 0;
    this.selectedTerrain = TERRAIN.ROCK;
    this.paintShape = "brush";
    this.symmetry = MAP_EDITOR_SYMMETRY.NONE;
    this.newLayoutPlayers = 2;
    this.pending = false;
    this.status = "Ready to edit the map.";
    this.statusError = false;
    this.destroyed = false;
    this.el = document.createElement("aside");
    this.el.className = "map-editor-panel";
    this.el.setAttribute("aria-label", "Map Editor controls");
    root.appendChild(this.el);
    this.onKeyDown = (event) => this.handleKeyDown(event);
    window.addEventListener("keydown", this.onKeyDown);
    this.unsubscribe = session.subscribe(() => this.render());
    void this.loadCatalog();
  }

  render() {
    if (this.destroyed) return;
    const previousBody = this.el.querySelector(".map-editor-panel-body");
    const scroll = previousBody && {
      left: previousBody.scrollLeft,
      top: previousBody.scrollTop,
    };
    this.el.replaceChildren();
    const header = document.createElement("header");
    header.className = "map-editor-header";
    const title = document.createElement("h1");
    title.textContent = "Map Editor";
    header.appendChild(title);
    const body = document.createElement("div");
    body.className = "map-editor-panel-body";
    if (!this.session.draft) {
      body.appendChild(readout("Preparing editor…"));
    } else {
      body.append(
        this.renderMapSource(),
        this.renderHistory(),
        this.renderDetails(),
        this.renderTerrain(),
        this.renderLayouts(),
        this.renderPlayers(),
        this.renderActions(),
        this.renderStatus(),
      );
    }
    this.el.append(header, body);
    if (scroll) {
      body.scrollLeft = scroll.left;
      body.scrollTop = scroll.top;
    }
  }

  renderMapSource() {
    const section = group("Map source");
    const select = document.createElement("select");
    select.setAttribute("aria-label", "Bundled map");
    for (const entry of this.catalog) {
      const option = document.createElement("option");
      option.value = entry.file;
      option.textContent = entry.name;
      select.appendChild(option);
    }
    select.value = this.selectedMapFile;
    select.addEventListener("change", () => { this.selectedMapFile = select.value; });
    section.append(
      field("Bundled map", select),
      button("Load bundled map", () => void this.loadBundledMap(), { disabled: !this.selectedMapFile || this.pending }),
      button("New blank 126 × 126", () => this.newBlankMap(), { disabled: this.pending }),
    );
    if (this.catalogError) section.appendChild(readout(this.catalogError, true));
    return section;
  }

  renderHistory() {
    const section = document.createElement("section");
    section.className = "map-editor-history";
    section.append(
      button("Undo", () => this.undo(), { disabled: !this.session.undoStack.length, title: "Ctrl/Cmd-Z" }),
      button("Redo", () => this.redo(), { disabled: !this.session.redoStack.length, title: "Ctrl/Cmd-Shift-Z" }),
      readout(`${this.session.undoStack.length}/${MAP_EDITOR_HISTORY_LIMIT}`),
    );
    return section;
  }

  renderDetails() {
    const section = group("Map details");
    section.append(
      textField("Name", this.session.draft.name, (value) => {
        this.session.mutate("Renamed map", (draft) => { draft.name = value; });
      }),
      textAreaField("Description", this.session.draft.description, (value) => {
        this.session.mutate("Changed description", (draft) => { draft.description = value; });
      }),
    );
    return section;
  }

  renderTerrain() {
    const section = group("Terrain paint");
    const palette = document.createElement("div");
    palette.className = "map-editor-palette";
    for (const [code, label] of [
      [TERRAIN.GRASS, "Grass / erase"],
      [TERRAIN.ROCK, "Stone"],
      [TERRAIN.WATER, "Water"],
    ]) {
      const control = button(label, () => {
        this.selectedTerrain = code;
        this.armTerrain();
        this.setStatus(`${this.paintShape === "box" ? "Drag to fill a box with" : "Painting"} ${terrainName(code)}.`);
      }, { active: this.viewport.tool?.kind === "terrain" && this.selectedTerrain === code });
      control.dataset.terrain = terrainName(code);
      control.classList.add("map-editor-terrain-button");
      const preview = this.viewport.createTerrainPreview?.(code);
      if (preview) {
        preview.className = "map-editor-terrain-icon";
        preview.setAttribute("aria-hidden", "true");
        control.prepend(preview);
      }
      palette.appendChild(control);
    }
    const shapes = document.createElement("div");
    shapes.className = "map-editor-palette";
    for (const [value, label] of [["brush", "Brush"], ["box", "Box fill"]]) {
      shapes.appendChild(button(label, () => this.setPaintShape(value), { active: this.paintShape === value }));
    }
    const symmetry = document.createElement("select");
    symmetry.setAttribute("aria-label", "Symmetry");
    for (const [value, label] of [
      [MAP_EDITOR_SYMMETRY.NONE, "None"],
      [MAP_EDITOR_SYMMETRY.HORIZONTAL, "Horizontal"],
      [MAP_EDITOR_SYMMETRY.VERTICAL, "Vertical"],
      [MAP_EDITOR_SYMMETRY.RADIAL, "Radial (180°)"],
      [MAP_EDITOR_SYMMETRY.DIAGONAL, "Diagonal (both axes)"],
    ]) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = label;
      symmetry.appendChild(option);
    }
    symmetry.value = this.symmetry;
    symmetry.addEventListener("change", () => this.setSymmetry(symmetry.value));
    section.append(
      palette,
      field("Paint shape", shapes),
      field("Symmetry", symmetry),
      readout("Symmetry applies to terrain and base moves. Diagonal mirrors through both map diagonals; each drag remains one render and undo transaction."),
      readout("Authored start and natural clearances stay grass."),
    );
    return section;
  }

  renderLayouts() {
    const section = group("Spawn layouts");
    const layouts = this.session.draft.layouts || [];
    const select = document.createElement("select");
    for (const layout of layouts) {
      const option = document.createElement("option");
      option.value = layout.id;
      option.textContent = `${layout.id} · ${layout.slots.length} players`;
      select.appendChild(option);
    }
    select.value = this.session.selectedLayoutId;
    select.addEventListener("change", () => {
      this.session.selectLayout(select.value);
      this.selectedPlayerIndex = 0;
    });
    const count = document.createElement("select");
    for (let players = 1; players <= 4; players++) {
      const option = document.createElement("option");
      option.value = String(players);
      option.textContent = `${players} player${players === 1 ? "" : "s"}`;
      count.appendChild(option);
    }
    count.value = String(this.newLayoutPlayers);
    count.addEventListener("change", () => { this.newLayoutPlayers = Number(count.value); });
    section.append(
      field("Active layout", select),
      field("New layout", count),
      button("Add layout", () => {
        this.session.addLayout(this.newLayoutPlayers);
        this.selectedPlayerIndex = 0;
      }),
      button("Remove active layout", () => this.session.removeSelectedLayout(), { disabled: layouts.length <= 1 }),
    );
    return section;
  }

  renderPlayers() {
    const section = group("Player starts and natural bases");
    const players = this.session.playerSlots();
    if (!players.length) {
      section.appendChild(readout("Add or select a spawn layout first.", true));
      return section;
    }
    this.selectedPlayerIndex = Math.max(0, Math.min(players.length - 1, this.selectedPlayerIndex));
    const picker = document.createElement("div");
    picker.className = "map-editor-player-picker";
    for (const player of players) {
      const control = button(`P${player.playerIndex + 1}`, () => {
        this.selectedPlayerIndex = player.playerIndex;
        this.render();
      }, { active: player.playerIndex === this.selectedPlayerIndex });
      control.style.setProperty("--map-player-color", PLAYER_PALETTE[player.playerIndex % PLAYER_PALETTE.length]);
      picker.appendChild(control);
    }
    const selected = players[this.selectedPlayerIndex];
    const list = document.createElement("div");
    list.className = "map-editor-natural-list";
    for (const [index, natural] of selected.naturals.entries()) {
      const row = document.createElement("div");
      row.append(
        document.createTextNode(`Natural ${index + 1}: ${natural.x}, ${natural.y}`),
        button("Move", () => this.armNatural(natural.id)),
        button("Remove", () => this.removeNatural(natural.id)),
      );
      list.appendChild(row);
    }
    const start = selected.start ? `${selected.start.x}, ${selected.start.y}` : "not placed";
    section.append(
      picker,
      readout(`Player ${selected.playerIndex + 1} start: ${start}`),
      button("Move start", () => {
        this.viewport.armTool({ kind: "start", playerIndex: selected.playerIndex, symmetry: this.symmetry });
        this.setStatus(`Click the map to place Player ${selected.playerIndex + 1}'s start.`);
      }, { active: this.viewport.tool?.kind === "start" && this.viewport.tool?.playerIndex === selected.playerIndex }),
      button("Add natural", () => this.armNatural(""), {
        disabled: selected.naturals.length >= MAP_EDITOR_MAX_NATURALS_PER_PLAYER,
      }),
      list,
    );
    return section;
  }

  renderActions() {
    const section = group("Save and test");
    section.append(
      button("Save on this device", () => this.saveLocal()),
      button("Load saved map", () => this.loadLocal()),
      button("Export map JSON", () => this.exportJson()),
      button(this.pending ? "Opening Lab…" : "Open in Lab", () => void this.openLab(), {
        disabled: this.pending,
        className: "map-editor-primary",
      }),
      readout("Opening Lab validates this map on the server and starts a fresh ordinary Lab. Units and elapsed time never return to the editor."),
    );
    return section;
  }

  renderStatus() {
    const status = document.createElement("p");
    status.className = "map-editor-status";
    status.dataset.state = this.statusError ? "error" : "ok";
    status.setAttribute("aria-live", "polite");
    status.textContent = this.status;
    return status;
  }

  armNatural(naturalId) {
    this.viewport.armTool({
      kind: "natural",
      playerIndex: this.selectedPlayerIndex,
      naturalId,
      symmetry: this.symmetry,
    });
    this.setStatus(`Click the map to ${naturalId ? "move" : "add"} Player ${this.selectedPlayerIndex + 1}'s natural base.`);
  }

  removeNatural(naturalId) {
    const player = this.selectedPlayerIndex;
    const changed = this.session.mutate(`Removed Player ${player + 1} natural`, (draft) => {
      removeDraftPlayerNatural(draft, player, naturalId, this.session.selectedLayoutId);
    });
    this.setStatus(changed ? "Natural base removed." : "Natural base was already absent.", !changed);
  }

  armTerrain() {
    this.viewport.armTool({
      kind: "terrain",
      terrain: this.selectedTerrain,
      shape: this.paintShape,
      symmetry: this.symmetry,
    });
  }

  setPaintShape(shape) {
    this.paintShape = shape === "box" ? "box" : "brush";
    if (this.viewport.tool?.kind === "terrain") this.armTerrain();
    this.render();
  }

  setSymmetry(symmetry) {
    this.symmetry = Object.values(MAP_EDITOR_SYMMETRY).includes(symmetry)
      ? symmetry
      : MAP_EDITOR_SYMMETRY.NONE;
    if (this.viewport.tool) this.viewport.armTool({ ...this.viewport.tool, symmetry: this.symmetry });
    this.render();
  }

  newBlankMap() {
    this.session.initializeBlank({ size: 126, playerCount: 2 });
    this.selectedPlayerIndex = 0;
    this.viewport.armTool(null);
    this.setStatus("Created a blank two-player map.");
  }

  async loadCatalog() {
    if (!this.fetchImpl) return;
    try {
      const response = await this.fetchImpl(MAP_CATALOG_URL, { cache: "no-store" });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const payload = await response.json();
      this.catalog = normalizeCatalog(payload?.maps);
      this.selectedMapFile ||= this.catalog[0]?.file || "";
      this.catalogError = this.catalog.length ? "" : "No bundled maps are available.";
    } catch (error) {
      this.catalogError = `Map catalog unavailable: ${error.message || error}`;
    }
    this.render();
  }

  async loadBundledMap() {
    if (!this.fetchImpl || !safeMapFile(this.selectedMapFile)) return;
    this.pending = true;
    this.setStatus("Loading bundled map…");
    try {
      const response = await this.fetchImpl(`/maps/${encodeURIComponent(this.selectedMapFile)}`, { cache: "no-store" });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      this.session.loadAuthoredMap(await response.json());
      this.selectedPlayerIndex = 0;
      this.viewport.armTool(null);
      this.setStatus("Bundled map loaded.");
    } catch (error) {
      this.setStatus(`Map load failed: ${error.message || error}`, true);
    } finally {
      this.pending = false;
      this.render();
    }
  }

  undo() {
    if (this.session.undo()) this.setStatus("Undid the last map edit.");
  }

  redo() {
    if (this.session.redo()) this.setStatus("Redid the map edit.");
  }

  saveLocal() {
    const ok = this.session.saveLocal(this.workspaceId);
    this.setStatus(ok ? "Saved this workspace on this device." : "Local storage is unavailable.", !ok);
  }

  loadLocal() {
    const ok = this.session.loadLocal(this.workspaceId);
    this.setStatus(ok ? "Loaded the saved workspace." : "No saved workspace was found.", !ok);
  }

  exportJson() {
    try {
      const map = this.session.exportMap();
      const blob = new Blob([`${JSON.stringify(map, null, 2)}\n`], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `${slug(map.name)}.json`;
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      URL.revokeObjectURL(url);
      this.setStatus(`Exported ${anchor.download}.`);
    } catch (error) {
      this.setStatus(error.message || String(error), true);
    }
  }

  async openLab() {
    if (this.pending) return;
    this.pending = true;
    this.setStatus("Validating map and preparing a fresh Lab…");
    try {
      await this.onOpenLab?.({
        authoredMap: this.session.exportMap(),
        materializedMap: this.session.materialized(),
        selectedLayoutId: this.session.selectedLayoutId,
        workspaceId: this.workspaceId,
      });
    } catch (error) {
      this.pending = false;
      this.setStatus(error.message || String(error), true);
    }
  }

  handleKeyDown(event) {
    if (event.defaultPrevented || isTextEntry(event.target) || !(event.ctrlKey || event.metaKey) || event.altKey) return;
    const key = String(event.key || "").toLowerCase();
    const redo = key === "y" || (key === "z" && event.shiftKey);
    const undo = key === "z" && !event.shiftKey;
    if (!undo && !redo) return;
    event.preventDefault();
    redo ? this.redo() : this.undo();
  }

  setStatus(message, error = false) {
    this.status = String(message || "");
    this.statusError = !!error;
    this.render();
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    window.removeEventListener("keydown", this.onKeyDown);
    this.unsubscribe?.();
    this.el.remove();
  }
}

function group(title) {
  const section = document.createElement("fieldset");
  section.className = "map-editor-group";
  const legend = document.createElement("legend");
  legend.textContent = title;
  section.appendChild(legend);
  return section;
}

function button(label, onClick, { disabled = false, active = false, title = "", className = "" } = {}) {
  const control = document.createElement("button");
  control.type = "button";
  control.className = `map-editor-button ${className}`.trim();
  control.textContent = label;
  control.disabled = !!disabled;
  control.dataset.active = active ? "true" : "false";
  if (title) control.title = title;
  control.addEventListener("click", onClick);
  return control;
}

function field(labelText, control) {
  const label = document.createElement("label");
  label.className = "map-editor-field";
  const text = document.createElement("span");
  text.textContent = labelText;
  label.append(text, control);
  return label;
}

function textField(labelText, value, onChange) {
  const input = document.createElement("input");
  input.value = value;
  input.maxLength = 80;
  input.addEventListener("change", () => onChange(input.value));
  return field(labelText, input);
}

function textAreaField(labelText, value, onChange) {
  const input = document.createElement("textarea");
  input.value = value;
  input.maxLength = 500;
  input.rows = 3;
  input.addEventListener("change", () => onChange(input.value));
  return field(labelText, input);
}

function readout(text, error = false) {
  const node = document.createElement("p");
  node.className = "map-editor-readout";
  node.dataset.state = error ? "error" : "ok";
  node.textContent = text;
  return node;
}

function terrainName(code) {
  if (code === TERRAIN.ROCK) return "stone";
  if (code === TERRAIN.WATER) return "water";
  return "grass";
}

function normalizeCatalog(entries) {
  if (!Array.isArray(entries)) return [];
  return entries.flatMap((entry) => {
    const file = String(entry?.file || "").trim();
    if (!safeMapFile(file)) return [];
    return [{
      file,
      name: String(entry?.name || file.replace(/\.json$/i, "")),
      description: String(entry?.description || ""),
    }];
  });
}

function safeMapFile(file) {
  return /^[a-z0-9][a-z0-9._-]*\.json$/i.test(file) && !file.includes("..");
}

function slug(value) {
  return String(value || "map").trim().toLowerCase().replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "").slice(0, 64) || "map";
}

function isTextEntry(target) {
  return ["INPUT", "TEXTAREA", "SELECT"].includes(String(target?.tagName || "")) || !!target?.isContentEditable;
}
