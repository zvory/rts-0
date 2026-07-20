import { Camera } from "../camera.js";
import { PixiWorkerPresentationAdapter } from "./pixi_worker_host.js";

export function createPixiBackendBundle() {
  return Object.freeze({
    id: "pixi",
    createCamera(options = {}) {
      return new Camera(0, 0, options);
    },
    async createRenderer(canvasParent, sources) {
      return PixiWorkerPresentationAdapter.create(canvasParent, sources);
    },
  });
}
