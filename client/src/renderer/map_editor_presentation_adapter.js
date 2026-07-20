import { Renderer } from "./index.js";
import { gfxCircle, gfxStrokePaths, gfxRect, gfxReset, gfxNoFill, gfxFill, gfxStroke } from "./native_graphics.js";
import { validateMapEditorPresentation } from "../map_editor_presentation.js";

export class MapEditorPixiPresentationAdapter {
  static async create(root) {
    const renderer = await Renderer.create(root);
    return new MapEditorPixiPresentationAdapter(renderer);
  }

  constructor(renderer) {
    if (!renderer) throw new TypeError("Map Editor Pixi adapter requires a renderer");
    this.renderer = renderer;
    this.overlay = new PIXI.Graphics();
    renderer.layers.feedback.addChild(this.overlay);
    this.labels = [];
    this.terrainRevision = 0;
    this.overlayRevision = 0;
    this.destroyed = false;
  }

  get canvas() {
    return this.renderer.app.canvas;
  }

  present(record) {
    if (this.destroyed) return;
    validateMapEditorPresentation(record);
    this._applyTerrain(record.terrainUpdate);
    this._applyOverlay(record.overlay);
    this.renderer.world.position.set(-record.camera.x * record.camera.zoom, -record.camera.y * record.camera.zoom);
    this.renderer.world.scale.set(record.camera.zoom);
    this.renderer.present();
  }

  resize(width, height) {
    this.renderer.resize(width, height);
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

  _applyOverlay(overlay) {
    if (!overlay || overlay.revision <= this.overlayRevision) return;
    gfxReset(this.overlay.clear());
    for (const label of this.labels) label.destroy();
    this.labels = [];
    gfxStrokePaths(this.overlay, overlay.gridPaths, 1, 0xffffff, 0.08);
    if (overlay.guides.length) gfxStrokePaths(this.overlay, overlay.guides, 2, 0xffd878, 0.82);
    if (overlay.guideCentre) {
      gfxStroke(this.overlay, 2, 0xffd878, 0.82);
      gfxCircle(gfxFill(this.overlay, 0xffd878, 0.82), overlay.guideCentre.x, overlay.guideCentre.y, 5);
      gfxNoFill(this.overlay);
    }
    for (const site of overlay.sites) this._drawSite(site);
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
    this.overlay.destroy();
    this.renderer.destroy();
  }
}
