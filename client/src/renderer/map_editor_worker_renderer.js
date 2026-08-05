import { validateMapEditorPresentation } from "../map_editor_presentation.js";
import {
  defaultMapAuthoringLayerVisibility,
  MAP_AUTHORING_LAYER,
  mapAuthoringDoodadVisible,
  normalizeMapAuthoringLayerVisibility,
} from "../map_authoring/layers.js";
import { DOODAD_TYPE } from "../config.js";
import { KIND } from "../protocol.js";
import { drawTankTrap } from "./buildings.js";
import { gfxCircle, gfxFill, gfxFillStrokePath, gfxNoFill, gfxRect, gfxReset, gfxStroke, gfxStrokePaths } from "./native_graphics.js";
import { drawResourceNodeGraphic } from "./resources.js";

export class MapEditorWorkerRenderer {
  constructor(renderer) {
    if (!renderer) throw new TypeError("Map Editor Pixi worker requires a renderer.");
    this.renderer = renderer;
    this.overlay = new PIXI.Graphics();
    renderer.layers.feedback.addChild(this.overlay);
    this.tankTraps = new Map();
    this.resourcePatches = new Map();
    this.doodads = new Map();
    this.labels = [];
    this.lastOverlay = null;
    this.layerVisibility = defaultMapAuthoringLayerVisibility();
    this.terrainRevision = 0;
    this.doodadRevision = 0;
    this.overlayRevision = 0;
    this.lastTiming = Object.freeze({ workerUpdateMs: 0, workerPresentMs: 0 });
    this.destroyed = false;
  }

  present(record) {
    if (this.destroyed) return;
    const updateStartedAt = performance.now();
    validateMapEditorPresentation(record);
    this._applyTerrain(record.terrainUpdate);
    this._applyLayerVisibility(record.layerVisibility);
    this._applyDoodads(record.doodadUpdate);
    this._applyOverlay(record.overlay);
    this.renderer.world.position.set(-record.camera.x * record.camera.zoom, -record.camera.y * record.camera.zoom);
    this.renderer.world.scale.set(record.camera.zoom);
    this.renderer.updateStaticDoodadWind(record.visualTimeMs, record.camera);
    const workerUpdateMs = performance.now() - updateStartedAt;
    const presentStartedAt = performance.now();
    this.renderer.present();
    this.lastTiming = Object.freeze({
      workerUpdateMs,
      workerPresentMs: performance.now() - presentStartedAt,
    });
  }

  resize(width, height, dpr) {
    this.renderer.resize(width, height, dpr);
  }

  _applyTerrain(update) {
    if (!update || update.revision <= this.terrainRevision) return;
    if (update.kind === "replace") {
      this.renderer.buildStaticMap({
        width: update.width,
        height: update.height,
        tileSize: update.tileSize,
        terrain: update.terrain,
      });
    } else {
      this.renderer.updateStaticTerrainTiles(update.changes);
    }
    this.terrainRevision = update.revision;
  }

  _applyDoodads(update) {
    if (!update || update.revision <= this.doodadRevision) return;
    if (update.kind === "replace") {
      this.doodads = new Map((update.doodads || []).map((record) => [record.id, record]));
      this._syncDoodads();
    } else if (update.kind === "patch") {
      for (const id of update.removedIds || []) this.doodads.delete(id);
      for (const record of update.upserts || []) this.doodads.set(record.id, record);
      const visibleUpserts = (update.upserts || [])
        .filter((record) => mapAuthoringDoodadVisible(record, this.layerVisibility));
      const hiddenUpsertIds = (update.upserts || [])
        .filter((record) => !mapAuthoringDoodadVisible(record, this.layerVisibility))
        .map((record) => record.id);
      const removedIds = [...new Set([...(update.removedIds || []), ...hiddenUpsertIds])];
      this._patchTankTraps({ upserts: visibleUpserts, removedIds });
      const tankTrapIds = visibleUpserts.filter(isTankTrap).map((record) => record.id);
      this.renderer.patchStaticDoodads({
        upserts: visibleUpserts.filter((record) => !isTankTrap(record)),
        removedIds: [...new Set([...removedIds, ...tankTrapIds])],
      });
    } else {
      throw new TypeError(`Unsupported Map Editor doodad update ${String(update.kind)}.`);
    }
    this.doodadRevision = update.revision;
  }

  _applyLayerVisibility(source) {
    const next = normalizeMapAuthoringLayerVisibility(source);
    if (sameVisibility(this.layerVisibility, next)) return;
    const doodadVisibilityChanged = [
      MAP_AUTHORING_LAYER.TREES,
      MAP_AUTHORING_LAYER.GAMEPLAY_DOODADS,
      MAP_AUTHORING_LAYER.DECORATIVE_DOODADS,
    ].some((id) => this.layerVisibility[id] !== next[id]);
    this.layerVisibility = next;
    this.renderer.layers.terrain.visible = next[MAP_AUTHORING_LAYER.BASE];
    if (doodadVisibilityChanged) this._syncDoodads();
    if (this.lastOverlay) this._redrawOverlay(this.lastOverlay);
  }

  _syncDoodads() {
    const visible = [...this.doodads.values()]
      .filter((record) => mapAuthoringDoodadVisible(record, this.layerVisibility));
    this._replaceTankTraps(visible);
    this.renderer.replaceStaticDoodads(visible.filter((record) => !isTankTrap(record)));
  }

  _replaceTankTraps(records) {
    const next = new Map((records || []).filter(isTankTrap).map((record) => [record.id, record]));
    for (const id of this.tankTraps.keys()) if (!next.has(id)) this._removeTankTrap(id);
    for (const record of next.values()) this._upsertTankTrap(record);
  }

  _patchTankTraps({ upserts = [], removedIds = [] }) {
    for (const id of removedIds || []) this._removeTankTrap(id);
    for (const record of upserts || []) {
      if (isTankTrap(record)) this._upsertTankTrap(record);
      else this._removeTankTrap(record.id);
    }
  }

  _upsertTankTrap(record) {
    let graphic = this.tankTraps.get(record.id);
    if (!graphic) {
      graphic = new PIXI.Graphics();
      this.renderer.layers.buildings.addChild(graphic);
      this.tankTraps.set(record.id, graphic);
    }
    gfxReset(graphic.clear());
    drawTankTrap(graphic, record.x, record.y, 32, record.id, 1);
  }

  _removeTankTrap(id) {
    const graphic = this.tankTraps.get(id);
    if (!graphic) return;
    graphic.parent?.removeChild?.(graphic);
    graphic.destroy();
    this.tankTraps.delete(id);
  }

  _applyOverlay(overlay) {
    if (!overlay || overlay.revision <= this.overlayRevision) return;
    this.lastOverlay = overlay;
    this._redrawOverlay(overlay);
    this.overlayRevision = overlay.revision;
  }

  _redrawOverlay(overlay) {
    gfxReset(this.overlay.clear());
    for (const label of this.labels) label.destroy();
    this.labels = [];
    drawGameplayOverlays(this.overlay, overlay, this.layerVisibility);
    gfxStrokePaths(this.overlay, overlay.gridPaths, 1, 0xffffff, 0.08);
    if (overlay.guides.length) gfxStrokePaths(this.overlay, overlay.guides, 2, 0xffd878, 0.82);
    if (overlay.guideCentre) {
      gfxStroke(this.overlay, 2, 0xffd878, 0.82);
      gfxCircle(gfxFill(this.overlay, 0xffd878, 0.82), overlay.guideCentre.x, overlay.guideCentre.y, 5);
      gfxNoFill(this.overlay);
    }
    if (this.layerVisibility[MAP_AUTHORING_LAYER.BASE]) {
      this._syncResourcePatches(overlay.resourcePatches || []);
      for (const site of overlay.sites) this._drawSite(site);
    } else {
      this._syncResourcePatches([]);
    }
    if (overlay.doodadBrushPreview) {
      const preview = overlay.doodadBrushPreview;
      const color = preview.mode === "erase" ? 0xff6f6f : 0xc5ef8a;
      gfxStroke(this.overlay, 2, color, 0.9);
      gfxCircle(this.overlay, preview.x, preview.y, Math.max(4, preview.radius || 4));
    }
    if (overlay.paintPreview) {
      const preview = overlay.paintPreview;
      if (Array.isArray(preview.paths)) {
        gfxStrokePaths(this.overlay, preview.paths, preview.lineWidth, preview.color, 0.22);
        gfxStrokePaths(this.overlay, preview.paths, 2, preview.color, 0.94);
      } else {
        gfxStroke(this.overlay, 2, preview.color, 0.9);
        gfxRect(gfxFill(this.overlay, preview.color, 0.16), preview.x, preview.y, preview.width, preview.height);
        gfxNoFill(this.overlay);
      }
    }
  }

  _drawSite(site) {
    if (site.selected) {
      gfxStroke(this.overlay, 2, 0xfff4ba, 0.96);
      gfxCircle(this.overlay, site.x, site.y, site.radius + 6);
    }
    gfxStroke(this.overlay, 3, 0x101418, 0.9);
    gfxCircle(gfxFill(this.overlay, site.color, 0.82), site.x, site.y, site.radius);
    gfxNoFill(this.overlay);
    const label = new PIXI.Text({ text: site.label, style: {
      fontFamily: "Inter, system-ui, sans-serif",
      fontSize: 11,
      fontWeight: "700",
      fill: 0xffffff,
      stroke: { color: 0x101418, width: 3 },
    } });
    label.anchor.set(0.5, 1);
    label.position.set(site.x, site.y - site.radius - 3);
    this.renderer.layers.feedback.addChild(label);
    this.labels.push(label);
  }

  _syncResourcePatches(records) {
    const next = new Set();
    for (const patch of records || []) {
      const key = `${patch.kind}:${patch.x}:${patch.y}`;
      next.add(key);
      if (this.resourcePatches.has(key)) continue;
      const graphic = new PIXI.Graphics();
      graphic.position.set(patch.x, patch.y);
      drawResourceNodeGraphic(graphic, patch.kind === "oil" ? KIND.OIL : KIND.STEEL);
      this.renderer.layers.resources.addChild(graphic);
      this.resourcePatches.set(key, graphic);
    }
    for (const [key, graphic] of this.resourcePatches) {
      if (next.has(key)) continue;
      graphic.parent?.removeChild?.(graphic);
      graphic.destroy();
      this.resourcePatches.delete(key);
    }
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    for (const label of this.labels) label.destroy();
    this.labels = [];
    for (const id of [...this.tankTraps.keys()]) this._removeTankTrap(id);
    this._syncResourcePatches([]);
    this.doodads.clear();
    this.lastOverlay = null;
    this.overlay.destroy();
    this.renderer.destroy();
  }
}

const OVERLAY_VISUALS = Object.freeze({
  concealment: Object.freeze({ color: 0x2f9f78, icon: drawClosedEye }),
  noVehicle: Object.freeze({ color: 0xd94b45, icon: drawNoEntry }),
  noBuilding: Object.freeze({ color: 0xd58a2f, icon: drawNoBuilding }),
  damageReduction: Object.freeze({ color: 0x3e82d7, icon: drawHalfShield }),
  slowMovement: Object.freeze({ color: 0x8b5fc7, icon: drawMiredBoot }),
});

function drawGameplayOverlays(graphics, overlay, visibility) {
  const byTile = new Map();
  const add = (kind, tiles, layer) => {
    if (!visibility[layer]) return;
    for (const tile of tiles || []) {
      const key = `${tile.x}:${tile.y}`;
      const entry = byTile.get(key) || { x: tile.x, y: tile.y, effects: [] };
      if (!entry.effects.includes(kind)) entry.effects.push(kind);
      byTile.set(key, entry);
    }
  };
  for (const tile of visibility[MAP_AUTHORING_LAYER.FOREST] ? overlay.forestTiles || [] : []) {
    const key = `${tile.x}:${tile.y}`;
    byTile.set(key, {
      x: tile.x,
      y: tile.y,
      effects: ["concealment", "noVehicle", "noBuilding", "damageReduction", "slowMovement"],
    });
  }
  add("concealment", overlay.concealmentTiles, MAP_AUTHORING_LAYER.CONCEALMENT);
  add("noVehicle", overlay.noVehicleTiles, MAP_AUTHORING_LAYER.NO_VEHICLE);
  add("noBuilding", overlay.noBuildingTiles, MAP_AUTHORING_LAYER.NO_BUILDING);
  add("damageReduction", overlay.damageReductionTiles, MAP_AUTHORING_LAYER.DAMAGE_REDUCTION);
  add("slowMovement", overlay.slowMovementTiles, MAP_AUTHORING_LAYER.SLOW_MOVEMENT);

  for (const tile of byTile.values()) {
    const shared = tile.effects.length > 1;
    for (let index = 0; index < tile.effects.length; index += 1) {
      const kind = tile.effects[index];
      const visual = OVERLAY_VISUALS[kind];
      const columns = tile.effects.length > 4 ? 3 : 2;
      const cell = shared
        ? { x: tile.x * 32 + 1 + (index % columns) * (columns === 3 ? 10 : 15), y: tile.y * 32 + 1 + Math.floor(index / columns) * 15, size: columns === 3 ? 9 : 14 }
        : { x: tile.x * 32 + 2, y: tile.y * 32 + 2, size: 28 };
      gfxStroke(graphics, shared ? 1 : 1.5, visual.color, 0.96);
      gfxRect(gfxFill(graphics, visual.color, shared ? 0.34 : 0.25), cell.x, cell.y, cell.size, cell.size);
      gfxNoFill(graphics);
      visual.icon(graphics, cell.x + cell.size / 2, cell.y + cell.size / 2, cell.size * 0.72);
    }
  }
}

function drawNoEntry(graphics, cx, cy, size) {
  const radius = size * 0.38;
  gfxStroke(graphics, Math.max(1.2, size * 0.12), 0xffffff, 0.96);
  gfxCircle(graphics, cx, cy, radius);
  gfxStrokePaths(graphics, [[[cx - radius * 0.7, cy + radius * 0.7], [cx + radius * 0.7, cy - radius * 0.7]]], Math.max(1.2, size * 0.13), 0xffffff, 0.96);
}

function drawNoBuilding(graphics, cx, cy, size) {
  const half = size * 0.32;
  gfxStroke(graphics, Math.max(1, size * 0.1), 0xffffff, 0.96);
  gfxStrokePaths(graphics, [
    [[cx - half, cy - size * 0.04], [cx, cy - half], [cx + half, cy - size * 0.04]],
    [[cx - half * 0.75, cy - size * 0.04], [cx - half * 0.75, cy + half], [cx + half * 0.75, cy + half], [cx + half * 0.75, cy - size * 0.04]],
    [[cx - half, cy + half], [cx + half, cy - half]],
  ], Math.max(1, size * 0.1), 0xffffff, 0.96);
}

function drawClosedEye(graphics, cx, cy, size) {
  const radius = size * 0.43;
  const paths = [
    [[cx - radius, cy - radius * 0.12], [cx - radius * 0.5, cy + radius * 0.25], [cx, cy + radius * 0.34], [cx + radius * 0.5, cy + radius * 0.25], [cx + radius, cy - radius * 0.12]],
    [[cx - radius * 0.55, cy + radius * 0.22], [cx - radius * 0.72, cy + radius * 0.58]],
    [[cx, cy + radius * 0.33], [cx, cy + radius * 0.72]],
    [[cx + radius * 0.55, cy + radius * 0.22], [cx + radius * 0.72, cy + radius * 0.58]],
  ];
  gfxStrokePaths(graphics, paths, Math.max(1.1, size * 0.11), 0xffffff, 0.96);
}

function drawHalfShield(graphics, cx, cy, size) {
  const radius = size * 0.43;
  const shield = [
    [cx - radius * 0.72, cy - radius * 0.78],
    [cx, cy - radius],
    [cx + radius * 0.72, cy - radius * 0.78],
    [cx + radius * 0.62, cy + radius * 0.12],
    [cx, cy + radius],
    [cx - radius * 0.62, cy + radius * 0.12],
  ];
  const leftHalf = [shield[0], shield[1], [cx, cy + radius], shield[5]];
  gfxFillStrokePath(graphics, leftHalf, { fill: { color: 0xffffff, alpha: 0.42 }, close: true });
  gfxFillStrokePath(graphics, shield, { stroke: { width: Math.max(1.1, size * 0.1), color: 0xffffff, alpha: 0.96 }, close: true });
  gfxStrokePaths(graphics, [[[cx, cy - radius], [cx, cy + radius]]], Math.max(1, size * 0.08), 0xffffff, 0.9);
}

function drawMiredBoot(graphics, cx, cy, size) {
  const radius = size * 0.45;
  const boot = [
    [cx - radius * 0.62, cy - radius],
    [cx + radius * 0.05, cy - radius],
    [cx + radius * 0.05, cy + radius * 0.12],
    [cx + radius * 0.82, cy + radius * 0.38],
    [cx + radius * 0.82, cy + radius * 0.7],
    [cx - radius * 0.62, cy + radius * 0.7],
  ];
  gfxFillStrokePath(graphics, boot, {
    fill: { color: 0xffffff, alpha: 0.28 },
    stroke: { width: Math.max(1.1, size * 0.1), color: 0xffffff, alpha: 0.96 },
    close: true,
  });
  gfxStrokePaths(graphics, [
    [[cx - radius, cy + radius * 0.72], [cx - radius * 0.55, cy + radius * 0.55], [cx - radius * 0.1, cy + radius * 0.72], [cx + radius * 0.35, cy + radius * 0.55], [cx + radius, cy + radius * 0.72]],
  ], Math.max(1, size * 0.09), 0xffffff, 0.9);
}

function isTankTrap(record) {
  return record?.typeId === DOODAD_TYPE.TANK_TRAP;
}

function sameVisibility(left, right) {
  return Object.keys(right).every((id) => left[id] === right[id]);
}
