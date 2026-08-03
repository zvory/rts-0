import { PixiWorkerPresentationAdapter } from "./pixi_worker_host.js";

export class MapEditorPixiPresentationAdapter {
  static async create(root) {
    const host = await PixiWorkerPresentationAdapter.create(root, {}, { surface: "mapEditor" });
    return new MapEditorPixiPresentationAdapter(host);
  }

  constructor(host) {
    if (!host) throw new TypeError("Map Editor Pixi adapter requires a worker host.");
    this.host = host;
    this.destroyed = false;
  }

  get canvas() {
    return this.host.app.canvas;
  }

  present(record) {
    if (this.destroyed) return;
    return this.host.presentEditor(record);
  }

  readPresentedPixels(frameId) {
    if (this.destroyed) throw new Error("Map Editor Pixi adapter is destroyed.");
    return this.host.readPresentedPixels(frameId);
  }

  captureReadiness() {
    return this.host.captureReadiness();
  }

  enterFixedCapture() {
    if (this.destroyed) throw new Error("Map Editor Pixi adapter is destroyed.");
    this.host.enterFixedCapture();
  }

  exitFixedCapture() {
    this.host.exitFixedCapture();
  }

  resize(width, height) {
    this.host.resize(width, height);
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    this.host.destroy();
  }
}
