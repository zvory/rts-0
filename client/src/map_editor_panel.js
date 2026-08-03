import { TERRAIN } from "./protocol.js";
import { LabPanelWindowChrome } from "./lab_panel_window.js";
import { buildMapFromRecipe, isMapAuthoringRecipe } from "./map_authoring/recipe.js";
import { mapSymmetryWarnings } from "./map_authoring/symmetry_validation.js";
import {
  canonicalDoodadColor,
  MAP_EDITOR_DEFAULT_FLOWER_COLOR,
  MAP_EDITOR_DOODAD_CATALOG,
  MAP_EDITOR_DOODAD_TYPES,
  MAP_EDITOR_MAX_DOODADS,
  isWildflowerDoodadType,
} from "./map_editor_doodads.js";
import {
  MAP_EDITOR_DEFAULT_SIZE,
  MAP_EDITOR_HISTORY_LIMIT,
  MAP_EDITOR_MAX_BASE_SITES,
  MAP_EDITOR_MAX_OIL_PATCHES,
  MAP_EDITOR_MAX_STEEL_PATCHES,
  MAP_EDITOR_MAX_SIZE,
  MAP_EDITOR_MAX_START_LOCATIONS,
  MAP_EDITOR_MIN_SIZE,
  MAP_EDITOR_SYMMETRY,
  mapEditorSymmetrySupported,
  removeDraftLocation,
} from "./map_editor_session.js";

const MAP_CATALOG_URL = "/maps/catalog";
const MAP_EDITOR_MAX_JSON_BYTES = 2 * 1024 * 1024;
const MAP_EDITOR_OPTIONS_STORAGE_KEY = "rts.mapEditor.panel.window.v1";
const MAP_EDITOR_TOOLS_STORAGE_KEY = "rts.mapEditor.tools.window.v1";
const MAP_EDITOR_ANALYSIS_TIMEOUT_MS = 20_000;

export class MapEditorPanel {
  constructor({
    root,
    session,
    viewport,
    onOpenLab,
    onOpenPreview,
    fetchImpl = globalThis.fetch?.bind(globalThis),
    createAbortController = () => new AbortController(),
    setTimeoutImpl = globalThis.setTimeout?.bind(globalThis),
    clearTimeoutImpl = globalThis.clearTimeout?.bind(globalThis),
    analysisTimeoutMs = MAP_EDITOR_ANALYSIS_TIMEOUT_MS,
  }) {
    this.root = root;
    this.session = session;
    this.viewport = viewport;
    this.onOpenLab = onOpenLab;
    this.onOpenPreview = onOpenPreview;
    this.fetchImpl = fetchImpl;
    this.createAbortController = createAbortController;
    this.setTimeoutImpl = setTimeoutImpl;
    this.clearTimeoutImpl = clearTimeoutImpl;
    this.analysisTimeoutMs = analysisTimeoutMs;
    this.catalog = [];
    this.catalogSkipped = [];
    this.catalogError = "";
    this.selectedMapFile = "";
    this.selectedStartIndex = 0;
    this.selectedBaseIndex = 0;
    this.selectedTerrain = TERRAIN.ROCK;
    this.paintShape = "brush";
    this.selectedDoodadType = MAP_EDITOR_DOODAD_TYPES.TREE_OAK;
    this.doodadMode = "place";
    this.doodadColor = MAP_EDITOR_DEFAULT_FLOWER_COLOR;
    this.doodadRadius = 48;
    this.doodadDensity = 4;
    this.symmetry = MAP_EDITOR_SYMMETRY.NONE;
    this.blankMapWidth = String(MAP_EDITOR_DEFAULT_SIZE);
    this.blankMapHeight = String(MAP_EDITOR_DEFAULT_SIZE);
    this.observedMapDimensions = null;
    this.pending = false;
    this.analysisPending = false;
    this.analysisResult = null;
    this.analysisKind = null;
    this.analysisRequestToken = 0;
    this.analysisAbortController = null;
    this.analysisTimeoutId = null;
    this.analysisMapFingerprint = null;
    this.recipeText = JSON.stringify({
      name: "Recipe map",
      width: session.draft?.width || MAP_EDITOR_DEFAULT_SIZE,
      height: session.draft?.height || MAP_EDITOR_DEFAULT_SIZE,
      symmetry: "none",
      operations: [],
    }, null, 2);
    this.status = "";
    this.statusError = false;
    this.analysisStatusOwned = false;
    this.destroyed = false;
    this.optionsEl = this.createPanelElement("map-editor-options-window", "Map Editor options");
    this.toolsEl = this.createPanelElement("map-editor-tools-window", "Map Editor tools");
    this.el = this.optionsEl;
    root.append(this.optionsEl, this.toolsEl);
    this.optionsWindowChrome = new LabPanelWindowChrome(this.optionsEl, {
      storageKey: MAP_EDITOR_OPTIONS_STORAGE_KEY,
      panelLabel: "map editor options",
    });
    this.toolsWindowChrome = new LabPanelWindowChrome(this.toolsEl, {
      storageKey: MAP_EDITOR_TOOLS_STORAGE_KEY,
      panelLabel: "map editor tools",
    });
    this.onKeyDown = (event) => this.handleKeyDown(event);
    window.addEventListener("keydown", this.onKeyDown);
    this.unsubscribe = session.subscribe((snapshot) => this.applySessionSnapshot(snapshot));
    this.unsubscribeCamera = viewport.subscribeZoom((percent) => this.updateZoomControl(percent));
    void this.loadCatalog();
  }

  createPanelElement(className, ariaLabel) {
    const el = document.createElement("aside");
    el.className = `lab-panel map-editor-panel ${className}`;
    el.setAttribute("aria-label", ariaLabel);
    return el;
  }

  applySessionSnapshot(snapshot) {
    const analysisFingerprint = authoredMapFingerprint(snapshot?.draft);
    if (this.analysisMapFingerprint === null) this.analysisMapFingerprint = analysisFingerprint;
    else if (analysisFingerprint !== this.analysisMapFingerprint) {
      MapEditorPanel.prototype.invalidateAuthoritativeAnalysis.call(this);
      this.analysisMapFingerprint = analysisFingerprint;
    }
    const width = snapshot?.draft?.width;
    const height = snapshot?.draft?.height;
    const loadedMap = snapshot?.reason === "loaded"
      || snapshot?.reason === "initialized"
      || snapshot?.lastAction === "Loaded local map";
    const dimensionsChanged = !this.observedMapDimensions
      || width !== this.observedMapDimensions.width
      || height !== this.observedMapDimensions.height;
    if (Number.isInteger(width) && Number.isInteger(height) && (dimensionsChanged || loadedMap)) {
      this.blankMapWidth = String(width);
      this.blankMapHeight = String(height);
    }
    if (Number.isInteger(width) && Number.isInteger(height)) this.observedMapDimensions = { width, height };
    if (!mapEditorSymmetrySupported(snapshot?.draft, this.symmetry)) {
      this.symmetry = MAP_EDITOR_SYMMETRY.NONE;
      this.viewport.setSymmetry(this.symmetry);
      if (this.viewport.tool) this.viewport.armTool({ ...this.viewport.tool, symmetry: this.symmetry });
    }
    this.render();
  }

  render() {
    if (this.destroyed) return;
    this.renderOptionsWindow();
    this.renderToolsWindow();
  }

  renderOptionsWindow() {
    const scroll = panelScroll(this.optionsEl);
    this.optionsEl.replaceChildren();
    const header = this.optionsWindowChrome.renderHeader({
      kicker: "Options",
      collapseLabel: "map editor options panel",
    });
    header.classList.add("map-editor-header");
    const body = document.createElement("div");
    body.className = "lab-panel-body map-editor-panel-body";
    const status = this.renderStatus();
    if (status) body.appendChild(status);
    if (!this.session.draft) {
      body.appendChild(readout("Preparing editor…"));
    } else {
      body.append(
        this.renderMapSource(),
        this.renderHistory(),
        this.renderDetails(),
        this.renderActions(),
      );
    }
    this.optionsEl.append(header, body, this.optionsWindowChrome.renderResizeHandle());
    restorePanelScroll(body, scroll);
  }

  renderToolsWindow() {
    const scroll = panelScroll(this.toolsEl);
    this.toolsEl.replaceChildren();
    const header = this.toolsWindowChrome.renderHeader({
      kicker: "Tools",
      collapseLabel: "map editor tools panel",
    });
    header.classList.add("map-editor-header");
    const body = document.createElement("div");
    body.className = "lab-panel-body map-editor-panel-body";
    if (!this.session.draft) {
      body.appendChild(readout("Preparing editor…"));
    } else {
      body.append(
        this.renderZoom(),
        this.renderTerrain(),
        this.renderMapOverlays(),
        this.renderDoodads(),
        this.renderLocations(),
      );
    }
    this.toolsEl.append(header, body, this.toolsWindowChrome.renderResizeHandle());
    restorePanelScroll(body, scroll);
  }

  renderZoom() {
    const section = group("Zoom");
    const framing = document.createElement("div");
    framing.className = "map-editor-zoom-framing";
    framing.append(
      button("Fill screen", () => this.viewport.fillScreen()),
      button("Fit to screen", () => this.viewport.fitToScreen()),
    );

    const controls = document.createElement("div");
    controls.className = "map-editor-zoom-controls";
    const zoomInput = document.createElement("input");
    const zoomLimits = this.viewport.zoomLimitsPercent();
    zoomInput.type = "number";
    zoomInput.min = String(zoomLimits.min);
    zoomInput.max = String(zoomLimits.max);
    zoomInput.step = "1";
    zoomInput.value = String(this.viewport.zoomPercent());
    zoomInput.setAttribute("aria-label", "Zoom percentage");
    zoomInput.addEventListener("change", () => {
      zoomInput.value = String(this.viewport.setZoomPercent(zoomInput.value));
    });
    this.zoomInput = zoomInput;
    const percent = document.createElement("span");
    percent.className = "map-editor-zoom-percent";
    percent.textContent = "%";
    controls.append(
      button("−", () => this.viewport.zoomOut(), { title: "Zoom out", className: "map-editor-zoom-step" }),
      zoomInput,
      percent,
      button("+", () => this.viewport.zoomIn(), { title: "Zoom in", className: "map-editor-zoom-step" }),
    );
    section.append(framing, controls);
    return section;
  }

  updateZoomControl(percent = this.viewport.zoomPercent()) {
    if (!this.zoomInput?.isConnected) return;
    this.zoomInput.value = String(percent);
  }

  renderMapSource() {
    const section = group("Map source");
    const select = document.createElement("select");
    select.setAttribute("aria-label", "Bundled map");
    for (const entry of this.catalog) {
      const option = document.createElement("option");
      option.value = entry.file;
      option.textContent = entry.label || entry.name;
      select.appendChild(option);
    }
    select.value = this.selectedMapFile;
    select.addEventListener("change", () => { this.selectedMapFile = select.value; });
    const blankWidth = dimensionInput("Map width", this.blankMapWidth, (value) => { this.blankMapWidth = value; });
    const blankHeight = dimensionInput("Map height", this.blankMapHeight, (value) => { this.blankMapHeight = value; });
    section.append(
      field("Bundled map", select),
      button("Load bundled map", () => void this.loadBundledMap(), { disabled: !this.selectedMapFile || this.pending }),
      field("Map width", blankWidth),
      field("Map height", blankHeight),
      button("New blank map", () => this.newBlankMap(), { disabled: this.pending }),
      button("Resize current map", () => this.resizeMap(), { disabled: this.pending }),
    );
    if (this.catalogSkipped.length) {
      section.appendChild(readout(`Skipped unsupported map filename${this.catalogSkipped.length === 1 ? "" : "s"}: ${this.catalogSkipped.join(", ")}`, true));
    }
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
      [TERRAIN.GRAVEL_A, "Gravel A — Slate"],
      [TERRAIN.GRAVEL_B, "Gravel B — Limestone"],
      [TERRAIN.GRAVEL_C, "Gravel C — Chalk"],
      [TERRAIN.DIRT_A, "Dirt A — Loam"],
      [TERRAIN.DIRT_B, "Dirt B — Red Clay"],
      [TERRAIN.DIRT_C, "Dirt C — Dry Ochre"],
      [TERRAIN.MUD_A, "Mud A — Churned"],
      [TERRAIN.MUD_B, "Mud B — Waterlogged"],
      [TERRAIN.MUD_C, "Mud C — Clay"],
      [TERRAIN.FROSTED_GROUND, "Frosted Ground"],
      [TERRAIN.ROCK, "Stone"],
      [TERRAIN.WATER, "Water"],
      [TERRAIN.ROAD_BARE, "Road — bare"],
      [TERRAIN.ROAD_HORIZONTAL, "Road — horizontal"],
      [TERRAIN.ROAD_VERTICAL, "Road — vertical"],
      [TERRAIN.ROAD_DIAGONAL_NW_SE, "Road — diagonal ↘"],
      [TERRAIN.ROAD_DIAGONAL_NE_SW, "Road — diagonal ↙"],
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
    symmetry.title = "Symmetry applies to terrain and base moves.";
    for (const [value, label] of [
      [MAP_EDITOR_SYMMETRY.NONE, "None"],
      [MAP_EDITOR_SYMMETRY.HORIZONTAL, "Horizontal"],
      [MAP_EDITOR_SYMMETRY.VERTICAL, "Vertical"],
      [MAP_EDITOR_SYMMETRY.HALF_TURN, "Half-turn (180°)"],
      [MAP_EDITOR_SYMMETRY.THREE_WAY, "3-way rotation (120°, square-grid approximation)"],
      [MAP_EDITOR_SYMMETRY.RADIAL, "Radial (4-way)"],
      [MAP_EDITOR_SYMMETRY.DIAGONAL_MAIN, "Diagonal ↘ (top-left ↔ bottom-right)"],
      [MAP_EDITOR_SYMMETRY.DIAGONAL_ANTI, "Diagonal ↙ (top-right ↔ bottom-left)"],
    ]) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = label;
      option.disabled = !mapEditorSymmetrySupported(this.session.draft, value);
      symmetry.appendChild(option);
    }
    symmetry.value = this.symmetry;
    symmetry.addEventListener("change", () => this.setSymmetry(symmetry.value));
    section.append(
      palette,
      field("Paint shape", shapes),
      field("Symmetry", symmetry),
    );
    for (const warning of this.currentSymmetryWarnings()) {
      section.appendChild(readout(`Symmetry warning: ${warning}`, true));
    }
    return section;
  }

  currentSymmetryWarnings() {
    return mapSymmetryWarnings(this.session.draft, this.symmetry);
  }

  renderMapOverlays() {
    const section = group("Gameplay overlays");
    const palette = document.createElement("div");
    palette.className = "map-editor-palette";
    const tools = [
      ["Forest", { stealth: true, noVehicle: true }, "forest tiles"],
      ["Stealth only", { stealth: true }, "stealth tiles"],
      ["No vehicles only", { noVehicle: true }, "no-vehicle tiles"],
      ["Erase stealth", { stealth: false }, "stealth erasure"],
      ["Erase no vehicles", { noVehicle: false }, "no-vehicle erasure"],
      ["Erase both", { stealth: false, noVehicle: false }, "overlay erasure"],
    ];
    for (const [label, edit, status] of tools) {
      palette.appendChild(button(label, () => {
        this.armOverlay(edit, status);
        this.setStatus(`${this.paintShape === "box" ? "Drag to fill a box with" : "Painting"} ${status}.`);
      }, {
        active: this.viewport.tool?.kind === "overlay"
          && JSON.stringify(this.viewport.tool.edit) === JSON.stringify(edit),
      }));
    }
    section.append(
      readout(`${this.session.draft.stealthTiles.length} stealth tiles; ${this.session.draft.noVehicleTiles.length} no-vehicle tiles.`),
      readout("Forest paints both layers. Long grass or other future cover can use stealth alone."),
      palette,
    );
    return section;
  }

  renderLocations() {
    const section = group("Start and base locations");
    const starts = this.session.draft.startLocations;
    const bases = this.session.mapOverlay()?.bases || [];
    this.selectedStartIndex = Math.max(0, Math.min(starts.length - 1, this.selectedStartIndex));
    const selectedBaseIndex = bases.findIndex((base) => base.index === this.viewport.selectedBaseIndex);
    if (selectedBaseIndex >= 0) this.selectedBaseIndex = selectedBaseIndex;
    this.selectedBaseIndex = Math.max(0, Math.min(bases.length - 1, this.selectedBaseIndex));
    const startPicker = document.createElement("div");
    startPicker.className = "map-editor-player-picker";
    for (const [index, start] of starts.entries()) {
      startPicker.appendChild(button(`S${index + 1}`, () => {
        this.selectedStartIndex = index;
        this.render();
      }, { active: index === this.selectedStartIndex, title: `${start.x}, ${start.y}` }));
    }
    const basePicker = document.createElement("div");
    basePicker.className = "map-editor-player-picker";
    for (const [index, base] of bases.entries()) {
      basePicker.appendChild(button(`B${index + 1}`, () => {
        this.selectedBaseIndex = index;
        this.viewport.setSelectedBase(base.index);
        this.render();
      }, { active: index === this.selectedBaseIndex, title: `${base.x}, ${base.y}` }));
    }
    const start = starts[this.selectedStartIndex];
    const base = bases[this.selectedBaseIndex];
    const startBaseIndex = start
      ? this.session.draft.baseSites.findIndex((site) => site.x === start.x && site.y === start.y)
      : -1;
    const startBase = this.session.draft.baseSites[startBaseIndex];
    this.viewport.setSelectedBase(base?.index ?? null);
    section.append(
      readout(`Start locations set player capacity (${starts.length}/${MAP_EDITOR_MAX_START_LOCATIONS}). Drafts may temporarily have none. Every base site always spawns resources.`),
      startPicker,
      readout(start ? `Start ${this.selectedStartIndex + 1}: ${start.x}, ${start.y}` : "No start locations yet. Choose Add start, then click the map."),
      button("Move start", () => this.armLocation("start", this.selectedStartIndex), {
        disabled: !start,
        active: this.viewport.tool?.kind === "start" && this.viewport.tool?.locationIndex === this.selectedStartIndex,
      }),
      button("Add start", () => this.armLocation("start", null, true), { disabled: starts.length >= MAP_EDITOR_MAX_START_LOCATIONS }),
      button("Remove start", () => this.removeLocation("start", this.selectedStartIndex), { disabled: !start }),
      patchCountField("Start-base steel patches", startBase?.steelPatches, MAP_EDITOR_MAX_STEEL_PATCHES, (value) => {
        this.updateBasePatchCount(startBaseIndex, "steelPatches", value);
      }, !startBase),
      patchCountField("Start-base oil patches", startBase?.oilPatches, MAP_EDITOR_MAX_OIL_PATCHES, (value) => {
        this.updateBasePatchCount(startBaseIndex, "oilPatches", value);
      }, !startBase),
      basePicker,
      readout(base ? `Base ${this.selectedBaseIndex + 1}: ${base.x}, ${base.y}` : "No neutral base sites yet."),
      button("Move base", () => this.armLocation("base", base?.index), {
        disabled: !base,
        active: this.viewport.tool?.kind === "base" && !this.viewport.tool?.add && this.viewport.tool?.locationIndex === base?.index,
      }),
      button("Add base", () => this.armLocation("base", null, true), {
        disabled: this.session.draft.baseSites.length >= MAP_EDITOR_MAX_BASE_SITES,
      }),
      button("Remove base", () => this.removeLocation("base", base?.index), { disabled: !base }),
      patchCountField("Base steel patches", base?.steelPatches, MAP_EDITOR_MAX_STEEL_PATCHES, (value) => {
        this.updateBasePatchCount(base?.index, "steelPatches", value);
      }, !base),
      patchCountField("Base oil patches", base?.oilPatches, MAP_EDITOR_MAX_OIL_PATCHES, (value) => {
        this.updateBasePatchCount(base?.index, "oilPatches", value);
      }, !base),
      readout("Bases and starts reserve a passable grass area."),
    );
    return section;
  }

  renderDoodads() {
    const section = group("Doodads");
    const trees = MAP_EDITOR_DOODAD_CATALOG.filter((entry) => entry.kind === "tree");
    const flowers = MAP_EDITOR_DOODAD_CATALOG.filter((entry) => entry.kind === "wildflower");
    const neutralUnits = MAP_EDITOR_DOODAD_CATALOG.filter((entry) => entry.kind === "neutral-unit");
    const treePalette = this.renderDoodadPalette(trees);
    const flowerPalette = this.renderDoodadPalette(flowers);
    const neutralUnitPalette = this.renderDoodadPalette(neutralUnits);

    const tools = document.createElement("div");
    tools.className = "map-editor-palette";
    tools.append(
      button("Place one", () => this.armDoodad("place"), {
        active: this.viewport.tool?.kind === "doodad" && this.viewport.tool?.mode === "place",
      }),
      button("Spray flowers", () => {
        if (!isWildflowerDoodadType(this.selectedDoodadType)) {
          this.selectedDoodadType = MAP_EDITOR_DOODAD_TYPES.WILDFLOWER_SINGLE;
        }
        this.armDoodad("spray");
      }, { active: this.viewport.tool?.kind === "doodad" && this.viewport.tool?.mode === "spray" }),
      button("Remove doodads", () => this.armDoodad("remove"), {
        active: this.viewport.tool?.kind === "doodad" && this.viewport.tool?.mode === "remove",
      }),
      button("Erase brush", () => this.armDoodad("erase"), {
        active: this.viewport.tool?.kind === "doodad" && this.viewport.tool?.mode === "erase",
      }),
      button("Delete selection", () => this.viewport.deleteSelectedDoodads()),
    );

    const color = document.createElement("input");
    color.type = "color";
    color.value = this.doodadColor;
    color.setAttribute("aria-label", "Wildflower color");
    color.addEventListener("input", () => {
      this.doodadColor = canonicalDoodadColor(color.value, MAP_EDITOR_DEFAULT_FLOWER_COLOR);
      if (this.viewport.tool?.kind === "doodad" && isWildflowerDoodadType(this.selectedDoodadType)) {
        this.armDoodad(this.doodadMode);
      }
    });

    const radius = numericInput(this.doodadRadius, 4, 256, (value) => {
      this.doodadRadius = value;
      if (this.viewport.tool?.kind === "doodad") this.armDoodad(this.doodadMode);
    }, "Doodad brush radius");
    const density = numericInput(this.doodadDensity, 1, 12, (value) => {
      this.doodadDensity = value;
      if (this.viewport.tool?.kind === "doodad") this.armDoodad(this.doodadMode);
    }, "Wildflower spray density");
    section.append(
      readout(`${this.session.draft.doodads.length}/${MAP_EDITOR_MAX_DOODADS} doodads. Tree species are visual variants of one semantic tree type; tank traps spawn as completed neutral units.`),
      readout("Trees"),
      treePalette,
      readout("Wildflowers"),
      flowerPalette,
      readout("Neutral units"),
      neutralUnitPalette,
      field("Tool", tools),
      field("Flower color", color),
      field("Brush radius (world px)", radius),
      field("Spray density", density),
      readout("Symmetry applies when creating doodads. Remove doodads uses a drag box; Delete/Backspace removes everything selected."),
    );
    return section;
  }

  renderDoodadPalette(entries) {
    const palette = document.createElement("div");
    palette.className = "map-editor-palette map-editor-doodad-palette";
    for (const entry of entries) {
      palette.appendChild(button(entry.label, () => {
        this.selectedDoodadType = entry.typeId;
        if (!isWildflowerDoodadType(entry.typeId)) this.doodadMode = "place";
        this.armDoodad(this.doodadMode);
        this.setStatus(`Placing ${entry.label.toLowerCase()}.`);
      }, {
        active: this.viewport.tool?.kind === "doodad"
          && !["remove", "erase"].includes(this.viewport.tool?.mode)
          && this.selectedDoodadType === entry.typeId,
      }));
    }
    return palette;
  }

  renderActions() {
    const section = group("Save and test");
    const recipe = document.createElement("textarea");
    recipe.value = this.recipeText;
    recipe.rows = 10;
    recipe.maxLength = MAP_EDITOR_MAX_JSON_BYTES;
    recipe.spellcheck = false;
    recipe.setAttribute("aria-label", "Map recipe JSON");
    recipe.addEventListener("input", () => { this.recipeText = recipe.value; });
    section.append(
      button("Load map or recipe JSON", () => this.chooseJsonFile()),
      field("Recipe JSON", recipe),
      button("Apply recipe JSON", () => this.applyRecipeText()),
      button("Export map JSON", () => this.exportJson()),
      button(this.analysisPending && this.analysisKind === "check" ? "Checking…" : "Authoritative check", () => void this.runAuthoritativeAnalysis("check"), {
        disabled: this.analysisPending,
      }),
      button(this.analysisPending && this.analysisKind === "report" ? "Reporting routes…" : "Route report", () => void this.runAuthoritativeAnalysis("report"), {
        disabled: this.analysisPending,
      }),
      button(this.pending ? "Preparing preview…" : "Preview PNGs", () => void this.openPreview(), {
        disabled: this.pending,
      }),
      button(this.pending ? "Opening Lab…" : "Open in Lab", () => void this.openLab(), {
        disabled: this.pending,
        className: "map-editor-primary",
      }),
      readout("Recipe JSON uses the same fill, rectangle, blob, stroke, road, base, start, and symmetry operations as the map-author CLI. Opening Lab validates the resulting map on the server and starts a fresh ordinary Lab."),
      readout("Preview PNGs opens the existing game renderer with 2048 px world and minimap downloads."),
    );
    if (this.analysisResult) section.appendChild(renderAnalysisResult(this.analysisKind, this.analysisResult));
    return section;
  }

  renderStatus() {
    if (!this.status) return null;
    const status = document.createElement("p");
    status.className = "map-editor-status";
    status.dataset.state = this.statusError ? "error" : "ok";
    status.setAttribute("role", this.statusError ? "alert" : "status");
    status.setAttribute("aria-live", this.statusError ? "assertive" : "polite");
    status.textContent = this.statusError ? `Error: ${this.status}` : this.status;
    return status;
  }

  armLocation(kind, locationIndex, add = false) {
    this.viewport.armTool({ kind, locationIndex, add, symmetry: this.symmetry });
    this.setStatus(`Click the map to ${add ? "add" : "move"} this ${kind === "start" ? "start location" : "base site"}.`);
  }

  removeLocation(kind, locationIndex) {
    let result = null;
    const changed = this.session.mutate(`Removed ${kind === "start" ? "start location" : "base site"}`, (draft) => {
      result = removeDraftLocation(draft, { kind, locationIndex });
    });
    if (changed) this.viewport.armTool(null);
    this.setStatus(changed ? "Map location removed." : result?.error || "Map location was already absent.", !changed);
  }

  updateBasePatchCount(baseIndex, fieldName, value) {
    const max = fieldName === "oilPatches" ? MAP_EDITOR_MAX_OIL_PATCHES : MAP_EDITOR_MAX_STEEL_PATCHES;
    const count = Math.max(0, Math.min(max, Math.trunc(Number(value)) || 0));
    const changed = this.session.mutate("Updated base resources", (draft) => {
      const site = draft.baseSites[Math.trunc(Number(baseIndex))];
      if (site) site[fieldName] = count;
    });
    this.setStatus(changed ? "Base resource counts updated." : "Base resource count unchanged.");
  }

  armTerrain() {
    this.viewport.armTool({
      kind: "terrain",
      terrain: this.selectedTerrain,
      shape: this.paintShape,
      symmetry: this.symmetry,
    });
  }

  armOverlay(edit, label) {
    this.viewport.armTool({
      kind: "overlay",
      edit: { ...edit },
      label,
      shape: this.paintShape,
      symmetry: this.symmetry,
    });
    this.render();
  }

  armDoodad(mode) {
    this.doodadMode = ["place", "spray", "remove", "erase"].includes(mode) ? mode : "place";
    this.viewport.armTool({
      kind: "doodad",
      mode: this.doodadMode,
      typeId: this.selectedDoodadType,
      color: isWildflowerDoodadType(this.selectedDoodadType) ? this.doodadColor : null,
      radius: this.doodadRadius,
      density: this.doodadDensity,
      symmetry: this.symmetry,
    });
    this.render();
  }

  setPaintShape(shape) {
    this.paintShape = shape === "box" ? "box" : "brush";
    if (this.viewport.tool?.kind === "terrain") this.armTerrain();
    else if (this.viewport.tool?.kind === "overlay") {
      this.viewport.armTool({ ...this.viewport.tool, shape: this.paintShape });
    }
    this.render();
  }

  setSymmetry(symmetry) {
    this.symmetry = Object.values(MAP_EDITOR_SYMMETRY).includes(symmetry)
      && mapEditorSymmetrySupported(this.session.draft, symmetry)
      ? symmetry
      : MAP_EDITOR_SYMMETRY.NONE;
    this.viewport.setSymmetry(this.symmetry);
    if (this.viewport.tool) this.viewport.armTool({ ...this.viewport.tool, symmetry: this.symmetry });
    this.render();
  }

  newBlankMap() {
    const dimensions = this.requestedDimensions();
    if (!dimensions) {
      this.setStatus(`Map width and height must be whole numbers from ${MAP_EDITOR_MIN_SIZE} to ${MAP_EDITOR_MAX_SIZE}.`, true);
      return false;
    }
    this.session.initializeBlank({ ...dimensions, playerCount: 2 });
    this.selectedStartIndex = 0;
    this.selectedBaseIndex = 0;
    this.viewport.armTool(null);
    this.setStatus(`Created a blank ${dimensions.width} × ${dimensions.height} two-player map.`);
    return true;
  }

  resizeMap() {
    const dimensions = this.requestedDimensions();
    if (!dimensions) {
      this.setStatus(`Map width and height must be whole numbers from ${MAP_EDITOR_MIN_SIZE} to ${MAP_EDITOR_MAX_SIZE}.`, true);
      return false;
    }
    const result = this.session.resize(dimensions);
    this.viewport.armTool(null);
    this.setStatus(result.ok
      ? result.count ? `Resized the map to ${dimensions.width} × ${dimensions.height}; new edge tiles are grass.` : "Map dimensions unchanged."
      : result.error, !result.ok);
    return result.ok;
  }

  requestedDimensions() {
    const width = Number(this.blankMapWidth);
    const height = Number(this.blankMapHeight);
    if (![width, height].every((value) => Number.isInteger(value) && value >= MAP_EDITOR_MIN_SIZE && value <= MAP_EDITOR_MAX_SIZE)) return null;
    return { width, height };
  }

  async loadCatalog() {
    if (!this.fetchImpl) return;
    try {
      const response = await this.fetchImpl(MAP_CATALOG_URL, { cache: "no-store" });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const payload = await response.json();
      const catalog = normalizeCatalog(payload?.maps);
      this.catalog = catalog.maps;
      this.catalogSkipped = catalog.skipped;
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
      this.loadMapData(await response.json());
      this.setStatus("Bundled map loaded.");
    } catch (error) {
      this.setStatus(`Map load failed: ${error.message || error}`, true);
    } finally {
      this.pending = false;
      this.render();
    }
  }

  loadMapData(map) {
    const recipe = isMapAuthoringRecipe(map);
    this.session.loadAuthoredMap(recipe ? buildMapFromRecipe(map) : map);
    this.selectedStartIndex = 0;
    this.selectedBaseIndex = 0;
    MapEditorPanel.prototype.invalidateAuthoritativeAnalysis.call(this);
    this.viewport.armTool(null);
    return recipe ? "recipe" : "map";
  }

  applyRecipeText() {
    try {
      if (this.recipeText.length > MAP_EDITOR_MAX_JSON_BYTES) throw new Error("Recipe JSON must be 2 MB or smaller.");
      const value = JSON.parse(this.recipeText);
      if (!isMapAuthoringRecipe(value)) {
        throw new Error("Recipe JSON needs width and height; operations may be omitted or an array.");
      }
      this.loadMapData(value);
      this.setStatus(`Applied recipe for ${this.session.draft.name}.`);
      return true;
    } catch (error) {
      this.setStatus(`Could not apply recipe: ${error.message || error}`, true);
      return false;
    }
  }

  async runAuthoritativeAnalysis(kind) {
    const mode = kind === "report" ? "report" : "check";
    if (this.analysisPending) return null;
    if (!this.fetchImpl) {
      this.setStatus("Authoritative map analysis is unavailable.", true);
      return null;
    }
    const body = JSON.stringify(this.session.exportMap());
    const fingerprint = body;
    const token = (this.analysisRequestToken || 0) + 1;
    this.analysisRequestToken = token;
    this.analysisMapFingerprint = fingerprint;
    const controller = this.createAbortController?.();
    this.analysisAbortController = controller || null;
    this.analysisPending = true;
    this.analysisKind = mode;
    this.analysisResult = null;
    setAuthoritativeAnalysisStatus(
      this,
      mode === "report" ? "Calculating authoritative routes…" : "Running authoritative map check…",
    );
    try {
      const responseAndJson = (async () => {
        const response = await this.fetchImpl(`/api/map-authoring/${mode}`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body,
          ...(controller?.signal ? { signal: controller.signal } : {}),
        });
        return { response, result: await response.json() };
      })();
      const timeout = new Promise((_, reject) => {
        if (typeof this.setTimeoutImpl !== "function") return;
        const delay = positiveTimeout(this.analysisTimeoutMs, MAP_EDITOR_ANALYSIS_TIMEOUT_MS);
        this.analysisTimeoutId = this.setTimeoutImpl(() => {
          controller?.abort?.();
          reject(new Error(`analysis timed out after ${delay} ms`));
        }, delay);
      });
      const { response, result } = await Promise.race([responseAndJson, timeout]);
      if (!MapEditorPanel.prototype.authoritativeAnalysisRequestIsCurrent.call(this, token, fingerprint)) return null;
      this.analysisResult = result;
      if (!response.ok || result?.valid !== true) throw new Error(result?.error || `HTTP ${response.status}`);
      setAuthoritativeAnalysisStatus(this, authoritativeAnalysisSummary(mode, result));
      return result;
    } catch (error) {
      if (!MapEditorPanel.prototype.authoritativeAnalysisRequestIsCurrent.call(this, token, fingerprint)) return null;
      setAuthoritativeAnalysisStatus(this, `Authoritative ${mode} failed: ${error.message || error}`, true);
      return null;
    } finally {
      if (this.analysisRequestToken === token) {
        MapEditorPanel.prototype.clearAuthoritativeAnalysisTimeout.call(this);
        this.analysisAbortController = null;
        this.analysisPending = false;
        this.render();
      }
    }
  }

  authoritativeAnalysisRequestIsCurrent(token, fingerprint) {
    if (this.destroyed || this.analysisRequestToken !== token) return false;
    return authoredMapFingerprint(this.session.exportMap()) === fingerprint;
  }

  clearAuthoritativeAnalysisTimeout() {
    if (this.analysisTimeoutId != null) this.clearTimeoutImpl?.(this.analysisTimeoutId);
    this.analysisTimeoutId = null;
  }

  invalidateAuthoritativeAnalysis() {
    this.analysisRequestToken = (this.analysisRequestToken || 0) + 1;
    this.analysisAbortController?.abort?.();
    this.analysisAbortController = null;
    MapEditorPanel.prototype.clearAuthoritativeAnalysisTimeout.call(this);
    this.analysisPending = false;
    this.analysisResult = null;
    this.analysisKind = null;
    if (this.analysisStatusOwned) {
      this.status = "";
      this.statusError = false;
      this.analysisStatusOwned = false;
    }
  }

  undo() {
    if (this.session.undo()) this.setStatus("Undid the last map edit.");
  }

  redo() {
    if (this.session.redo()) this.setStatus("Redid the map edit.");
  }

  chooseJsonFile() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json,application/json";
    input.addEventListener("change", () => {
      const file = input.files?.[0];
      if (file) void this.loadJsonFile(file);
    }, { once: true });
    input.click();
  }

  async loadJsonFile(file) {
    const name = String(file?.name || "selected file");
    try {
      if (Number(file?.size) > MAP_EDITOR_MAX_JSON_BYTES) {
        throw new Error("Map JSON files must be 2 MB or smaller.");
      }
      if (typeof file?.text !== "function") throw new Error("The selected file could not be read.");
      const text = await file.text();
      const kind = this.loadMapData(JSON.parse(text));
      this.setStatus(`Loaded ${kind === "recipe" ? "recipe " : ""}${name}.`);
    } catch (error) {
      this.setStatus(`Could not load ${name}: ${error.message || error}`, true);
    }
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
      this.session.markSaved();
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
      });
    } catch (error) {
      this.pending = false;
      this.setStatus(error.message || String(error), true);
    }
  }

  async openPreview() {
    if (this.pending) return;
    this.pending = true;
    this.setStatus("Validating map and preparing PNG previews…");
    try {
      await this.onOpenPreview?.({
        authoredMap: this.session.exportMap(),
        materializedMap: this.session.materialized(),
      });
      this.pending = false;
      this.setStatus("Opened the PNG preview page.");
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
    this.analysisStatusOwned = false;
    this.render();
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    MapEditorPanel.prototype.invalidateAuthoritativeAnalysis.call(this);
    window.removeEventListener("keydown", this.onKeyDown);
    this.unsubscribe?.();
    this.unsubscribeCamera?.();
    this.optionsWindowChrome.destroy();
    this.toolsWindowChrome.destroy();
    this.optionsEl.remove();
    this.toolsEl.remove();
  }
}

function authoredMapFingerprint(map) {
  return JSON.stringify(map ?? null);
}

function setAuthoritativeAnalysisStatus(panel, message, error = false) {
  panel.setStatus(message, error);
  panel.analysisStatusOwned = true;
}

function positiveTimeout(value, fallback) {
  const timeout = Number(value);
  return Number.isFinite(timeout) && timeout > 0 ? timeout : fallback;
}

export function authoritativeAnalysisSummary(kind, result) {
  if (result?.valid !== true) return result?.error || "Authoritative analysis rejected the map.";
  if (kind === "report") {
    const analyzed = nonnegativeIntegerOrUnknown(result.analyzedRouteCount);
    const unanalyzed = nonnegativeIntegerOrUnknown(result.unanalyzedRouteCount);
    const routes = Array.isArray(result.routes) ? result.routes : [];
    const unreachable = routes.filter((route) => route?.analyzed !== false && route?.reachable === false).length;
    return `Route report: ${analyzed} analyzed, ${unreachable} unreachable; ${unanalyzed} unanalyzed/truncated.`;
  }
  const bases = Array.isArray(result.baseSites) ? result.baseSites.length : 0;
  const starts = Array.isArray(result.startLocations) ? result.startLocations.length : 0;
  return `Authoritative check passed: ${bases} bases, ${starts} starts.`;
}

function nonnegativeIntegerOrUnknown(value) {
  const count = Number(value);
  return Number.isInteger(count) && count >= 0 ? String(count) : "unknown";
}

function renderAnalysisResult(kind, result) {
  const wrapper = document.createElement("div");
  wrapper.className = "map-editor-readout";
  wrapper.appendChild(readout(authoritativeAnalysisSummary(kind, result), result?.valid !== true));
  const details = document.createElement("details");
  const summary = document.createElement("summary");
  summary.textContent = "Full authoritative JSON";
  const json = document.createElement("pre");
  json.textContent = JSON.stringify(result, null, 2);
  details.append(summary, json);
  wrapper.appendChild(details);
  return wrapper;
}

function panelScroll(el) {
  const body = el.querySelector(".map-editor-panel-body");
  return body && { left: body.scrollLeft, top: body.scrollTop };
}

function restorePanelScroll(body, scroll) {
  if (!scroll) return;
  body.scrollLeft = scroll.left;
  body.scrollTop = scroll.top;
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

function patchCountField(labelText, value, max, onChange, disabled = false) {
  const input = document.createElement("input");
  input.type = "number";
  input.min = "0";
  input.max = String(max);
  input.step = "1";
  input.value = String(value ?? 0);
  input.disabled = disabled;
  input.addEventListener("change", () => onChange(input.value));
  return field(labelText, input);
}

function numericInput(value, min, max, onChange, ariaLabel) {
  const input = document.createElement("input");
  input.type = "number";
  input.min = String(min);
  input.max = String(max);
  input.step = "1";
  input.value = String(value);
  input.setAttribute("aria-label", ariaLabel);
  input.addEventListener("change", () => {
    const next = Math.max(min, Math.min(max, Math.trunc(Number(input.value)) || min));
    input.value = String(next);
    onChange(next);
  });
  return input;
}

function dimensionInput(label, value, onInput) {
  const input = document.createElement("input");
  input.type = "number";
  input.min = String(MAP_EDITOR_MIN_SIZE);
  input.max = String(MAP_EDITOR_MAX_SIZE);
  input.step = "1";
  input.value = value;
  input.className = "map-editor-blank-size";
  input.setAttribute("aria-label", label);
  input.addEventListener("input", () => onInput(input.value));
  return input;
}

function readout(text, error = false) {
  const node = document.createElement("p");
  node.className = "map-editor-readout";
  node.dataset.state = error ? "error" : "ok";
  node.textContent = text;
  return node;
}

function terrainName(code) {
  if (code === TERRAIN.GRAVEL_A) return "gravel-a";
  if (code === TERRAIN.GRAVEL_B) return "gravel-b";
  if (code === TERRAIN.GRAVEL_C) return "gravel-c";
  if (code === TERRAIN.DIRT_A) return "dirt-a";
  if (code === TERRAIN.DIRT_B) return "dirt-b";
  if (code === TERRAIN.DIRT_C) return "dirt-c";
  if (code === TERRAIN.MUD_A) return "mud-a";
  if (code === TERRAIN.MUD_B) return "mud-b";
  if (code === TERRAIN.MUD_C) return "mud-c";
  if (code === TERRAIN.FROSTED_GROUND) return "frosted-ground";
  if (code === TERRAIN.ROCK) return "stone";
  if (code === TERRAIN.WATER) return "water";
  if (code === TERRAIN.ROAD_BARE) return "road-bare";
  if (code === TERRAIN.ROAD_HORIZONTAL) return "road-horizontal";
  if (code === TERRAIN.ROAD_VERTICAL) return "road-vertical";
  if (code === TERRAIN.ROAD_DIAGONAL_NW_SE) return "road-diagonal-nw-se";
  if (code === TERRAIN.ROAD_DIAGONAL_NE_SW) return "road-diagonal-ne-sw";
  return "grass";
}

function normalizeCatalog(entries) {
  const maps = [];
  const skipped = [];
  if (!Array.isArray(entries)) return { maps, skipped };
  for (const entry of entries) {
    const file = String(entry?.file || "").trim();
    if (!safeMapFile(file)) {
      if (file) skipped.push(file);
      continue;
    }
    maps.push({
      file,
      name: String(entry?.name || file.replace(/\.json$/i, "")),
      description: String(entry?.description || ""),
    });
  }
  const nameCounts = new Map();
  for (const entry of maps) nameCounts.set(entry.name, (nameCounts.get(entry.name) || 0) + 1);
  for (const entry of maps) entry.label = nameCounts.get(entry.name) > 1 ? `${entry.name} — ${entry.file}` : entry.name;
  return { maps, skipped };
}

function safeMapFile(file) {
  return file.length > 0
    && file.length <= 128
    && /\.json$/i.test(file)
    && !file.includes("..")
    && !/[\\/\?#\x00-\x1f\x7f]/.test(file);
}

function slug(value) {
  return String(value || "map").trim().toLowerCase().replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "").slice(0, 64) || "map";
}

function isTextEntry(target) {
  return ["INPUT", "TEXTAREA", "SELECT"].includes(String(target?.tagName || "")) || !!target?.isContentEditable;
}
