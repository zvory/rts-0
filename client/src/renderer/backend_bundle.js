import { Camera } from "../camera.js";
import { PixiPresentationAdapter } from "./pixi_compatibility_adapter.js";

export function createPixiBackendBundle() {
  return Object.freeze({
    id: "pixi",
    createCamera(options = {}) {
      return new Camera(0, 0, options);
    },
    createRenderer(canvasParent, sources) {
      return new PixiPresentationAdapter(canvasParent, sources);
    },
  });
}
