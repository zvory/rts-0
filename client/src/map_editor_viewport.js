import { Camera } from "./camera.js";
import { createMapEditorPresentation } from "./map_editor_presentation.js";
import { createMapEditorTerrainPreview } from "./map_editor_terrain_preview.js";
import { mapEditorResourcePatches } from "./map_editor_resource_patches.js";
import { PRESENTATION_OUTCOME } from "./presentation/submission.js";
import { TERRAIN } from "./protocol.js";
import { MapEditorPixiPresentationAdapter } from "./renderer/map_editor_presentation_adapter.js";
import { lineTiles, pathTiles } from "./map_authoring/geometry.js";
import {
  defaultMapAuthoringLayerVisibility,
  MAP_AUTHORING_LAYER_IDS,
} from "./map_authoring/layers.js";
import {
  allocateMapEditorDoodadId,
  createDoodadSprayStroke,
  doodadTypeFromSelection,
  doodadIdsWithinRadius,
  extendDoodadSprayStroke,
  MAP_EDITOR_MAX_DOODADS,
  symmetricDoodadPlacements,
} from "./map_editor_doodads.js";
import {
  addSymmetricDraftLocations,
  mapEditorRectTiles,
  MAP_EDITOR_BASE_SITE_CLEARANCE_TILES,
  MAP_EDITOR_MAIN_CLEARANCE_TILES,
  MAP_EDITOR_SYMMETRY,
  mapEditorSymmetrySupported,
  moveSymmetricDraftLocation,
  protectDraftBaseTerrain,
  symmetricMapTiles,
  symmetricTerrainTiles,
} from "./map_editor_session.js";

const TILE_SIZE = 32;
const MAP_EDITOR_ZOOM_STEP = 1.25;

export function mapEditorSymmetryGuideLines(dimensions, symmetry) {
  const { width, height } = mapDimensions(dimensions);
  const worldWidth = width * TILE_SIZE;
  const worldHeight = height * TILE_SIZE;
  const centreX = worldWidth / 2;
  const centreY = worldHeight / 2;
  const horizontal = { x0: 0, y0: centreY, x1: worldWidth, y1: centreY };
  const vertical = { x0: centreX, y0: 0, x1: centreX, y1: worldHeight };
  if (symmetry === MAP_EDITOR_SYMMETRY.HORIZONTAL) return [horizontal];
  if (symmetry === MAP_EDITOR_SYMMETRY.VERTICAL) return [vertical];
  if (symmetry === MAP_EDITOR_SYMMETRY.THREE_WAY) {
    return [-Math.PI / 2, Math.PI / 6, 5 * Math.PI / 6]
      .map((angle) => symmetryGuideRay(centreX, centreY, worldWidth, worldHeight, angle));
  }
  if (symmetry === MAP_EDITOR_SYMMETRY.RADIAL) return [horizontal, vertical];
  if (symmetry === MAP_EDITOR_SYMMETRY.DIAGONAL_MAIN) {
    return [{ x0: 0, y0: 0, x1: worldWidth, y1: worldHeight }];
  }
  if (symmetry === MAP_EDITOR_SYMMETRY.DIAGONAL_ANTI) {
    return [{ x0: 0, y0: worldHeight, x1: worldWidth, y1: 0 }];
  }
  return [];
}

function symmetryGuideRay(centreX, centreY, worldWidth, worldHeight, angle) {
  const dx = Math.cos(angle);
  const dy = Math.sin(angle);
  const distances = [];
  if (dx > 0) distances.push((worldWidth - centreX) / dx);
  if (dx < 0) distances.push(-centreX / dx);
  if (dy > 0) distances.push((worldHeight - centreY) / dy);
  if (dy < 0) distances.push(-centreY / dy);
  const distance = Math.min(...distances.filter((candidate) => candidate >= 0));
  return {
    x0: centreX,
    y0: centreY,
    x1: centreX + dx * distance,
    y1: centreY + dy * distance,
  };
}

export function mapEditorSymmetryGuideCentre(dimensions, symmetry) {
  if (symmetry !== MAP_EDITOR_SYMMETRY.HALF_TURN) return null;
  const { width, height } = mapDimensions(dimensions);
  return { x: width * TILE_SIZE / 2, y: height * TILE_SIZE / 2 };
}

export function mapEditorSunDirectionPreview(dimensions, azimuthDegrees) {
  const { width, height } = mapDimensions(dimensions);
  const degrees = Math.max(0, Math.min(359, Math.trunc(Number(azimuthDegrees)) || 0));
  const radians = degrees * Math.PI / 180;
  const centreX = width * TILE_SIZE / 2;
  const centreY = height * TILE_SIZE / 2;
  const length = Math.max(160, Math.min(640, Math.min(width, height) * TILE_SIZE * 0.18));
  return {
    fromX: centreX,
    fromY: centreY,
    toX: centreX + Math.sin(radians) * length,
    toY: centreY - Math.cos(radians) * length,
    azimuthDegrees: degrees,
    label: `Sun source · ${degrees}° ${compassPoint(degrees)}`,
  };
}

export class MapEditorViewport {
  static async create(options) {
    const presentation = await MapEditorPixiPresentationAdapter.create(options.root);
    return new MapEditorViewport({ ...options, presentation });
  }

  constructor({ root, session, onStatus = () => {}, presentation }) {
    this.root = root;
    this.session = session;
    this.onStatus = onStatus;
    if (!presentation) throw new TypeError("MapEditorViewport.create() must prepare presentation.");
    this.presentation = presentation;
    this.camera = new Camera(root.clientWidth, root.clientHeight, {
      minZoom: 0.05,
      maxZoom: 4,
    });
    this.tool = null;
    this.symmetry = MAP_EDITOR_SYMMETRY.NONE;
    this.selectedBaseIndex = null;
    this.sunDirectionPreviewDegrees = null;
    this.paintPointerId = null;
    this.doodadPointerId = null;
    this.doodadPointerMode = null;
    this.doodadSprayStroke = null;
    this.doodadLastWorld = null;
    this.doodadBrushPoint = null;
    this.panPointerId = null;
    this.lastPointer = null;
    this.lastPaintTile = null;
    this.paintStartTile = null;
    this.keys = { up: false, down: false, left: false, right: false };
    this.destroyed = false;
    this.presentationFrameId = 0;
    this.terrainRevision = 0;
    this.overlayRevision = 0;
    this.doodadRevision = 0;
    this.resourcePatchRevision = -1;
    this.resourcePatches = [];
    this.pendingTerrainUpdate = null;
    this.pendingOverlay = null;
    this.pendingDoodadUpdate = null;
    this.layerVisibility = defaultMapAuthoringLayerVisibility();
    this.presentationInFlight = null;
    this.presentationStopped = false;
    this.visualTimeMs = 0;

    this.onPointerDown = (event) => this.handlePointerDown(event);
    this.onPointerMove = (event) => this.handlePointerMove(event);
    this.onPointerUp = (event) => this.handlePointerUp(event);
    this.onWheel = (event) => this.handleWheel(event);
    this.onContextMenu = (event) => event.preventDefault();
    this.onKeyDown = (event) => this.handleKey(event, true);
    this.onKeyUp = (event) => this.handleKey(event, false);
    this.onResize = () => this.resize();
    const canvas = this.presentation.canvas;
    canvas.addEventListener("pointerdown", this.onPointerDown);
    canvas.addEventListener("pointermove", this.onPointerMove);
    canvas.addEventListener("pointerup", this.onPointerUp);
    canvas.addEventListener("pointercancel", this.onPointerUp);
    canvas.addEventListener("wheel", this.onWheel, { passive: false });
    canvas.addEventListener("contextmenu", this.onContextMenu);
    window.addEventListener("keydown", this.onKeyDown);
    window.addEventListener("keyup", this.onKeyUp);
    window.addEventListener("resize", this.onResize);
    this.unsubscribe = session.subscribe((snapshot) => this.applySessionSnapshot(snapshot));
    this.lastFrameAt = performance.now();
    this.frame = requestAnimationFrame((at) => this.tick(at));
  }

  armTool(tool) {
    this.tool = tool ? structuredCloneSafe(tool) : null;
    if (this.tool?.symmetry) {
      this.symmetry = mapEditorSymmetrySupported(this.session.draft, this.tool.symmetry)
        ? this.tool.symmetry
        : MAP_EDITOR_SYMMETRY.NONE;
      this.tool.symmetry = this.symmetry;
    }
    this.drawOverlay();
    return this.tool;
  }

  setSymmetry(symmetry) {
    this.symmetry = Object.values(MAP_EDITOR_SYMMETRY).includes(symmetry)
      && mapEditorSymmetrySupported(this.session.draft, symmetry)
      ? symmetry
      : MAP_EDITOR_SYMMETRY.NONE;
    this.drawOverlay();
  }

  setLayerVisibility(layerId, visible) {
    if (!MAP_AUTHORING_LAYER_IDS.includes(layerId)) {
      throw new RangeError(`Unsupported Map Editor layer ${JSON.stringify(layerId)}`);
    }
    const next = !!visible;
    if (this.layerVisibility[layerId] === next) return false;
    this.layerVisibility[layerId] = next;
    this.drawOverlay();
    return true;
  }

  layerVisibilitySnapshot() {
    return { ...this.layerVisibility };
  }

  setSelectedBase(locationIndex) {
    const index = Number.isInteger(locationIndex) && locationIndex >= 0 ? locationIndex : null;
    if (this.selectedBaseIndex === index) return;
    this.selectedBaseIndex = index;
    this.drawOverlay();
  }

  createTerrainPreview(terrain) {
    return createMapEditorTerrainPreview(terrain);
  }

  setZoomPercent(percent) {
    const value = Number(percent);
    if (!Number.isFinite(value)) return this.zoomPercent();
    this.camera.setZoom(value / 100);
    return this.zoomPercent();
  }

  zoomIn() {
    this.camera.setZoom(this.camera.zoom * MAP_EDITOR_ZOOM_STEP);
    return this.zoomPercent();
  }

  zoomOut() {
    this.camera.setZoom(this.camera.zoom / MAP_EDITOR_ZOOM_STEP);
    return this.zoomPercent();
  }

  fitToScreen() {
    return this.frameMap(false);
  }

  fillScreen() {
    return this.frameMap(true);
  }

  frameMap(fill) {
    const { worldW, worldH, viewW, viewH } = this.camera;
    if (!(worldW > 0 && worldH > 0 && viewW > 0 && viewH > 0)) return false;
    const widthZoom = viewW / worldW;
    const heightZoom = viewH / worldH;
    this.camera.setView({
      centerX: worldW / 2,
      centerY: worldH / 2,
      zoom: fill ? Math.max(widthZoom, heightZoom) : Math.min(widthZoom, heightZoom),
    });
    return true;
  }

  zoomPercent() {
    return Math.round(this.camera.zoom * 100);
  }

  zoomLimitsPercent() {
    return {
      min: Math.round(this.camera.minZoom * 100),
      max: Math.round(this.camera.maxZoom * 100),
    };
  }

  cameraSnapshot() {
    return this.camera.snapshot();
  }

  cameraViewportSnapshot() {
    return { widthCssPx: this.camera.viewW, heightCssPx: this.camera.viewH };
  }

  cameraWorldBoundsSnapshot() {
    return {
      minX: this.camera.x,
      minY: this.camera.y,
      maxX: this.camera.x + this.camera.viewW / this.camera.zoom,
      maxY: this.camera.y + this.camera.viewH / this.camera.zoom,
    };
  }

  async controlInteractCamera(input) {
    const action = String(input?.action || "");
    if (action === "overview") {
      this.fitToScreen();
    } else if (action === "zoom") {
      const focus = this.camera.snapshot().focus;
      this.camera.setView({ centerX: focus.x, centerY: focus.y, zoom: Number(input.zoom) });
    } else if (action === "focus") {
      const scale = input.space === "tile" ? TILE_SIZE : 1;
      const x = Number(input.x) * scale;
      const y = Number(input.y) * scale;
      const width = input.width == null ? 0 : Number(input.width) * scale;
      const height = input.height == null ? 0 : Number(input.height) * scale;
      if (width > 0 && height > 0) {
        this.camera.fitWorldPoints([
          { x, y }, { x: x + width, y }, { x: x + width, y: y + height }, { x, y: y + height },
        ], { paddingCssPx: Number(input.padding || 0) });
      } else {
        this.camera.setView({ centerX: x, centerY: y, zoom: input.zoom == null ? this.camera.zoom : Number(input.zoom) });
      }
    } else {
      throw Object.assign(new Error("Map Editor camera action must be overview, zoom, or focus."), { code: "invalidCamera" });
    }
    await animationFrames(2);
    return {
      camera: this.cameraSnapshot(),
      cameraViewport: this.cameraViewportSnapshot(),
      cameraWorldBounds: this.cameraWorldBoundsSnapshot(),
    };
  }

  subscribeZoom(listener) {
    if (typeof listener !== "function") throw new TypeError("zoom listener must be a function");
    return this.camera.subscribe((snapshot) => listener(Math.round(snapshot.framingScale * 100)));
  }

  applySessionSnapshot(snapshot) {
    if (!snapshot?.draft) return;
    if (!["terrainStroke", "doodadStroke", "overlayStroke"].includes(snapshot.reason)) {
      this.rebuildTerrain();
    }
    if (snapshot.reason === "doodadStroke" && snapshot.doodadPatch) {
      this.queueDoodadPatch(snapshot.doodadPatch);
    } else {
      this.rebuildDoodads();
    }
    this.drawOverlay();
  }

  rebuildTerrain() {
    if (!this.session.draft) return;
    const materialized = this.session.materialized();
    MapEditorViewport.prototype.queueTerrainReplacement.call(this, materialized);
  }

  previewSunConditions(sun) {
    if (!this.session.draft?.sun) return false;
    const materialized = this.session.materialized();
    MapEditorViewport.prototype.queueTerrainReplacement.call(this, { ...materialized, sun: { ...sun } });
    return true;
  }

  previewSunDirection(azimuthDegrees) {
    this.sunDirectionPreviewDegrees = Math.max(0, Math.min(359, Math.trunc(Number(azimuthDegrees)) || 0));
    this.drawOverlay();
    return true;
  }

  clearSunDirectionPreview() {
    if (this.sunDirectionPreviewDegrees == null) return false;
    this.sunDirectionPreviewDegrees = null;
    this.drawOverlay();
    return true;
  }

  queueTerrainReplacement(materialized) {
    this.terrainRevision += 1;
    this.pendingTerrainUpdate = {
      kind: "replace",
      revision: this.terrainRevision,
      width: materialized.width,
      height: materialized.height,
      tileSize: TILE_SIZE,
      terrain: materialized.terrain,
      elevation: materialized.elevation,
      sun: materialized.sun,
    };
    const worldWidth = materialized.width * TILE_SIZE;
    const worldHeight = materialized.height * TILE_SIZE;
    const firstMap = this.camera.worldW <= 0;
    this.camera.setBounds(worldWidth, worldHeight, this.root.clientWidth, this.root.clientHeight);
    if (firstMap) {
      const fit = Math.min(this.root.clientWidth / worldWidth, this.root.clientHeight / worldHeight) * 0.92;
      this.camera.setZoom(fit);
      this.camera.centerOn(worldWidth / 2, worldHeight / 2);
    }
  }

  rebuildDoodads() {
    if (!this.session.draft) return;
    this.doodadRevision += 1;
    this.pendingDoodadUpdate = {
      kind: "replace",
      revision: this.doodadRevision,
      doodads: structuredCloneSafe(this.session.draft.doodads || []),
    };
  }

  resourcePatchRecords() {
    if (this.resourcePatchRevision !== this.terrainRevision) {
      this.resourcePatches = mapEditorResourcePatches(this.session.draft);
      this.resourcePatchRevision = this.terrainRevision;
    }
    return this.resourcePatches;
  }

  queueDoodadPatch({ upserts = [], removedIds = [] } = {}) {
    this.doodadRevision += 1;
    if (this.pendingDoodadUpdate?.kind === "replace") {
      this.pendingDoodadUpdate = {
        kind: "replace",
        revision: this.doodadRevision,
        doodads: structuredCloneSafe(this.session.draft?.doodads || []),
      };
      return;
    }
    const byId = new Map((this.pendingDoodadUpdate?.upserts || []).map((record) => [record.id, record]));
    const removed = new Set(this.pendingDoodadUpdate?.removedIds || []);
    for (const id of removedIds) {
      byId.delete(id);
      removed.add(id);
    }
    for (const record of upserts) {
      removed.delete(record.id);
      byId.set(record.id, structuredCloneSafe(record));
    }
    this.pendingDoodadUpdate = {
      kind: "patch",
      revision: this.doodadRevision,
      upserts: [...byId.values()].sort((left, right) => left.id - right.id),
      removedIds: [...removed].sort((left, right) => left - right),
    };
  }

  drawOverlay() {
    const draft = this.session.draft;
    if (!draft) return;
    const dimensions = { width: draft.width, height: draft.height };
    const worldWidth = draft.width * TILE_SIZE;
    const worldHeight = draft.height * TILE_SIZE;
    const gridPaths = [];
    for (let tile = 0; tile <= draft.width; tile += 8) {
      const x = tile * TILE_SIZE;
      gridPaths.push([[x, 0], [x, worldHeight]]);
    }
    for (let tile = 0; tile <= draft.height; tile += 8) {
      const y = tile * TILE_SIZE;
      gridPaths.push([[0, y], [worldWidth, y]]);
    }
    const guides = mapEditorSymmetryGuideLines(dimensions, this.symmetry).map((guide) => [
      [guide.x0, guide.y0], [guide.x1, guide.y1],
    ]);
    const guideCentre = mapEditorSymmetryGuideCentre(dimensions, this.symmetry);
    const locations = this.session.mapOverlay();
    const sites = [];
    for (const start of locations?.starts || []) sites.push(this.siteRecord(start, 0x4ec9ff, 11, `S${start.index + 1}`));
    for (const [index, base] of (locations?.bases || []).entries()) {
      sites.push(this.siteRecord(base, 0xf4c542, 7, `B${index + 1}`, base.index === this.selectedBaseIndex));
    }
    this.overlayRevision += 1;
    this.pendingOverlay = {
      revision: this.overlayRevision,
      gridPaths,
      guides,
      guideCentre,
      sites,
      resourcePatches: this.resourcePatchRecords(),
      concealmentTiles: structuredCloneSafe(draft.concealmentTiles || []),
      noVehicleTiles: structuredCloneSafe(draft.noVehicleTiles || []),
      noBuildingTiles: structuredCloneSafe(draft.noBuildingTiles || []),
      noEntrenchmentTiles: structuredCloneSafe(draft.noEntrenchmentTiles || []),
      damageReductionTiles: structuredCloneSafe(draft.damageReductionTiles || []),
      slowMovementTiles: structuredCloneSafe(draft.slowMovementTiles || []),
      forestTiles: structuredCloneSafe(this.session.forestTiles?.() || []),
      paintPreview: this.paintPreviewRecord(),
      doodadBrushPreview: this.doodadBrushPreviewRecord?.() || null,
      sunDirectionPreview: this.sunDirectionPreviewDegrees == null
        ? null
        : mapEditorSunDirectionPreview(dimensions, this.sunDirectionPreviewDegrees),
    };
  }

  doodadBrushPreviewRecord() {
    if (this.tool?.kind !== "doodad" || !this.doodadBrushPoint) return null;
    return {
      x: this.doodadBrushPoint.x,
      y: this.doodadBrushPoint.y,
      radius: this.tool.mode === "place" ? 12 : Math.max(4, Number(this.tool.radius) || 48),
      mode: this.tool.mode,
      typeId: this.tool.typeId || null,
      color: this.tool.color || null,
    };
  }

  paintPreviewRecord() {
    if (this.tool?.kind === "road" && this.paintStartTile && this.lastPaintTile) {
      const end = snapRoadEnd(this.paintStartTile, this.lastPaintTile, this.session.draft);
      const x0 = (this.paintStartTile.x + 0.5) * TILE_SIZE;
      const y0 = (this.paintStartTile.y + 0.5) * TILE_SIZE;
      const x1 = (end.x + 0.5) * TILE_SIZE;
      const y1 = (end.y + 0.5) * TILE_SIZE;
      return {
        paths: [[[x0, y0], [x1, y1]]],
        lineWidth: Math.max(TILE_SIZE, Number(this.tool.width) * TILE_SIZE),
        color: 0xe6bf42,
      };
    }
    if (this.tool?.kind === "forest" && this.lastPaintTile) {
      const size = Math.max(1, Number(this.tool.width) || 1) * TILE_SIZE;
      return {
        x: (this.lastPaintTile.x + 0.5) * TILE_SIZE - size / 2,
        y: (this.lastPaintTile.y + 0.5) * TILE_SIZE - size / 2,
        width: size,
        height: size,
        color: this.tool.paint ? 0x2f9f78 : 0xd94b45,
      };
    }
    if (!["terrain", "overlay"].includes(this.tool?.kind) || this.tool.shape !== "box" || !this.paintStartTile || !this.lastPaintTile) return null;
    const x0 = Math.min(this.paintStartTile.x, this.lastPaintTile.x) * TILE_SIZE;
    const y0 = Math.min(this.paintStartTile.y, this.lastPaintTile.y) * TILE_SIZE;
    const width = (Math.abs(this.lastPaintTile.x - this.paintStartTile.x) + 1) * TILE_SIZE;
    const height = (Math.abs(this.lastPaintTile.y - this.paintStartTile.y) + 1) * TILE_SIZE;
    const color = this.tool.kind === "overlay" ? overlayPreviewColor(this.tool.edit) : terrainPreviewColor(this.tool.terrain);
    return { x: x0, y: y0, width, height, color };
  }

  siteRecord(site, color, radius, label, selected = false) {
    const x = (site.x + 0.5) * TILE_SIZE;
    const y = (site.y + 0.5) * TILE_SIZE;
    return { x, y, color, radius, label, selected: !!selected };
  }

  handlePointerDown(event) {
    if (event.button === 1 || event.button === 2 || (event.button === 0 && event.altKey)) {
      this.panPointerId = event.pointerId;
      this.lastPointer = { x: event.clientX, y: event.clientY };
      event.currentTarget.setPointerCapture?.(event.pointerId);
      event.preventDefault();
      return;
    }
    if (event.button !== 0 || !this.tool) return;
    if (this.tool.kind === "doodad") {
      const world = this.eventWorld(event);
      if (!world) return;
      this.doodadBrushPoint = world;
      this.beginDoodadPointer(event, world);
      event.preventDefault();
      return;
    }
    const tile = this.eventTile(event, { kind: this.tool.kind });
    if (!tile) return;
    if (["terrain", "elevation", "overlay", "forest", "road"].includes(this.tool.kind)) {
      this.paintPointerId = event.pointerId;
      this.paintStartTile = tile;
      this.lastPaintTile = tile;
      const action = this.tool.shape === "box" ? "Filled" : "Painted";
      if (this.tool.kind === "elevation") {
        this.session.beginElevationStroke(`${action} elevation level ${this.tool.level}`);
      } else if (this.tool.kind === "terrain" || this.tool.kind === "road") {
        this.session.beginTerrainStroke(`${action} ${terrainLabel(this.tool.terrain)} terrain`);
      } else {
        this.session.beginOverlayStroke(this.tool.kind === "forest"
          ? `${this.tool.paint ? "Painted" : "Erased"} forest`
          : `${action} ${this.tool.label || "map overlay"}`);
      }
      if (this.tool.kind === "road" || this.tool.shape === "box") this.drawOverlay();
      else if (this.tool.kind === "forest") this.paintForest(tile, tile);
      else this.paintLine(tile, tile);
      event.currentTarget.setPointerCapture?.(event.pointerId);
    } else {
      this.applySiteTool(tile);
    }
    event.preventDefault();
  }

  handlePointerMove(event) {
    if (event.pointerId === this.panPointerId && this.lastPointer) {
      this.camera.panByScreenDelta(event.clientX - this.lastPointer.x, event.clientY - this.lastPointer.y);
      this.lastPointer = { x: event.clientX, y: event.clientY };
      return;
    }
    if (this.tool?.kind === "doodad") {
      const world = this.eventWorld(event);
      if (world) {
        this.doodadBrushPoint = world;
        if (event.pointerId === this.doodadPointerId) this.continueDoodadPointer(world);
        this.drawOverlay();
      }
      if (event.pointerId === this.doodadPointerId) return;
    }
    if (event.pointerId !== this.paintPointerId || !["terrain", "elevation", "overlay", "forest", "road"].includes(this.tool?.kind)) return;
    const tile = this.eventTile(event);
    if (!tile || !this.lastPaintTile) return;
    if (this.tool.kind === "forest") this.paintForest(this.lastPaintTile, tile);
    else if (this.tool.kind !== "road" && this.tool.shape !== "box") this.paintLine(this.lastPaintTile, tile);
    this.lastPaintTile = tile;
    if (this.tool.kind === "road" || this.tool.shape === "box") this.drawOverlay();
  }

  handlePointerUp(event) {
    if (event.pointerId === this.panPointerId) {
      this.panPointerId = null;
      this.lastPointer = null;
    }
    if (event.pointerId === this.doodadPointerId) {
      const cancelled = event.type === "pointercancel";
      if (!cancelled) {
        const world = this.eventWorld(event);
        if (world) this.continueDoodadPointer(world);
      }
      const mode = this.doodadPointerMode;
      this.doodadPointerId = null;
      this.doodadPointerMode = null;
      this.doodadSprayStroke = null;
      this.doodadLastWorld = null;
      const changed = cancelled ? (this.session.cancelDoodadStroke(), false) : this.session.commitDoodadStroke();
      this.drawOverlay();
      this.onStatus(
        cancelled ? "Doodad edit cancelled." : changed ? doodadCommitLabel(mode) : "No doodads changed.",
        !cancelled && !changed,
      );
    }
    if (event.pointerId === this.paintPointerId) {
      const cancelled = event.type === "pointercancel";
      if (!cancelled) {
        const tile = this.eventTile(event);
        if (tile) this.lastPaintTile = tile;
        if (this.tool?.kind === "road" && this.paintStartTile && this.lastPaintTile) {
          this.paintRoad(this.paintStartTile, this.lastPaintTile);
        } else if (["terrain", "elevation", "overlay"].includes(this.tool?.kind) && this.tool.shape === "box" && this.paintStartTile && this.lastPaintTile) {
          this.paintBox(this.paintStartTile, this.lastPaintTile);
        }
      }
      this.paintPointerId = null;
      this.lastPaintTile = null;
      this.paintStartTile = null;
      let changed = false;
      if (this.tool?.kind === "elevation") {
        if (cancelled) this.session.cancelElevationStroke();
        else changed = this.session.commitElevationStroke();
      } else if (["overlay", "forest"].includes(this.tool?.kind)) {
        if (cancelled) this.session.cancelOverlayStroke();
        else changed = this.session.commitOverlayStroke();
      } else if (cancelled) this.session.cancelTerrainStroke();
      else changed = this.session.commitTerrainStroke();
      this.drawOverlay();
      this.onStatus(
        cancelled
          ? (["overlay", "forest"].includes(this.tool?.kind) ? "Overlay paint cancelled." : this.tool?.kind === "elevation" ? "Elevation paint cancelled." : "Terrain paint cancelled.")
          : changed ? "Map paint committed." : "No map tiles changed.",
        !cancelled && !changed,
      );
    }
    event.currentTarget.releasePointerCapture?.(event.pointerId);
  }

  beginDoodadPointer(event, world) {
    const tool = this.tool;
    this.session.beginDoodadStroke(tool.mode === "erase" ? "Erased doodads" : tool.mode === "spray" ? "Sprayed doodads" : "Placed doodads");
    this.doodadPointerMode = tool.mode;
    if (tool.mode === "spray") {
      const result = createDoodadSprayStroke(world, {
        radius: tool.radius,
        density: tool.density,
        seed: allocateMapEditorDoodadId(this.session.draft?.doodads || []),
      });
      this.doodadSprayStroke = result?.stroke || null;
      this.placeDoodadPoints(result?.placements || []);
    } else if (tool.mode === "erase") {
      this.eraseDoodadsAt(world);
    } else {
      this.placeDoodadPoints([world]);
    }
    this.doodadPointerId = event.pointerId;
    this.doodadLastWorld = world;
    event.currentTarget.setPointerCapture?.(event.pointerId);
  }

  continueDoodadPointer(world) {
    if (this.doodadPointerMode === "spray" && this.doodadSprayStroke) {
      this.placeDoodadPoints(extendDoodadSprayStroke(this.doodadSprayStroke, world));
    } else if (this.doodadPointerMode === "erase") {
      const spacing = Math.max(4, (Number(this.tool?.radius) || 48) / 2);
      for (const point of resampleWorldSegment(this.doodadLastWorld, world, spacing)) this.eraseDoodadsAt(point);
    }
    this.doodadLastWorld = world;
  }

  placeDoodadPoints(points) {
    const dimensions = {
      width: (this.session.draft?.width || 0) * TILE_SIZE,
      height: (this.session.draft?.height || 0) * TILE_SIZE,
    };
    const typeIds = this.tool?.typeIds?.length ? this.tool.typeIds : [this.tool?.typeId];
    const existing = this.session.draft?.doodads || [];
    const typeSeed = allocateMapEditorDoodadId(existing);
    const available = Math.max(0, MAP_EDITOR_MAX_DOODADS - existing.length);
    const planned = [];
    const plannedKeys = new Set();
    for (const point of points || []) {
      const group = symmetricDoodadPlacements(dimensions, [point], this.tool?.symmetry)
        .filter((placement) => !plannedKeys.has(`${placement.x},${placement.y}`));
      if (planned.length + group.length > available) break;
      const typeId = doodadTypeFromSelection(typeIds, typeSeed + planned.length);
      planned.push(...group.map((placement) => ({
        ...placement,
        typeId,
        color: this.tool?.color,
      })));
      for (const placement of group) plannedKeys.add(`${placement.x},${placement.y}`);
    }
    const added = this.session.placeDoodadRecords(planned);
    if (added.length) this.queueDoodadPatch({ upserts: added });
  }

  eraseDoodadsAt(world) {
    const dimensions = {
      width: (this.session.draft?.width || 0) * TILE_SIZE,
      height: (this.session.draft?.height || 0) * TILE_SIZE,
    };
    const radius = Math.max(4, Number(this.tool?.radius) || 48);
    const ids = new Set();
    for (const point of symmetricDoodadPlacements(dimensions, [world], this.tool?.symmetry)) {
      for (const id of doodadIdsWithinRadius(this.session.draft?.doodads || [], point, radius)) ids.add(id);
    }
    const removed = this.session.removeDoodads(ids);
    if (removed.length) this.queueDoodadPatch({ removedIds: removed });
  }

  paintLine(from, to) {
    if (this.tool?.kind === "terrain" && Number(this.tool.width) > 1) {
      const { tiles } = pathTiles(this.session.draft, {
        points: [[from.x, from.y], [to.x, to.y]],
        width: this.tool.width,
        roughness: 0,
      });
      this.paintTiles(tiles);
      return;
    }
    this.paintTiles(lineTiles(from, to));
  }

  paintForest(from, to) {
    const { tiles } = pathTiles(this.session.draft, {
      points: [[from.x, from.y], [to.x, to.y]],
      width: Math.max(1, Number(this.tool?.width) || 1),
      roughness: 0,
    });
    const symmetric = symmetricMapTiles(this.session.draft, tiles, this.tool?.symmetry);
    if (this.session.paintForestTiles(symmetric, this.tool?.paint !== false).length) this.drawOverlay();
  }

  paintBox(from, to) {
    const dimensions = this.session.draft;
    this.paintTiles(mapEditorRectTiles(from, to, dimensions));
  }

  paintRoad(from, to) {
    const end = snapRoadEnd(from, to, this.session.draft);
    const changes = this.session.paintRoad(from, end, this.tool?.width, this.tool?.symmetry);
    if (changes.length) this.queueTerrainChanges(changes);
  }

  paintTiles(tiles) {
    const dimensions = this.session.draft;
    if (this.tool?.kind === "elevation") {
      const symmetric = symmetricMapTiles(dimensions, tiles, this.tool.symmetry);
      if (this.session.paintElevationTiles(symmetric, this.tool.level).length) this.rebuildTerrain();
      return;
    }
    if (this.tool?.kind === "forest") {
      const symmetric = symmetricMapTiles(dimensions, tiles, this.tool?.symmetry);
      if (this.session.paintForestTiles(symmetric, this.tool?.paint !== false).length) this.drawOverlay();
      return;
    }
    if (this.tool?.kind === "overlay") {
      const symmetric = symmetricMapTiles(dimensions, tiles, this.tool?.symmetry);
      if (this.session.paintOverlayTiles(symmetric, this.tool.edit).length) this.drawOverlay();
      return;
    }
    const changes = this.session.paintTerrainTiles(
      symmetricTerrainTiles(dimensions, tiles, this.tool.terrain, this.tool?.symmetry),
      this.tool.terrain,
    );
    if (changes.length > 0) this.queueTerrainChanges(changes);
  }

  queueTerrainChanges(changes) {
    this.terrainRevision += 1;
    if (this.pendingTerrainUpdate?.kind === "replace") {
      const materialized = this.session.materialized();
      this.pendingTerrainUpdate = {
        ...this.pendingTerrainUpdate,
        revision: this.terrainRevision,
        terrain: materialized.terrain,
      };
    } else {
      const priorChanges = this.pendingTerrainUpdate?.kind === "patch"
        ? this.pendingTerrainUpdate.changes
        : [];
      this.pendingTerrainUpdate = {
        kind: "patch",
        revision: this.terrainRevision,
        changes: coalesceTerrainChanges(priorChanges, changes),
      };
    }
  }

  applySiteTool(tile) {
    const tool = this.tool;
    let result = null;
    const label = tool.add
      ? `Added ${tool.kind === "start" ? "start location" : "base site"}`
      : `Moved ${tool.kind === "start" ? "start location" : "base site"}`;
    const changed = this.session.mutate(label, (draft) => {
      result = tool.add
        ? addSymmetricDraftLocations(draft, { kind: tool.kind, tile, symmetry: tool.symmetry })
        : moveSymmetricDraftLocation(draft, {
          kind: tool.kind,
          locationIndex: tool.locationIndex,
          tile,
          symmetry: tool.symmetry,
        });
      if (result?.ok) protectDraftBaseTerrain(draft);
    });
    if (changed && tool.kind === "base" && !tool.add) {
      const selectedBase = this.session.mapOverlay()?.bases.find((base) => base.x === tile.x && base.y === tile.y);
      this.setSelectedBase(selectedBase?.index ?? null);
    }
    const extra = Math.max(0, Number(result?.count || 1) - 1);
    this.onStatus(
      changed ? `${label}${extra ? ` and ${extra} symmetric location${extra === 1 ? "" : "s"}` : ""}.` : result?.error || "No map change.",
      !changed,
    );
  }

  eventTile(event, { kind = this.tool?.kind } = {}) {
    const rect = this.presentation.canvas.getBoundingClientRect();
    const world = this.camera.screenToWorld(event.clientX - rect.left, event.clientY - rect.top);
    const width = this.session.draft?.width || 0;
    const height = this.session.draft?.height || 0;
    const radius = kind === "start" ? MAP_EDITOR_MAIN_CLEARANCE_TILES : kind === "base" ? MAP_EDITOR_BASE_SITE_CLEARANCE_TILES : 0;
    if (!width || !height || width <= radius * 2 || height <= radius * 2) return null;
    return {
      x: Math.max(radius, Math.min(width - radius - 1, Math.floor(world.x / TILE_SIZE))),
      y: Math.max(radius, Math.min(height - radius - 1, Math.floor(world.y / TILE_SIZE))),
    };
  }

  eventWorld(event, { clamp = false } = {}) {
    const rect = this.presentation.canvas.getBoundingClientRect();
    const world = this.camera.screenToWorld(event.clientX - rect.left, event.clientY - rect.top);
    const worldWidth = (this.session.draft?.width || 0) * TILE_SIZE;
    const worldHeight = (this.session.draft?.height || 0) * TILE_SIZE;
    if (!worldWidth || !worldHeight) return null;
    if (!clamp && (world.x < 0 || world.y < 0 || world.x >= worldWidth || world.y >= worldHeight)) return null;
    return {
      x: Math.max(0, Math.min(worldWidth - 1, Math.round(world.x))),
      y: Math.max(0, Math.min(worldHeight - 1, Math.round(world.y))),
    };
  }

  handleWheel(event) {
    const rect = this.presentation.canvas.getBoundingClientRect();
    const factor = event.deltaY > 0 ? 0.88 : 1.14;
    this.camera.setZoom(
      this.camera.zoom * factor,
      event.clientX - rect.left,
      event.clientY - rect.top,
    );
    event.preventDefault();
  }

  handleKey(event, pressed) {
    if (isTextEntry(event.target)) return;
    const direction = event.code === "ArrowUp" || event.code === "KeyW" ? "up"
      : event.code === "ArrowDown" || event.code === "KeyS" ? "down"
        : event.code === "ArrowLeft" || event.code === "KeyA" ? "left"
          : event.code === "ArrowRight" || event.code === "KeyD" ? "right" : "";
    if (!direction) return;
    this.keys[direction] = pressed;
    event.preventDefault();
  }

  tick(at) {
    if (this.destroyed || this.presentationStopped) return;
    const dt = Math.min(0.1, Math.max(0, (at - this.lastFrameAt) / 1000));
    this.lastFrameAt = at;
    this.visualTimeMs = Math.max(0, Number(at) || 0);
    this.camera.update(dt, { keys: this.keys, mouse: null });
    this.submitPresentation();
    this.frame = requestAnimationFrame((next) => this.tick(next));
  }

  submitPresentation() {
    if (this.destroyed || this.presentationStopped || this.presentationInFlight) return;
    this.presentationFrameId += 1;
    const terrainRevision = this.pendingTerrainUpdate?.revision ?? 0;
    const overlayRevision = this.pendingOverlay?.revision ?? 0;
    const doodadRevision = this.pendingDoodadUpdate?.revision ?? 0;
    const token = Object.freeze({
      frameId: this.presentationFrameId,
      terrainRevision,
      overlayRevision,
      doodadRevision,
    });
    this.presentationInFlight = token;
    let settlement;
    try {
      settlement = this.presentation.present(createMapEditorPresentation({
        frameId: token.frameId,
        camera: { x: this.camera.x, y: this.camera.y, zoom: this.camera.zoom },
        terrainUpdate: this.pendingTerrainUpdate,
        doodadUpdate: this.pendingDoodadUpdate,
        overlay: this.pendingOverlay,
        layerVisibility: this.layerVisibility,
        visualTimeMs: this.visualTimeMs,
      }));
    } catch (error) {
      this.settlePresentation(token, { status: PRESENTATION_OUTCOME.FAILED, error });
      return;
    }
    Promise.resolve(settlement).then(
      (outcome) => this.settlePresentation(token, outcome),
      (error) => this.settlePresentation(token, { status: PRESENTATION_OUTCOME.FAILED, error }),
    );
  }

  settlePresentation(token, outcome) {
    if (this.presentationInFlight !== token) return;
    this.presentationInFlight = null;
    if (this.destroyed) return;
    if (outcome?.status === PRESENTATION_OUTCOME.PRESENTED) {
      if (token.terrainRevision > 0 && this.pendingTerrainUpdate?.revision === token.terrainRevision) {
        this.pendingTerrainUpdate = null;
      }
      if (token.overlayRevision > 0 && this.pendingOverlay?.revision === token.overlayRevision) {
        this.pendingOverlay = null;
      }
      if (token.doodadRevision > 0 && this.pendingDoodadUpdate?.revision === token.doodadRevision) {
        this.pendingDoodadUpdate = null;
      }
      return;
    }
    if (outcome?.status === PRESENTATION_OUTCOME.SUPERSEDED) {
      this.stopPresentation("Map renderer discarded a serialized editor frame.");
      return;
    }
    if (outcome?.status === PRESENTATION_OUTCOME.FAILED) {
      this.stopPresentation(`Map renderer stopped: ${presentationOutcomeMessage(outcome)}`);
      return;
    }
    if (outcome?.status === PRESENTATION_OUTCOME.DESTROYED) {
      this.stopPresentation("Map renderer stopped unexpectedly.");
      return;
    }
    this.stopPresentation("Map renderer returned an invalid presentation result.");
  }

  stopPresentation(message) {
    if (this.presentationStopped || this.destroyed) return;
    this.presentationStopped = true;
    if (this.frame !== undefined) cancelAnimationFrame(this.frame);
    this.onStatus(message, true);
  }

  resize() {
    const width = this.root.clientWidth || window.innerWidth;
    const height = this.root.clientHeight || window.innerHeight;
    this.presentation.resize(width, height);
    this.camera.setBounds(this.camera.worldW, this.camera.worldH, width, height);
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    this.presentationStopped = true;
    cancelAnimationFrame(this.frame);
    this.unsubscribe?.();
    const canvas = this.presentation.canvas;
    canvas.removeEventListener("pointerdown", this.onPointerDown);
    canvas.removeEventListener("pointermove", this.onPointerMove);
    canvas.removeEventListener("pointerup", this.onPointerUp);
    canvas.removeEventListener("pointercancel", this.onPointerUp);
    canvas.removeEventListener("wheel", this.onWheel);
    canvas.removeEventListener("contextmenu", this.onContextMenu);
    window.removeEventListener("keydown", this.onKeyDown);
    window.removeEventListener("keyup", this.onKeyUp);
    window.removeEventListener("resize", this.onResize);
    this.presentation.destroy();
  }
}

function coalesceTerrainChanges(previous, changes) {
  const byTile = new Map();
  for (const change of previous.concat(changes)) {
    if (!Number.isInteger(change?.x) || !Number.isInteger(change?.y)) continue;
    byTile.set(`${change.x},${change.y}`, change);
  }
  return [...byTile.values()];
}

function snapRoadEnd(from, to, dimensions) {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  if (dx === 0 && dy === 0) return { ...from };
  const octant = Math.round(Math.atan2(dy, dx) / (Math.PI / 4));
  const stepX = Math.round(Math.cos(octant * Math.PI / 4));
  const stepY = Math.round(Math.sin(octant * Math.PI / 4));
  const projected = stepX && stepY
    ? Math.max(1, Math.round((Math.abs(dx) + Math.abs(dy)) / 2))
    : Math.max(1, Math.round(Math.abs(dx * stepX + dy * stepY)));
  const width = Math.max(1, Math.trunc(Number(dimensions?.width)) || 1);
  const height = Math.max(1, Math.trunc(Number(dimensions?.height)) || 1);
  const maxX = stepX > 0 ? width - 1 - from.x : stepX < 0 ? from.x : Infinity;
  const maxY = stepY > 0 ? height - 1 - from.y : stepY < 0 ? from.y : Infinity;
  const distance = Math.min(projected, maxX, maxY);
  return { x: from.x + stepX * distance, y: from.y + stepY * distance };
}

function presentationOutcomeMessage(outcome) {
  return outcome?.error?.message || outcome?.error?.name || "unknown worker failure";
}

function mapDimensions(value) {
  if (typeof value === "number") {
    const size = Math.max(0, Math.trunc(Number(value)) || 0);
    return { width: size, height: size };
  }
  return {
    width: Math.max(0, Math.trunc(Number(value?.width)) || 0),
    height: Math.max(0, Math.trunc(Number(value?.height)) || 0),
  };
}

function compassPoint(degrees) {
  return ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][Math.round(degrees / 45) % 8];
}

function terrainLabel(code) {
  if (code === TERRAIN.GRAVEL_A) return "slate gravel";
  if (code === TERRAIN.GRAVEL_B) return "limestone gravel";
  if (code === TERRAIN.GRAVEL_C) return "chalk gravel";
  if (code === TERRAIN.DIRT_A) return "loam dirt";
  if (code === TERRAIN.DIRT_B) return "red clay dirt";
  if (code === TERRAIN.DIRT_C) return "dry ochre dirt";
  if (code === TERRAIN.MUD_A) return "churned mud";
  if (code === TERRAIN.MUD_B) return "waterlogged mud";
  if (code === TERRAIN.MUD_C) return "clay mud";
  if (code === TERRAIN.FROSTED_GROUND) return "frosted ground";
  if (code === TERRAIN.ROCK) return "stone";
  if (code === TERRAIN.WATER) return "water";
  return "grass";
}

function terrainPreviewColor(code) {
  if (code === TERRAIN.GRAVEL_A) return 0x6d6f65;
  if (code === TERRAIN.GRAVEL_B) return 0x847864;
  if (code === TERRAIN.GRAVEL_C) return 0x928f7f;
  if (code === TERRAIN.DIRT_A) return 0x785a42;
  if (code === TERRAIN.DIRT_B) return 0x825742;
  if (code === TERRAIN.DIRT_C) return 0x897550;
  if (code === TERRAIN.MUD_A) return 0x513e31;
  if (code === TERRAIN.MUD_B) return 0x4c493a;
  if (code === TERRAIN.MUD_C) return 0x584038;
  if (code === TERRAIN.FROSTED_GROUND) return 0x6a7164;
  if (code === TERRAIN.ROCK) return 0xa69a82;
  if (code === TERRAIN.WATER) return 0x4b9bd0;
  return 0x6d9f58;
}

function overlayPreviewColor(edit) {
  if (edit?.concealment != null) return 0x5ed19a;
  if (edit?.noVehicle != null) return 0xf26a5a;
  if (edit?.noBuilding != null) return 0xe9a23b;
  if (edit?.damageReduction != null) return 0x6da8ff;
  return 0xb276e8;
}

function doodadCommitLabel(mode) {
  if (mode === "erase") return "Doodads erased.";
  if (mode === "spray") return "Doodads sprayed.";
  return "Doodads placed.";
}

function resampleWorldSegment(from, to, spacing) {
  if (!from || !to) return to ? [to] : [];
  const distance = Math.hypot(to.x - from.x, to.y - from.y);
  if (!distance) return [to];
  const count = Math.max(1, Math.ceil(distance / Math.max(1, spacing)));
  return Array.from({ length: count }, (_, index) => {
    const ratio = (index + 1) / count;
    return {
      x: Math.round(from.x + (to.x - from.x) * ratio),
      y: Math.round(from.y + (to.y - from.y) * ratio),
    };
  });
}

function isTextEntry(target) {
  return ["INPUT", "TEXTAREA", "SELECT"].includes(String(target?.tagName || "")) || !!target?.isContentEditable;
}

function animationFrames(count) {
  return new Promise((resolve) => {
    const next = () => count-- <= 0 ? resolve() : requestAnimationFrame(next);
    next();
  });
}

function structuredCloneSafe(value) {
  return typeof structuredClone === "function" ? structuredClone(value) : JSON.parse(JSON.stringify(value));
}
