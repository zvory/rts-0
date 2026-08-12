import { TERRAIN } from "./protocol.js";
import {
  createMapEditorBrushWidthInput,
  createMapEditorNumericInput,
} from "./map_editor_brush_controls.js";
import { createMapEditorElevationTool, selectMapEditorElevationOperation } from "./map_editor_elevation_controls.js";
import { LabPanelWindowChrome } from "./lab_panel_window.js";
import { MAP_AUTHORING_LAYERS } from "./map_authoring/layers.js";
import { mapSymmetryWarnings } from "./map_authoring/symmetry_validation.js";
import { createMapEditorPreviewButton } from "./map_editor_preview_button.js";
import { createMapEditorSunSettings } from "./map_editor_sun_controls.js";
import {
  MAP_EDITOR_CATEGORIES,
  MAP_EDITOR_OPERATIONS,
  activeMapEditorOperation,
  availableMapEditorOperations,
  mapEditorContentLabel,
  mapEditorOperationHelp,
  selectMapEditorCategoryState,
} from "./map_editor_panel_workflow.js";
import {
  canonicalDoodadColor,
  MAP_EDITOR_DEFAULT_FLOWER_COLOR,
  MAP_EDITOR_DOODAD_CATALOG,
  MAP_EDITOR_DOODAD_TYPES,
  MAP_EDITOR_MAX_DOODADS,
  MAP_EDITOR_MAX_SPRAY_DENSITY,
  isTreeDoodadType,
  isWildflowerDoodadType,
} from "./map_editor_doodads.js";
import {
  MAP_EDITOR_DEFAULT_SIZE,
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
const MAP_EDITOR_MAX_JSON_BYTES = 8 * 1024 * 1024;
const MAP_EDITOR_OPTIONS_STORAGE_KEY = "rts.mapEditor.panel.window.v1";
const MAP_EDITOR_TOOLS_STORAGE_KEY = "rts.mapEditor.tools.window.v1";
const MAP_EDITOR_LAYERS_STORAGE_KEY = "rts.mapEditor.layers.window.v1";
const MAP_EDITOR_PANEL_TOP_INSET = 70;
const MAP_EDITOR_ANALYSIS_TIMEOUT_MS = 20_000;
export class MapEditorPanel {
  constructor({
    root,
    session,
    viewport,
    onOpenLab,
    onShowPreview,
    onHidePreview,
    onCopyPreview,
    onInvalidatePreview,
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
    this.onShowPreview = onShowPreview;
    this.onHidePreview = onHidePreview;
    this.onCopyPreview = onCopyPreview;
    this.onInvalidatePreview = onInvalidatePreview;
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
    this.selectedElevation = 1;
    this.paintShape = "brush";
    this.terrainBrushWidth = 1;
    this.selectedOverlayEffects = new Set(["concealment"]);
    this.overlayMode = "paint";
    this.forestMode = "paint";
    this.forestBrushWidth = 9;
    this.roadWidth = 5;
    this.selectedDoodadType = MAP_EDITOR_DOODAD_TYPES.TREE_OAK;
    this.selectedTreeTypes = new Set([MAP_EDITOR_DOODAD_TYPES.TREE_OAK]);
    this.doodadMode = "place";
    this.doodadColor = MAP_EDITOR_DEFAULT_FLOWER_COLOR;
    this.doodadRadius = 48;
    this.doodadDensity = 4;
    this.symmetry = MAP_EDITOR_SYMMETRY.NONE;
    this.activeCategory = "terrain";
    this.terrainContent = "material";
    this.locationContent = "start";
    this.lastOperation = {
      terrain: "brush",
      objects: "place",
      zones: "brush",
      locations: "add",
    };
    this.mapSettingsOpen = false;
    this.layersOpen = !window.matchMedia?.("(max-width: 720px)")?.matches;
    this.categoryScroll = Object.fromEntries(MAP_EDITOR_CATEGORIES.map(([category]) => [category, 0]));
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
    this.status = "";
    this.statusError = false;
    this.analysisStatusOwned = false;
    this.destroyed = false;
    this.toolbarEl = document.createElement("nav");
    this.toolbarEl.className = "map-editor-toolbar";
    this.toolbarEl.setAttribute("aria-label", "Map document actions");
    this.toolRailEl = document.createElement("aside");
    this.toolRailEl.className = "map-editor-tool-rail";
    this.toolRailEl.setAttribute("aria-label", "Map editing operations");
    this.statusEl = document.createElement("div");
    this.statusEl.className = "map-editor-status-dock";
    this.optionsEl = this.createPanelElement("map-editor-options-window", "Map settings");
    this.toolsEl = this.createPanelElement("map-editor-tools-window", "Map content palette");
    this.layersEl = this.createPanelElement("map-editor-layers-window", "Map Editor layers");
    this.el = this.optionsEl;
    root.append(this.toolbarEl, this.toolRailEl, this.optionsEl, this.layersEl, this.toolsEl, this.statusEl);
    this.optionsWindowChrome = new LabPanelWindowChrome(this.optionsEl, {
      storageKey: MAP_EDITOR_OPTIONS_STORAGE_KEY,
      panelLabel: "map settings",
      minWidth: 220,
      topInset: MAP_EDITOR_PANEL_TOP_INSET,
    });
    this.toolsWindowChrome = new LabPanelWindowChrome(this.toolsEl, {
      storageKey: MAP_EDITOR_TOOLS_STORAGE_KEY,
      panelLabel: "map content palette",
      minWidth: 220,
      topInset: MAP_EDITOR_PANEL_TOP_INSET,
    });
    this.layersWindowChrome = new LabPanelWindowChrome(this.layersEl, {
      storageKey: MAP_EDITOR_LAYERS_STORAGE_KEY,
      panelLabel: "map editor layers",
      minWidth: 220,
      minHeight: 120,
      topInset: MAP_EDITOR_PANEL_TOP_INSET,
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
      this.onInvalidatePreview?.();
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
    MapEditorPanel.prototype.reconcileOperationAvailability.call(this);
    this.render();
  }

  render() {
    if (this.destroyed) return;
    this.renderToolbar();
    this.renderToolRail();
    this.renderOptionsWindow();
    this.renderLayersWindow();
    this.renderToolsWindow();
    this.renderStatusDock();
  }

  renderToolbar() {
    this.toolbarEl.replaceChildren();
    const identity = document.createElement("div");
    identity.className = "map-editor-toolbar-identity";
    const title = document.createElement("strong");
    title.textContent = this.session.draft?.name || "Untitled map";
    const dimensions = document.createElement("span");
    dimensions.textContent = this.session.draft
      ? `${this.session.draft.width} × ${this.session.draft.height}`
      : "Preparing…";
    identity.append(title, dimensions);

    const history = document.createElement("div");
    history.className = "map-editor-toolbar-group";
    history.append(
      button("Undo", () => this.undo(), { disabled: !this.session.undoStack.length, title: "Ctrl/Cmd-Z" }),
      button("Redo", () => this.redo(), { disabled: !this.session.redoStack.length, title: "Ctrl/Cmd-Shift-Z" }),
    );

    const view = document.createElement("div");
    view.className = "map-editor-toolbar-group";
    const zoomInput = document.createElement("input");
    const zoomLimits = this.viewport.zoomLimitsPercent();
    zoomInput.type = "number";
    zoomInput.min = String(zoomLimits.min);
    zoomInput.max = String(zoomLimits.max);
    zoomInput.step = "1";
    zoomInput.value = String(this.viewport.zoomPercent());
    zoomInput.className = "map-editor-toolbar-zoom";
    zoomInput.setAttribute("aria-label", "Zoom percentage");
    zoomInput.addEventListener("change", () => {
      zoomInput.value = String(this.viewport.setZoomPercent(zoomInput.value));
    });
    this.zoomInput = zoomInput;
    view.append(
      button("Map settings", () => {
        this.mapSettingsOpen = !this.mapSettingsOpen;
        this.render();
      }, { active: this.mapSettingsOpen }),
      button("Layers", () => {
        this.layersOpen = !this.layersOpen;
        this.render();
      }, {
        active: this.layersOpen && !this.mapSettingsOpen,
        disabled: this.mapSettingsOpen,
        title: this.mapSettingsOpen ? "Close Map settings to edit layer visibility." : "Toggle layer visibility controls.",
      }),
      button("Fit", () => this.viewport.fitToScreen(), { title: "Fit the whole map" }),
      button("Fill", () => this.viewport.fillScreen(), { title: "Fill the viewport" }),
      button("−", () => this.viewport.zoomOut(), { title: "Zoom out", className: "map-editor-zoom-step" }),
      zoomInput,
      document.createTextNode("%"),
      button("+", () => this.viewport.zoomIn(), { title: "Zoom in", className: "map-editor-zoom-step" }),
    );

    const workflow = document.createElement("div");
    workflow.className = "map-editor-toolbar-group map-editor-toolbar-workflow";
    workflow.append(
      button("Import", () => this.chooseJsonFile()),
      button("Export", () => this.exportJson()),
      button(this.analysisPending && this.analysisKind === "check" ? "Checking…" : "Check", () => void this.runAuthoritativeAnalysis("check"), {
        disabled: this.analysisPending,
      }),
      createMapEditorPreviewButton({
        session: this.session,
        onShow: this.onShowPreview,
        onHide: this.onHidePreview,
        onCopy: this.onCopyPreview,
        onStatus: (message, error) => this.setStatus(message, error),
      }),
      button(this.pending ? "Opening…" : "Open in Lab", () => void this.openLab(), {
        disabled: this.pending,
        className: "map-editor-primary",
      }),
    );
    this.toolbarEl.append(identity, history, view, workflow);
  }

  availableOperations() {
    return availableMapEditorOperations(this);
  }

  activeOperation() {
    return activeMapEditorOperation(this);
  }

  reconcileOperationAvailability() {
    const active = MapEditorPanel.prototype.activeOperation.call(this);
    if (!active || MapEditorPanel.prototype.availableOperations.call(this).has(active)) return true;
    this.viewport.armTool(null);
    return false;
  }

  operationHelp(operation) {
    return mapEditorOperationHelp(operation, this.activeCategory);
  }
  selectCategory(category) {
    if (!MAP_EDITOR_CATEGORIES.some(([value]) => value === category)) return;
    const preferred = selectMapEditorCategoryState(this, category);
    const available = this.availableOperations();
    const operation = available.has(preferred) ? preferred : available.values().next().value;
    if (operation) this.selectOperation(operation);
    else {
      this.viewport.armTool(null);
      this.render();
    }
  }

  selectOperation(operation) {
    if (!this.availableOperations().has(operation)) return false;
    if (this.activeCategory === "objects") {
      this.lastOperation.objects = operation;
      this.armDoodad(operation);
      this.setStatus(this.operationHelp(operation));
      return true;
    }
    if (this.activeCategory === "zones") {
      this.lastOperation.zones = operation;
      if (operation === "erase") this.armSelectedOverlays("erase");
      else {
        this.paintShape = operation === "box" ? "box" : "brush";
        this.armSelectedOverlays("paint");
      }
      return true;
    }
    if (this.activeCategory === "locations") {
      const kind = this.locationContent;
      const index = kind === "start" ? this.selectedStartIndex : this.session.mapOverlay()?.bases?.[this.selectedBaseIndex]?.index;
      if (operation === "remove") {
        this.removeLocation(kind, index);
        return true;
      }
      this.lastOperation.locations = operation;
      this.armLocation(kind, operation === "add" ? null : index, operation === "add");
      return true;
    }
    this.lastOperation.terrain = operation;
    if (this.terrainContent === "road" && operation === "path") {
      this.armRoad();
      this.setStatus(this.operationHelp(operation));
    } else if (this.terrainContent === "forest") {
      this.armForest(operation === "erase" ? "erase" : "paint");
    } else if (this.terrainContent === "elevation") {
      selectMapEditorElevationOperation(this, operation);
    } else {
      this.paintShape = operation === "box" ? "box" : "brush";
      this.armTerrain(operation === "erase" ? TERRAIN.GRASS : this.selectedTerrain);
      this.setStatus(this.operationHelp(operation));
    }
    return true;
  }

  renderContextSummary() {
    const section = document.createElement("section");
    section.className = "map-editor-context-summary";
    const summary = document.createElement("div");
    summary.className = "map-editor-current-tool";
    const heading = document.createElement("strong");
    heading.textContent = "Current tool";
    const detail = document.createElement("span");
    detail.textContent = `${this.activeOperation() || "None"} · ${this.currentContentLabel()}`;
    summary.append(heading, detail);
    section.append(summary, field("Symmetry", this.renderSymmetrySelect()));
    for (const warning of this.currentSymmetryWarnings()) {
      section.appendChild(readout(`Symmetry warning: ${warning}`, true));
    }
    return section;
  }

  renderSymmetrySelect() {
    const symmetry = document.createElement("select");
    symmetry.setAttribute("aria-label", "Symmetry");
    symmetry.title = "Symmetry applies to terrain, zones, objects, forests, roads, and locations.";
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
    return symmetry;
  }

  currentContentLabel() {
    return mapEditorContentLabel(this, overlayEffectName, terrainName);
  }

  renderToolRail() {
    this.toolRailEl.replaceChildren();
    this.toolRailEl.hidden = this.mapSettingsOpen;
    const label = document.createElement("span");
    label.className = "map-editor-tool-rail-label";
    label.textContent = "Apply";
    this.toolRailEl.appendChild(label);
    const available = this.availableOperations();
    const active = this.activeOperation();
    for (const [operation, operationLabel] of MAP_EDITOR_OPERATIONS) {
      const enabled = available.has(operation);
      const control = button(operationLabel, () => this.selectOperation(operation), {
        disabled: !enabled,
        active: enabled && active === operation,
        pressed: enabled ? active === operation : null,
        title: enabled ? this.operationHelp(operation) : `${operationLabel} is unavailable for ${this.activeCategory}.`,
        className: "map-editor-tool-rail-button",
      });
      if (!enabled) control.setAttribute("aria-label", `${operationLabel}. Unavailable for ${this.currentContentLabel()}.`);
      this.toolRailEl.appendChild(control);
    }
  }

  renderStatusDock() {
    this.statusEl.replaceChildren();
    const status = this.renderStatus();
    if (status) this.statusEl.appendChild(status);
  }

  renderOptionsWindow() {
    const scroll = panelScroll(this.optionsEl);
    this.optionsEl.replaceChildren();
    const header = this.optionsWindowChrome.renderHeader({
      kicker: "Map settings",
      collapseLabel: "map settings panel",
    });
    header.classList.add("map-editor-header");
    const body = document.createElement("div");
    body.className = "lab-panel-body map-editor-panel-body";
    if (!this.session.draft) {
      body.appendChild(readout("Preparing editor…"));
    } else {
      body.append(
        this.renderMapSource(),
        this.renderDetails(),
        createMapEditorSunSettings(this.session, this.viewport),
        this.renderDocumentUtilities(),
      );
    }
    this.optionsEl.append(header, body, this.optionsWindowChrome.renderResizeHandle());
    this.optionsEl.hidden = !this.mapSettingsOpen;
    restorePanelScroll(body, scroll);
  }

  renderToolsWindow() {
    const priorContent = this.toolsEl.querySelector(".map-editor-category-content");
    const priorCategory = priorContent?.dataset.category;
    if (priorCategory && priorCategory in this.categoryScroll) this.categoryScroll[priorCategory] = priorContent.scrollTop;
    this.toolsEl.replaceChildren();
    const header = this.toolsWindowChrome.renderHeader({
      kicker: "Palette",
      collapseLabel: "map content palette",
    });
    header.classList.add("map-editor-header");
    const body = document.createElement("div");
    body.className = "lab-panel-body map-editor-panel-body";
    let content = null;
    if (!this.session.draft) {
      body.appendChild(readout("Preparing editor…"));
    } else {
      const tabs = document.createElement("div");
      tabs.className = "map-editor-category-tabs";
      for (const [category, label] of MAP_EDITOR_CATEGORIES) {
        const tab = button(label, () => this.selectCategory(category), {
          active: this.activeCategory === category,
          pressed: this.activeCategory === category,
          className: "map-editor-category-tab",
        });
        tabs.appendChild(tab);
      }
      content = document.createElement("div");
      content.className = "map-editor-category-content";
      content.dataset.category = this.activeCategory;
      if (this.activeCategory === "objects") content.appendChild(this.renderDoodads());
      else if (this.activeCategory === "elevation") content.appendChild(createMapEditorElevationTool(this));
      else if (this.activeCategory === "zones") content.appendChild(this.renderMapOverlays());
      else if (this.activeCategory === "locations") content.appendChild(this.renderLocations());
      else content.append(this.renderTerrain(), this.renderForest());
      body.append(tabs, content, this.renderContextSummary());
    }
    this.toolsEl.append(header, body, this.toolsWindowChrome.renderResizeHandle());
    if (content) content.scrollTop = this.categoryScroll[this.activeCategory] || 0;
    this.toolsEl.hidden = this.mapSettingsOpen;
  }

  renderLayersWindow() {
    this.layersEl.replaceChildren();
    const header = this.layersWindowChrome.renderHeader({
      kicker: "Layers",
      collapseLabel: "map editor layers panel",
    });
    header.classList.add("map-editor-header");
    const body = document.createElement("div");
    body.className = "lab-panel-body map-editor-panel-body map-editor-layers-body";
    if (!this.session.draft) body.appendChild(readout("Preparing editor…"));
    else body.appendChild(this.renderLayers());
    this.layersEl.append(header, body, this.layersWindowChrome.renderResizeHandle());
    this.layersEl.hidden = !this.layersOpen || this.mapSettingsOpen;
  }

  updateZoomControl(percent = this.viewport.zoomPercent()) {
    if (!this.zoomInput?.isConnected) return;
    this.zoomInput.value = String(percent);
  }

  renderLayers() {
    const list = document.createElement("div");
    list.className = "map-editor-layer-list";
    list.setAttribute("aria-label", "Visible layers");
    const visibility = this.viewport.layerVisibilitySnapshot();
    for (const layer of MAP_AUTHORING_LAYERS) {
      const input = document.createElement("input");
      input.type = "checkbox";
      input.checked = visibility[layer.id];
      input.setAttribute("aria-label", `Show ${layer.label}`);
      input.addEventListener("change", () => this.viewport.setLayerVisibility(layer.id, input.checked));
      const label = document.createElement("label");
      label.className = "map-editor-layer-toggle";
      label.title = `${layer.label} — ${layer.description}`;
      const text = document.createElement("span");
      text.textContent = layer.label;
      const description = document.createElement("small");
      description.textContent = layer.description;
      label.append(input, text, description);
      list.appendChild(label);
    }
    return list;
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
    const width = createMapEditorBrushWidthInput(this.terrainBrushWidth, (value) => {
      this.terrainBrushWidth = value;
      if (this.viewport.tool?.kind === "terrain" && this.viewport.tool.shape === "brush") {
        this.armTerrain(this.viewport.tool.terrain);
      }
    }, "Terrain brush width in tiles");
    const palette = document.createElement("div");
    palette.className = "map-editor-palette";
    for (const [code, label] of [
      [TERRAIN.GRASS, "Grass"],
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
        this.terrainContent = "material";
        this.selectedTerrain = code;
        if (!["brush", "box", "erase"].includes(this.lastOperation.terrain)) this.lastOperation.terrain = "brush";
        this.selectOperation(this.lastOperation.terrain);
      }, { active: this.terrainContent === "material" && this.selectedTerrain === code });
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
    section.append(field("Brush width (tiles)", width), palette, this.renderRoadTool());
    return section;
  }
  renderRoadTool() {
    const controls = document.createElement("div");
    controls.className = "map-editor-road-tool";
    const width = document.createElement("input");
    width.type = "number";
    width.min = "1";
    width.max = "15";
    width.step = "1";
    width.value = String(this.roadWidth);
    width.setAttribute("aria-label", "Road width in tiles");
    width.addEventListener("change", () => {
      this.roadWidth = Math.max(1, Math.min(15, Math.trunc(Number(width.value)) || 5));
      width.value = String(this.roadWidth);
      if (this.viewport.tool?.kind === "road") this.armRoad();
    });
    controls.append(
      field("Width (tiles)", width),
      button("Automatic road", () => {
        this.terrainContent = "road";
        this.lastOperation.terrain = "path";
        this.selectOperation("path");
        this.setStatus("Drag to lay a road. The path snaps to horizontal, vertical, or diagonal and adds yellow centre markers.");
      }, { active: this.terrainContent === "road" }),
      readout("Drag in any of 8 directions. Road edges are bare; the centre line is marked automatically."),
    );
    return field("Road tool", controls);
  }

  currentSymmetryWarnings() {
    return mapSymmetryWarnings(this.session.draft, this.symmetry);
  }

  renderMapOverlays() {
    const section = group("Gameplay overlays");
    const palette = document.createElement("div");
    palette.className = "map-editor-palette";
    for (const [key, label] of [
      ["concealment", "Concealment"],
      ["noVehicle", "No vehicles"],
      ["noBuilding", "No buildings"],
      ["noEntrenchment", "No entrenchment"],
      ["damageReduction", "Damage reduction"],
      ["slowMovement", "Slowed movement"],
    ]) {
      const input = document.createElement("input");
      input.type = "checkbox";
      input.checked = this.selectedOverlayEffects.has(key);
      input.addEventListener("change", () => {
        if (input.checked) this.selectedOverlayEffects.add(key);
        else this.selectedOverlayEffects.delete(key);
        if (!this.selectedOverlayEffects.size) {
          this.viewport.armTool(null);
          this.setStatus("Select at least one zone effect to enable Brush, Box, or Erase.", true);
        } else if (this.viewport.tool?.kind === "overlay") this.armSelectedOverlays(this.overlayMode);
        else this.selectOperation(this.lastOperation.zones);
      });
      const control = document.createElement("label");
      control.className = "map-editor-overlay-toggle";
      control.append(input, document.createTextNode(label));
      palette.appendChild(control);
    }
    section.append(
      readout(`${this.session.draft.concealmentTiles.length} concealment; ${this.session.draft.noVehicleTiles.length} no-vehicle; ${this.session.draft.noBuildingTiles.length} no-building; ${this.session.draft.noEntrenchmentTiles.length} no-entrenchment; ${this.session.draft.damageReductionTiles.length} damage-reduction; ${this.session.draft.slowMovementTiles.length} slowed tiles.`),
      readout("Select any combination, then paint or erase all selected effects in one stroke. Damage reduction and slowed movement each reduce their affected value by 25%."),
      palette,
    );
    return section;
  }

  renderForest() {
    const section = group("Forest");
    const width = createMapEditorBrushWidthInput(this.forestBrushWidth, (value) => {
      this.forestBrushWidth = value;
      if (this.viewport.tool?.kind === "forest") this.armForest(this.forestMode);
    }, "Forest brush width in tiles");
    const tileCount = this.session.forestTiles().length;
    section.append(
      button("Forest tile", () => {
        this.terrainContent = "forest";
        this.lastOperation.terrain = "brush";
        this.selectOperation("brush");
      }, { active: this.terrainContent === "forest" }),
      field("Brush width (tiles)", width),
      readout(`${tileCount} forest tile${tileCount === 1 ? "" : "s"}. Painting a forest adds its trees and all five gameplay effects together.`),
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
        if (this.locationContent === "start" && this.lastOperation.locations === "move") this.selectOperation("move");
        else this.render();
      }, { active: index === this.selectedStartIndex, title: `${start.x}, ${start.y}` }));
    }
    const basePicker = document.createElement("div");
    basePicker.className = "map-editor-player-picker";
    for (const [index, base] of bases.entries()) {
      basePicker.appendChild(button(`B${index + 1}`, () => {
        this.selectedBaseIndex = index;
        this.viewport.setSelectedBase(base.index);
        if (this.locationContent === "base" && this.lastOperation.locations === "move") this.selectOperation("move");
        else this.render();
      }, { active: index === this.selectedBaseIndex, title: `${base.x}, ${base.y}` }));
    }
    const start = starts[this.selectedStartIndex];
    const base = bases[this.selectedBaseIndex];
    const startBaseIndex = start
      ? this.session.draft.baseSites.findIndex((site) => site.x === start.x && site.y === start.y)
      : -1;
    const startBase = this.session.draft.baseSites[startBaseIndex];
    this.viewport.setSelectedBase(base?.index ?? null);
    const locationTypes = document.createElement("div");
    locationTypes.className = "map-editor-palette";
    locationTypes.append(
      button("Player starts", () => {
        this.locationContent = "start";
        const operations = this.availableOperations();
        const preferred = this.lastOperation.locations;
        const operation = operations.has(preferred) ? preferred : operations.values().next().value;
        if (operation) this.selectOperation(operation);
        else {
          this.viewport.armTool(null);
          this.render();
        }
      }, { active: this.locationContent === "start" }),
      button("Neutral bases", () => {
        this.locationContent = "base";
        const operations = this.availableOperations();
        const preferred = this.lastOperation.locations;
        const operation = operations.has(preferred) ? preferred : operations.values().next().value;
        if (operation) this.selectOperation(operation);
        else {
          this.viewport.armTool(null);
          this.render();
        }
      }, { active: this.locationContent === "base" }),
    );
    section.append(
      readout(`Start locations set player capacity (${starts.length}/${MAP_EDITOR_MAX_START_LOCATIONS}). Drafts may temporarily have none. Every base site always spawns resources.`),
      locationTypes,
      readout("Bases and starts reserve a passable grass area."),
    );
    if (this.locationContent === "start") {
      section.append(
        startPicker,
        readout(start ? `Start ${this.selectedStartIndex + 1}: ${start.x}, ${start.y}` : "No start locations yet. Choose Add, then click the map."),
        patchCountField("Start-base steel patches", startBase?.steelPatches, MAP_EDITOR_MAX_STEEL_PATCHES, (value) => {
          this.updateBasePatchCount(startBaseIndex, "steelPatches", value);
        }, !startBase),
        patchCountField("Start-base oil patches", startBase?.oilPatches, MAP_EDITOR_MAX_OIL_PATCHES, (value) => {
          this.updateBasePatchCount(startBaseIndex, "oilPatches", value);
        }, !startBase),
      );
    } else {
      section.append(
        basePicker,
        readout(base ? `Base ${this.selectedBaseIndex + 1}: ${base.x}, ${base.y}` : "No neutral base sites yet."),
        patchCountField("Base steel patches", base?.steelPatches, MAP_EDITOR_MAX_STEEL_PATCHES, (value) => {
          this.updateBasePatchCount(base?.index, "steelPatches", value);
        }, !base),
        patchCountField("Base oil patches", base?.oilPatches, MAP_EDITOR_MAX_OIL_PATCHES, (value) => {
          this.updateBasePatchCount(base?.index, "oilPatches", value);
        }, !base),
      );
    }
    return section;
  }

  renderDoodads() {
    const section = group(`Doodads (${this.session.draft.doodads.length} / ${MAP_EDITOR_MAX_DOODADS})`);
    const trees = MAP_EDITOR_DOODAD_CATALOG.filter((entry) => entry.kind === "tree");
    const flowers = MAP_EDITOR_DOODAD_CATALOG.filter((entry) => entry.kind === "wildflower");
    const neutralUnits = MAP_EDITOR_DOODAD_CATALOG.filter((entry) => entry.kind === "neutral-unit");
    const treePalette = this.renderDoodadPalette(trees, { multiple: true });
    const flowerPalette = this.renderDoodadPalette(flowers);
    const neutralUnitPalette = this.renderDoodadPalette(neutralUnits);

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

    const radius = createMapEditorNumericInput(this.doodadRadius, 4, 256, (value) => {
      this.doodadRadius = value;
      if (this.viewport.tool?.kind === "doodad") this.armDoodad(this.doodadMode);
    }, "Doodad brush radius");
    const density = createMapEditorNumericInput(this.doodadDensity, 1, MAP_EDITOR_MAX_SPRAY_DENSITY, (value) => {
      this.doodadDensity = value;
      if (this.viewport.tool?.kind === "doodad") this.armDoodad(this.doodadMode);
    }, "Doodad spray density");
    section.append(
      readout("Trees"),
      treePalette,
      readout("Wildflowers"),
      flowerPalette,
      readout("Neutral units"),
      neutralUnitPalette,
      ...(isWildflowerDoodadType(this.selectedDoodadType) ? [field("Flower color", color)] : []),
      ...(["spray", "erase"].includes(this.lastOperation.objects) ? [field("Brush radius (world px)", radius)] : []),
      ...(this.lastOperation.objects === "spray" ? [field("Spray density", density)] : []),
      readout("Place adds one doodad. Spray and erase work continuously while held; symmetry applies when placing and erasing doodads."),
    );
    return section;
  }

  renderDoodadPalette(entries, { multiple = false } = {}) {
    const palette = document.createElement("div");
    palette.className = "map-editor-palette map-editor-doodad-palette";
    for (const entry of entries) {
      palette.appendChild(button(entry.label, () => {
        if (multiple) {
          if (this.selectedTreeTypes.has(entry.typeId) && this.selectedTreeTypes.size > 1) {
            this.selectedTreeTypes.delete(entry.typeId);
            this.selectedDoodadType = this.selectedTreeTypes.values().next().value;
          } else {
            this.selectedTreeTypes.add(entry.typeId);
            this.selectedDoodadType = entry.typeId;
          }
        } else this.selectedDoodadType = entry.typeId;
        if (!isTreeDoodadType(this.selectedDoodadType) && !isWildflowerDoodadType(this.selectedDoodadType)
          && this.lastOperation.objects === "spray") this.lastOperation.objects = "place";
        this.selectOperation(this.lastOperation.objects);
        if (multiple) this.setStatus(`Tree mix: ${this.selectedTreeTypes.size} species selected.`);
      }, {
        active: multiple
          ? this.selectedTreeTypes.has(entry.typeId)
          : this.viewport.tool?.kind === "doodad"
            && !["remove", "erase"].includes(this.viewport.tool?.mode)
            && this.selectedDoodadType === entry.typeId,
        pressed: multiple ? this.selectedTreeTypes.has(entry.typeId) : null,
      }));
    }
    return palette;
  }

  renderDocumentUtilities() {
    const section = group("Advanced validation");
    section.append(
      button(this.analysisPending && this.analysisKind === "report" ? "Reporting routes…" : "Route report", () => void this.runAuthoritativeAnalysis("report"), {
        disabled: this.analysisPending,
      }),
      readout("Check, Preview, Import, Export, and Open in Lab stay available in the document bar."),
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

  armTerrain(terrain = this.selectedTerrain) {
    this.viewport.armTool({
      kind: "terrain",
      terrain,
      shape: this.paintShape,
      width: this.terrainBrushWidth,
      symmetry: this.symmetry,
    });
  }

  armRoad() {
    this.viewport.armTool({
      kind: "road",
      width: this.roadWidth,
      symmetry: this.symmetry,
    });
    this.render();
  }

  armSelectedOverlays(mode) {
    if (!this.selectedOverlayEffects.size) {
      if (this.viewport.tool?.kind === "overlay") this.viewport.armTool(null);
      this.setStatus("Select at least one gameplay overlay first.", true);
      return;
    }
    this.overlayMode = mode === "erase" ? "erase" : "paint";
    const value = this.overlayMode === "paint";
    const edit = Object.fromEntries([...this.selectedOverlayEffects].map((key) => [key, value]));
    const names = [...this.selectedOverlayEffects].map((key) => overlayEffectName(key)).join(", ");
    this.armOverlay(edit, `${this.overlayMode === "paint" ? "painted" : "erased"} ${names}`);
    this.setStatus(`${this.paintShape === "box" ? "Drag to fill a box" : "Paint"} to ${this.overlayMode} ${names}.`);
  }

  armForest(mode) {
    this.forestMode = mode === "erase" ? "erase" : "paint";
    this.viewport.armTool({
      kind: "forest",
      paint: this.forestMode === "paint",
      width: this.forestBrushWidth,
      symmetry: this.symmetry,
    });
    this.setStatus(`${this.forestMode === "paint" ? "Paint" : "Erase"} forest with the ${this.forestBrushWidth}-tile brush.`);
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
    this.doodadMode = ["place", "spray", "erase"].includes(mode) ? mode : "place";
    this.viewport.armTool({
      kind: "doodad",
      mode: this.doodadMode,
      typeId: this.selectedDoodadType,
      typeIds: isTreeDoodadType(this.selectedDoodadType)
        ? [...this.selectedTreeTypes]
        : [this.selectedDoodadType],
      color: isWildflowerDoodadType(this.selectedDoodadType) ? this.doodadColor : null,
      radius: this.doodadRadius,
      density: this.doodadDensity,
      symmetry: this.symmetry,
    });
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
    if (!Array.isArray(map?.terrain)) throw new Error("Map JSON needs a terrain array.");
    this.session.loadAuthoredMap(map);
    this.selectedStartIndex = 0;
    this.selectedBaseIndex = 0;
    MapEditorPanel.prototype.invalidateAuthoritativeAnalysis.call(this);
    this.viewport.armTool(null);
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
        throw new Error("Map JSON files must be 8 MiB or smaller.");
      }
      if (typeof file?.text !== "function") throw new Error("The selected file could not be read.");
      const text = await file.text();
      this.loadMapData(JSON.parse(text));
      this.setStatus(`Loaded ${name}.`);
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
    this.layersWindowChrome.destroy();
    this.toolbarEl.remove();
    this.toolRailEl.remove();
    this.statusEl.remove();
    this.optionsEl.remove();
    this.toolsEl.remove();
    this.layersEl.remove();
  }
}

function overlayEffectName(key) {
  if (key === "noVehicle") return "no vehicles";
  if (key === "noBuilding") return "no buildings";
  if (key === "noEntrenchment") return "no entrenchment";
  if (key === "damageReduction") return "damage reduction";
  if (key === "slowMovement") return "slowed movement";
  return "concealment";
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

function button(label, onClick, { disabled = false, active = false, pressed = null, title = "", className = "" } = {}) {
  const control = document.createElement("button");
  control.type = "button";
  control.className = `map-editor-button ${className}`.trim();
  control.textContent = label;
  control.disabled = !!disabled;
  control.dataset.active = active ? "true" : "false";
  if (pressed != null) control.setAttribute("aria-pressed", String(!!pressed));
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
