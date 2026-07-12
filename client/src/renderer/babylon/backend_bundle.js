import { FixedPerspectiveCamera } from "../../fixed_perspective_camera.js";
import { BabylonPresentationAdapter } from "./presentation_adapter.js";

export function createBabylonBackendBundle({ Babylon }) {
  if (!Babylon) throw new TypeError("Babylon backend bundle requires the pinned dependency.");
  return Object.freeze({
    id: "babylon",
    createCamera(options = {}) {
      return new FixedPerspectiveCamera(0, 0, options);
    },
    createRenderer(canvasParent) {
      return new BabylonPresentationAdapter(canvasParent, { Babylon });
    },
  });
}
