import { Camera } from "./camera.js";
import { createMapEditorPresentation } from "./map_editor_presentation.js";
import { createMapEditorTerrainPreview } from "./map_editor_terrain_preview.js";
import { TERRAIN } from "./protocol.js";
import { MapEditorPixiPresentationAdapter } from "./renderer/map_editor_presentation_adapter.js";
import {
  addSymmetricDraftLocations,
  mapEditorRectTiles,
  MAP_EDITOR_BASE_SITE_CLEARANCE_TILES,
  MAP_EDITOR_MAIN_CLEARANCE_TILES,
  MAP_EDITOR_SYMMETRY,
  moveSymmetricDraftLocation,
  protectDraftBaseTerrain,
  symmetricTerrainTiles,
} from "./map_editor_session.js";

const TILE_SIZE = 32;

export function mapEditorSymmetryGuideLines(size, symmetry) {
  const mapSize = Math.max(0, Math.trunc(Number(size)) || 0);
  const worldSize = mapSize * TILE_SIZE;
  const centre = worldSize / 2;
  const horizontal = { x0: 0, y0: centre, x1: worldSize, y1: centre };
  const vertical = { x0: centre, y0: 0, x1: centre, y1: worldSize };
  if (symmetry === MAP_EDITOR_SYMMETRY.HORIZONTAL) return [horizontal];
  if (symmetry === MAP_EDITOR_SYMMETRY.VERTICAL) return [vertical];
  if (symmetry === MAP_EDITOR_SYMMETRY.THREE_WAY) {
    return [-Math.PI / 2, Math.PI / 6, 5 * Math.PI / 6]
      .map((angle) => symmetryGuideRay(centre, worldSize, angle));
  }
  if (symmetry === MAP_EDITOR_SYMMETRY.RADIAL) return [horizontal, vertical];
  if (symmetry === MAP_EDITOR_SYMMETRY.DIAGONAL_MAIN) {
    return [{ x0: 0, y0: 0, x1: worldSize, y1: worldSize }];
  }
  if (symmetry === MAP_EDITOR_SYMMETRY.DIAGONAL_ANTI) {
    return [{ x0: 0, y0: worldSize, x1: worldSize, y1: 0 }];
  }
  return [];
}

function symmetryGuideRay(centre, worldSize, angle) {
  const dx = Math.cos(angle);
  const dy = Math.sin(angle);
  const distances = [];
  if (dx > 0) distances.push((worldSize - centre) / dx);
  if (dx < 0) distances.push(-centre / dx);
  if (dy > 0) distances.push((worldSize - centre) / dy);
  if (dy < 0) distances.push(-centre / dy);
  const distance = Math.min(...distances.filter((candidate) => candidate >= 0));
  return {
    x0: centre,
    y0: centre,
    x1: centre + dx * distance,
    y1: centre + dy * distance,
  };
}

export function mapEditorSymmetryGuideCentre(size, symmetry) {
  if (symmetry !== MAP_EDITOR_SYMMETRY.HALF_TURN) return null;
  const mapSize = Math.max(0, Math.trunc(Number(size)) || 0);
  const centre = mapSize * TILE_SIZE / 2;
  return { x: centre, y: centre };
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
    this.paintPointerId = null;
    this.panPointerId = null;
    this.lastPointer = null;
    this.lastPaintTile = null;
    this.paintStartTile = null;
    this.keys = { up: false, down: false, left: false, right: false };
    this.destroyed = false;
    this.presentationFrameId = 0;
    this.terrainRevision = 0;
    this.overlayRevision = 0;
    this.pendingTerrainUpdate = null;
    this.pendingOverlay = null;

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
    if (this.tool?.symmetry) this.symmetry = this.tool.symmetry;
    this.drawOverlay();
    return this.tool;
  }

  setSymmetry(symmetry) {
    this.symmetry = Object.values(MAP_EDITOR_SYMMETRY).includes(symmetry)
      ? symmetry
      : MAP_EDITOR_SYMMETRY.NONE;
    this.drawOverlay();
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

  applySessionSnapshot(snapshot) {
    if (!snapshot?.draft) return;
    if (snapshot.reason !== "terrainStroke") this.rebuildTerrain();
    this.drawOverlay();
  }

  rebuildTerrain() {
    if (!this.session.draft) return;
    const materialized = this.session.materialized();
    this.terrainRevision += 1;
    this.pendingTerrainUpdate = {
      kind: "replace",
      revision: this.terrainRevision,
      width: materialized.size,
      height: materialized.size,
      tileSize: TILE_SIZE,
      terrain: materialized.terrain,
    };
    const worldSize = materialized.size * TILE_SIZE;
    const firstMap = this.camera.worldW <= 0;
    this.camera.setBounds(worldSize, worldSize, this.root.clientWidth, this.root.clientHeight);
    if (firstMap) {
      const fit = Math.min(this.root.clientWidth / worldSize, this.root.clientHeight / worldSize) * 0.92;
      this.camera.setZoom(fit);
      this.camera.centerOn(worldSize / 2, worldSize / 2);
    }
  }

  drawOverlay() {
    const draft = this.session.draft;
    if (!draft) return;
    const size = draft.terrain.length;
    const gridPaths = [];
    for (let tile = 0; tile <= size; tile += 8) {
      const p = tile * TILE_SIZE;
      gridPaths.push([[p, 0], [p, size * TILE_SIZE]], [[0, p], [size * TILE_SIZE, p]]);
    }
    const guides = mapEditorSymmetryGuideLines(size, this.symmetry).map((guide) => [
      [guide.x0, guide.y0], [guide.x1, guide.y1],
    ]);
    const guideCentre = mapEditorSymmetryGuideCentre(size, this.symmetry);
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
      paintPreview: this.paintPreviewRecord(),
    };
  }

  paintPreviewRecord() {
    if (this.tool?.kind !== "terrain" || this.tool.shape !== "box" || !this.paintStartTile || !this.lastPaintTile) return null;
    const x0 = Math.min(this.paintStartTile.x, this.lastPaintTile.x) * TILE_SIZE;
    const y0 = Math.min(this.paintStartTile.y, this.lastPaintTile.y) * TILE_SIZE;
    const width = (Math.abs(this.lastPaintTile.x - this.paintStartTile.x) + 1) * TILE_SIZE;
    const height = (Math.abs(this.lastPaintTile.y - this.paintStartTile.y) + 1) * TILE_SIZE;
    return { x: x0, y: y0, width, height, color: terrainPreviewColor(this.tool.terrain) };
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
    const tile = this.eventTile(event, { kind: this.tool.kind });
    if (!tile) return;
    if (this.tool.kind === "terrain") {
      this.paintPointerId = event.pointerId;
      this.paintStartTile = tile;
      this.lastPaintTile = tile;
      const action = this.tool.shape === "box" ? "Filled" : "Painted";
      this.session.beginTerrainStroke(`${action} ${terrainLabel(this.tool.terrain)} terrain`);
      if (this.tool.shape === "box") this.drawOverlay();
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
    if (event.pointerId !== this.paintPointerId || this.tool?.kind !== "terrain") return;
    const tile = this.eventTile(event);
    if (!tile || !this.lastPaintTile) return;
    if (this.tool.shape !== "box") this.paintLine(this.lastPaintTile, tile);
    this.lastPaintTile = tile;
    if (this.tool.shape === "box") this.drawOverlay();
  }

  handlePointerUp(event) {
    if (event.pointerId === this.panPointerId) {
      this.panPointerId = null;
      this.lastPointer = null;
    }
    if (event.pointerId === this.paintPointerId) {
      const cancelled = event.type === "pointercancel";
      if (!cancelled) {
        const tile = this.eventTile(event);
        if (tile) this.lastPaintTile = tile;
        if (this.tool?.kind === "terrain" && this.tool.shape === "box" && this.paintStartTile && this.lastPaintTile) {
          this.paintBox(this.paintStartTile, this.lastPaintTile);
        }
      }
      this.paintPointerId = null;
      this.lastPaintTile = null;
      this.paintStartTile = null;
      let changed = false;
      if (cancelled) this.session.cancelTerrainStroke();
      else changed = this.session.commitTerrainStroke();
      this.drawOverlay();
      this.onStatus(
        cancelled ? "Terrain paint cancelled." : changed ? "Terrain paint committed." : "Protected bases remain grass.",
        !cancelled && !changed,
      );
    }
    event.currentTarget.releasePointerCapture?.(event.pointerId);
  }

  paintLine(from, to) {
    this.paintTiles(lineTiles(from, to));
  }

  paintBox(from, to) {
    const size = this.session.draft?.terrain?.length || 0;
    this.paintTiles(mapEditorRectTiles(from, to, size));
  }

  paintTiles(tiles) {
    const size = this.session.draft?.terrain?.length || 0;
    const changes = this.session.paintTerrainTiles(
      symmetricTerrainTiles(size, tiles, this.tool.terrain, this.tool?.symmetry),
      this.tool.terrain,
    );
    if (changes.length > 0) {
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
          changes: priorChanges.concat(changes),
        };
      }
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
    const removed = Math.max(0, Number(result?.removed || 0));
    this.onStatus(
      changed ? `${label}${extra ? ` and ${extra} symmetric location${extra === 1 ? "" : "s"}` : ""}${removed ? ` and removed ${removed} corresponding base${removed === 1 ? "" : "s"}` : ""}.` : result?.error || "No map change.",
      !changed,
    );
  }

  eventTile(event, { kind = this.tool?.kind } = {}) {
    const rect = this.presentation.canvas.getBoundingClientRect();
    const world = this.camera.screenToWorld(event.clientX - rect.left, event.clientY - rect.top);
    const size = this.session.draft?.terrain?.length || 0;
    const radius = kind === "start" ? MAP_EDITOR_MAIN_CLEARANCE_TILES : kind === "base" ? MAP_EDITOR_BASE_SITE_CLEARANCE_TILES : 0;
    if (!size || size <= radius * 2) return null;
    return {
      x: Math.max(radius, Math.min(size - radius - 1, Math.floor(world.x / TILE_SIZE))),
      y: Math.max(radius, Math.min(size - radius - 1, Math.floor(world.y / TILE_SIZE))),
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
    if (this.destroyed) return;
    const dt = Math.min(0.1, Math.max(0, (at - this.lastFrameAt) / 1000));
    this.lastFrameAt = at;
    this.camera.update(dt, { keys: this.keys, mouse: null });
    this.presentationFrameId += 1;
    this.presentation.present(createMapEditorPresentation({
      frameId: this.presentationFrameId,
      camera: { x: this.camera.x, y: this.camera.y, zoom: this.camera.zoom },
      terrainUpdate: this.pendingTerrainUpdate,
      overlay: this.pendingOverlay,
    }));
    this.pendingTerrainUpdate = null;
    this.pendingOverlay = null;
    this.frame = requestAnimationFrame((next) => this.tick(next));
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

function lineTiles(from, to) {
  const out = [];
  let x = from.x;
  let y = from.y;
  const dx = Math.abs(to.x - x);
  const sx = x < to.x ? 1 : -1;
  const dy = -Math.abs(to.y - y);
  const sy = y < to.y ? 1 : -1;
  let error = dx + dy;
  while (true) {
    out.push({ x, y });
    if (x === to.x && y === to.y) break;
    const twice = error * 2;
    if (twice >= dy) { error += dy; x += sx; }
    if (twice <= dx) { error += dx; y += sy; }
  }
  return out;
}

function terrainLabel(code) {
  if (code === TERRAIN.ROCK) return "stone";
  if (code === TERRAIN.WATER) return "water";
  return "grass";
}

function terrainPreviewColor(code) {
  if (code === TERRAIN.ROCK) return 0xa69a82;
  if (code === TERRAIN.WATER) return 0x4b9bd0;
  return 0x6d9f58;
}

function isTextEntry(target) {
  return ["INPUT", "TEXTAREA", "SELECT"].includes(String(target?.tagName || "")) || !!target?.isContentEditable;
}

function structuredCloneSafe(value) {
  return typeof structuredClone === "function" ? structuredClone(value) : JSON.parse(JSON.stringify(value));
}
