import { FixedPerspectiveCamera } from "../../fixed_perspective_camera.js";
import { BabylonPresentationAdapter } from "./presentation_adapter.js";
import { liveUnitIconMarkupFor } from "../rigs/unit_icon_sources.js";

export function createBabylonBackendBundle({ Babylon }) {
  if (!Babylon) throw new TypeError("Babylon backend bundle requires the pinned dependency.");
  return Object.freeze({
    id: "babylon",
    unitIconMarkupForKind: liveUnitIconMarkupFor,
    createCamera(options = {}) {
      return new FixedPerspectiveCamera(0, 0, options);
    },
    createRenderer(canvasParent, sources) {
      return new BabylonPresentationAdapter(canvasParent, { Babylon, sources });
    },
  });
}
