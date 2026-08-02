import { validateMapEditorPresentation } from "../map_editor_presentation.js";
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
    this.labels = [];
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
      this._replaceTankTraps(update.doodads || []);
      this.renderer.replaceStaticDoodads((update.doodads || []).filter((record) => !isTankTrap(record)));
    } else if (update.kind === "patch") {
      this._patchTankTraps(update);
      const tankTrapIds = (update.upserts || []).filter(isTankTrap).map((record) => record.id);
      this.renderer.patchStaticDoodads({
        upserts: (update.upserts || []).filter((record) => !isTankTrap(record)),
        removedIds: [...new Set([...(update.removedIds || []), ...tankTrapIds])],
      });
    } else {
      throw new TypeError(`Unsupported Map Editor doodad update ${String(update.kind)}.`);
    }
    this.doodadRevision = update.revision;
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
    gfxReset(this.overlay.clear());
    for (const label of this.labels) label.destroy();
    this.labels = [];
    for (const tile of overlay.stealthTiles || []) {
      gfxStroke(this.overlay, 1, 0x5ed19a, 0.72);
      gfxRect(gfxFill(this.overlay, 0x2d8c64, 0.24), tile.x * 32, tile.y * 32, 32, 32);
      gfxNoFill(this.overlay);
    }
    for (const tile of overlay.noVehicleTiles || []) {
      gfxStroke(this.overlay, 2, 0xf2b866, 0.88);
      gfxRect(gfxFill(this.overlay, 0xc26a2e, 0.12), tile.x * 32 + 2, tile.y * 32 + 2, 28, 28);
      gfxNoFill(this.overlay);
    }
    gfxStrokePaths(this.overlay, overlay.gridPaths, 1, 0xffffff, 0.08);
    if (overlay.guides.length) gfxStrokePaths(this.overlay, overlay.guides, 2, 0xffd878, 0.82);
    if (overlay.guideCentre) {
      gfxStroke(this.overlay, 2, 0xffd878, 0.82);
      gfxCircle(gfxFill(this.overlay, 0xffd878, 0.82), overlay.guideCentre.x, overlay.guideCentre.y, 5);
      gfxNoFill(this.overlay);
    }
    for (const site of overlay.sites) this._drawSite(site);
    if (overlay.doodadSelection) {
      gfxStroke(this.overlay, 2, 0xfff4ba, 0.96);
      gfxCircle(this.overlay, overlay.doodadSelection.x, overlay.doodadSelection.y, 15);
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
    this.overlayRevision = overlay.revision;
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
    this.overlay.destroy();
    this.renderer.destroy();
  }
}

function isTankTrap(record) {
  return record?.typeId === DOODAD_TYPE.TANK_TRAP;
}
