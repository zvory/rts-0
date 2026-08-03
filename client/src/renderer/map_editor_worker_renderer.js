import { validateMapEditorPresentation } from "../map_editor_presentation.js";
import {
  defaultMapAuthoringLayerVisibility,
  MAP_AUTHORING_LAYER,
  mapAuthoringDoodadVisible,
  normalizeMapAuthoringLayerVisibility,
} from "../map_authoring/layers.js";
import { DOODAD_TYPE } from "../config.js";
import { drawTankTrap } from "./buildings.js";
import { gfxCircle, gfxFill, gfxNoFill, gfxRect, gfxReset, gfxStroke, gfxStrokePaths } from "./native_graphics.js";

export class MapEditorWorkerRenderer {
  constructor(renderer) {
    if (!renderer) throw new TypeError("Map Editor Pixi worker requires a renderer.");
    this.renderer = renderer;
    this.overlay = new PIXI.Graphics();
    renderer.layers.feedback.addChild(this.overlay);
    this.tankTraps = new Map();
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
    if (this.layerVisibility[MAP_AUTHORING_LAYER.STEALTH]) {
      for (const tile of overlay.stealthTiles || []) {
        gfxStroke(this.overlay, 1, 0x5ed19a, 0.72);
        gfxRect(gfxFill(this.overlay, 0x2d8c64, 0.24), tile.x * 32, tile.y * 32, 32, 32);
        gfxNoFill(this.overlay);
      }
    }
    if (this.layerVisibility[MAP_AUTHORING_LAYER.NO_VEHICLE]) {
      for (const tile of overlay.noVehicleTiles || []) {
        gfxStroke(this.overlay, 2, 0xf2b866, 0.88);
        gfxRect(gfxFill(this.overlay, 0xc26a2e, 0.12), tile.x * 32 + 2, tile.y * 32 + 2, 28, 28);
        gfxNoFill(this.overlay);
      }
    }
    gfxStrokePaths(this.overlay, overlay.gridPaths, 1, 0xffffff, 0.08);
    if (overlay.guides.length) gfxStrokePaths(this.overlay, overlay.guides, 2, 0xffd878, 0.82);
    if (overlay.guideCentre) {
      gfxStroke(this.overlay, 2, 0xffd878, 0.82);
      gfxCircle(gfxFill(this.overlay, 0xffd878, 0.82), overlay.guideCentre.x, overlay.guideCentre.y, 5);
      gfxNoFill(this.overlay);
    }
    if (this.layerVisibility[MAP_AUTHORING_LAYER.BASE]) {
      for (const site of overlay.sites) this._drawSite(site);
    }
    for (const selection of overlay.doodadSelections || []) {
      gfxStroke(this.overlay, 2, 0xfff4ba, 0.96);
      gfxCircle(this.overlay, selection.x, selection.y, 15);
    }
    if (overlay.doodadSelectionBox) {
      const box = overlay.doodadSelectionBox;
      gfxStroke(this.overlay, 2, 0xfff4ba, 0.96);
      gfxRect(gfxFill(this.overlay, 0xfff4ba, 0.12), box.x, box.y, box.width, box.height);
      gfxNoFill(this.overlay);
    }
    if (overlay.doodadBrushPreview) {
      const preview = overlay.doodadBrushPreview;
      const color = preview.mode === "erase" ? 0xff6f6f : 0xc5ef8a;
      gfxStroke(this.overlay, 2, color, 0.9);
      gfxCircle(this.overlay, preview.x, preview.y, Math.max(4, preview.radius || 4));
    }
    if (overlay.paintPreview) {
      const preview = overlay.paintPreview;
      gfxStroke(this.overlay, 2, preview.color, 0.9);
      gfxRect(gfxFill(this.overlay, preview.color, 0.16), preview.x, preview.y, preview.width, preview.height);
      gfxNoFill(this.overlay);
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

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    for (const label of this.labels) label.destroy();
    this.labels = [];
    for (const id of [...this.tankTraps.keys()]) this._removeTankTrap(id);
    this.doodads.clear();
    this.lastOverlay = null;
    this.overlay.destroy();
    this.renderer.destroy();
  }
}

function isTankTrap(record) {
  return record?.typeId === DOODAD_TYPE.TANK_TRAP;
}

function sameVisibility(left, right) {
  return Object.keys(right).every((id) => left[id] === right[id]);
}
