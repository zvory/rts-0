import { Camera } from "../camera.js";
import { PixiPresentationAdapter } from "./pixi_compatibility_adapter.js";

export function createPixiBackendBundle() {
  return Object.freeze({
    id: "pixi",
    createCamera(options = {}) {
      return new Camera(0, 0, options);
    },
    async createRenderer(canvasParent, sources) {
      return PixiPresentationAdapter.create(canvasParent, sources);
    },
  });
}
