import { createMinimapAnimation } from "./renderer/rigs/minimap_animation.js";
import { createFormationGesture } from "./input/formation_gesture.js";
import { createMinimapUnitIconLoader } from "./minimap_icon_image.js";
import { Minimap } from "./minimap.js";
import { MinimapGpuBridge } from "./minimap_gpu_bridge.js";
import { liveUnitIconMarkupFor } from "./renderer/rigs/unit_icon_sources.js";
import { KIND } from "./protocol.js";

export function createMatchMinimap(match, dom) {
  return new Minimap(dom.minimap, match.state, match.camera, match.fog, match.commandInteraction, match.inputRouter, {
    commandsEnabled: !!match.capabilities.commands.gameplay,
    createFormationGesture,
    clientIntent: match.clientIntent,
    controlPolicy: match.controlPolicy,
    gpuBridge: new MinimapGpuBridge(dom.minimap, match.renderer.app.canvas),
    unitAnimation: createMinimapAnimation({ simpleTank: true }),
    loadUnitIcon: createMinimapUnitIconLoader(liveUnitIconMarkupFor, dom.minimap),
    artilleryIconMarkup: liveUnitIconMarkupFor(KIND.ARTILLERY),
  });
}
