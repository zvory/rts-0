import { Camera } from "./camera.js";
import { PLAYER_PALETTE } from "./config.js";
import { TERRAIN } from "./protocol.js";
import { Renderer } from "./renderer/index.js";
import { createTerrainPreviewCanvas } from "./renderer/terrain.js";
import {
  addSymmetricDraftNaturals,
  mapEditorRectTiles,
  MAP_EDITOR_MAIN_CLEARANCE_TILES,
  MAP_EDITOR_NATURAL_CLEARANCE_TILES,
  moveSymmetricDraftBase,
  protectDraftBaseTerrain,
  symmetricMapTiles,
} from "./map_editor_session.js";

const TILE_SIZE = 32;

export class MapEditorViewport {
  constructor({ root, session, onStatus = () => {} }) {
    this.root = root;
    this.session = session;
    this.onStatus = onStatus;
    this.renderer = new Renderer(root);
    this.camera = new Camera(root.clientWidth, root.clientHeight, {
      minZoom: 0.05,
      maxZoom: 4,
    });
    this.tool = null;
    this.paintPointerId = null;
    this.panPointerId = null;
    this.lastPointer = null;
    this.lastPaintTile = null;
    this.paintStartTile = null;
    this.keys = { up: false, down: false, left: false, right: false };
    this.destroyed = false;
    this.overlay = new PIXI.Graphics();
    this.renderer.layers.feedback.addChild(this.overlay);
    this.labels = [];

    this.onPointerDown = (event) => this.handlePointerDown(event);
    this.onPointerMove = (event) => this.handlePointerMove(event);
    this.onPointerUp = (event) => this.handlePointerUp(event);
    this.onWheel = (event) => this.handleWheel(event);
    this.onContextMenu = (event) => event.preventDefault();
    this.onKeyDown = (event) => this.handleKey(event, true);
    this.onKeyUp = (event) => this.handleKey(event, false);
    this.onResize = () => this.resize();
    const canvas = this.renderer.app.view;
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
    this.drawOverlay();
    return this.tool;
  }

  createTerrainPreview(terrain) {
    return createTerrainPreviewCanvas(terrain);
  }

  applySessionSnapshot(snapshot) {
    if (!snapshot?.draft) return;
    if (snapshot.reason !== "terrainStroke") this.rebuildTerrain();
    this.drawOverlay();
  }

  rebuildTerrain() {
    if (!this.session.draft) return;
    const materialized = this.session.materialized();
    this.renderer.buildStaticMap({
      width: materialized.size,
      height: materialized.size,
      tileSize: TILE_SIZE,
      terrain: materialized.terrain,
    });
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
    this.overlay.clear();
    for (const label of this.labels) label.destroy();
    this.labels = [];
    const size = draft.terrain.length;
    this.overlay.lineStyle(1, 0xffffff, 0.08);
    for (let tile = 0; tile <= size; tile += 8) {
      const p = tile * TILE_SIZE;
      this.overlay.moveTo(p, 0).lineTo(p, size * TILE_SIZE);
      this.overlay.moveTo(0, p).lineTo(size * TILE_SIZE, p);
    }
    for (const player of this.session.playerSlots()) {
      const color = hexColor(PLAYER_PALETTE[player.playerIndex % PLAYER_PALETTE.length]);
      if (player.start) this.drawSite(player.start, color, 11, `P${player.playerIndex + 1}`);
      for (const [index, natural] of player.naturals.entries()) {
        this.drawSite(natural, color, 7, `N${index + 1}`);
      }
    }
    this.drawPaintPreview();
  }

  drawPaintPreview() {
    if (this.tool?.kind !== "terrain" || this.tool.shape !== "box" || !this.paintStartTile || !this.lastPaintTile) return;
    const x0 = Math.min(this.paintStartTile.x, this.lastPaintTile.x) * TILE_SIZE;
    const y0 = Math.min(this.paintStartTile.y, this.lastPaintTile.y) * TILE_SIZE;
    const width = (Math.abs(this.lastPaintTile.x - this.paintStartTile.x) + 1) * TILE_SIZE;
    const height = (Math.abs(this.lastPaintTile.y - this.paintStartTile.y) + 1) * TILE_SIZE;
    this.overlay.lineStyle(2, terrainPreviewColor(this.tool.terrain), 0.9);
    this.overlay.beginFill(terrainPreviewColor(this.tool.terrain), 0.16).drawRect(x0, y0, width, height).endFill();
  }

  drawSite(site, color, radius, labelText) {
    const x = (site.x + 0.5) * TILE_SIZE;
    const y = (site.y + 0.5) * TILE_SIZE;
    this.overlay.lineStyle(3, 0x101418, 0.9);
    this.overlay.beginFill(color, 0.82).drawCircle(x, y, radius).endFill();
    const label = new PIXI.Text(labelText, {
      fontFamily: "Inter, system-ui, sans-serif",
      fontSize: 11,
      fontWeight: "700",
      fill: 0xffffff,
      stroke: 0x101418,
      strokeThickness: 3,
    });
    label.anchor.set(0.5, 1);
    label.position.set(x, y - radius - 3);
    this.renderer.layers.feedback.addChild(label);
    this.labels.push(label);
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
      symmetricMapTiles(size, tiles, this.tool?.symmetry),
      this.tool.terrain,
    );
    this.renderer.updateStaticTerrainTiles(changes);
  }

  applySiteTool(tile) {
    const tool = this.tool;
    let result = null;
    const player = Number(tool.playerIndex);
    const label = tool.kind === "start"
      ? `Moved Player ${player + 1} start`
      : tool.naturalId
        ? `Moved Player ${player + 1} natural`
        : `Added Player ${player + 1} natural`;
    const changed = this.session.mutate(label, (draft) => {
      result = tool.kind === "start"
        ? moveSymmetricDraftBase(draft, {
          kind: "main",
          playerIndex: player,
          tile,
          layoutId: this.session.selectedLayoutId,
          symmetry: tool.symmetry,
        })
        : tool.naturalId
          ? moveSymmetricDraftBase(draft, {
            kind: "natural",
            playerIndex: player,
            naturalId: tool.naturalId,
            tile,
            layoutId: this.session.selectedLayoutId,
            symmetry: tool.symmetry,
          })
          : addSymmetricDraftNaturals(draft, {
            playerIndex: player,
            tile,
            layoutId: this.session.selectedLayoutId,
            symmetry: tool.symmetry,
          });
      if (result?.ok) protectDraftBaseTerrain(draft);
    });
    const extra = Math.max(0, Number(result?.count || 1) - 1);
    this.onStatus(
      changed ? `${label}${extra ? ` and ${extra} symmetric base${extra === 1 ? "" : "s"}` : ""}.` : result?.error || "No map change.",
      !changed,
    );
  }

  eventTile(event, { kind = this.tool?.kind } = {}) {
    const rect = this.renderer.app.view.getBoundingClientRect();
    const world = this.camera.screenToWorld(event.clientX - rect.left, event.clientY - rect.top);
    const size = this.session.draft?.terrain?.length || 0;
    const radius = kind === "start"
      ? MAP_EDITOR_MAIN_CLEARANCE_TILES
      : kind === "natural"
        ? MAP_EDITOR_NATURAL_CLEARANCE_TILES
        : 0;
    if (!size || size <= radius * 2) return null;
    return {
      x: Math.max(radius, Math.min(size - radius - 1, Math.floor(world.x / TILE_SIZE))),
      y: Math.max(radius, Math.min(size - radius - 1, Math.floor(world.y / TILE_SIZE))),
    };
  }

  handleWheel(event) {
    const rect = this.renderer.app.view.getBoundingClientRect();
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
    const key = String(event.key || "").toLowerCase();
    const direction = key === "arrowup" || key === "w" ? "up"
      : key === "arrowdown" || key === "s" ? "down"
        : key === "arrowleft" || key === "a" ? "left"
          : key === "arrowright" || key === "d" ? "right" : "";
    if (!direction) return;
    this.keys[direction] = pressed;
    event.preventDefault();
  }

  tick(at) {
    if (this.destroyed) return;
    const dt = Math.min(0.1, Math.max(0, (at - this.lastFrameAt) / 1000));
    this.lastFrameAt = at;
    this.camera.update(dt, { keys: this.keys, mouse: null });
    this.renderer.world.position.set(-this.camera.x * this.camera.zoom, -this.camera.y * this.camera.zoom);
    this.renderer.world.scale.set(this.camera.zoom);
    this.frame = requestAnimationFrame((next) => this.tick(next));
  }

  resize() {
    const width = this.root.clientWidth || window.innerWidth;
    const height = this.root.clientHeight || window.innerHeight;
    this.renderer.app.renderer.resize(width, height);
    this.camera.setBounds(this.camera.worldW, this.camera.worldH, width, height);
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    cancelAnimationFrame(this.frame);
    this.unsubscribe?.();
    const canvas = this.renderer.app.view;
    canvas.removeEventListener("pointerdown", this.onPointerDown);
    canvas.removeEventListener("pointermove", this.onPointerMove);
    canvas.removeEventListener("pointerup", this.onPointerUp);
    canvas.removeEventListener("pointercancel", this.onPointerUp);
    canvas.removeEventListener("wheel", this.onWheel);
    canvas.removeEventListener("contextmenu", this.onContextMenu);
    window.removeEventListener("keydown", this.onKeyDown);
    window.removeEventListener("keyup", this.onKeyUp);
    window.removeEventListener("resize", this.onResize);
    for (const label of this.labels) label.destroy();
    this.overlay.destroy();
    this.renderer.destroy();
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

function hexColor(value) {
  return Number.parseInt(String(value || "#ffffff").replace("#", ""), 16) || 0xffffff;
}

function isTextEntry(target) {
  return ["INPUT", "TEXTAREA", "SELECT"].includes(String(target?.tagName || "")) || !!target?.isContentEditable;
}

function structuredCloneSafe(value) {
  return typeof structuredClone === "function" ? structuredClone(value) : JSON.parse(JSON.stringify(value));
}
