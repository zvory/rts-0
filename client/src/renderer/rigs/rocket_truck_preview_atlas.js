import { ROCKET_LAUNCHER_PARTS } from "./vehicle_svg.js";

// Team-tint the truck body and preserve the existing facing animation.
export const ROCKET_TRUCK_PREVIEW_ATLAS = Object.freeze({
  enabled: true,
  unit: "rocket_launcher",
  image: "/assets/rigs/rocket-truck-preview/truck.png",
  viewBox: { x: -36, y: -28, width: 72, height: 56 },
  grid: { layout: "semantic", width: 1774, height: 887 },
  sprites: [{
    id: "sprite.truck",
    animationPart: "part.hull",
    sourceParts: ROCKET_LAUNCHER_PARTS.unit,
    tintSlot: "team-light",
    drawOrder: 20,
    frame: {
      x: 0, y: 0, w: 1774, h: 887,
      originX: 887, originY: 455,
      pixelsPerUnitX: 26 / 1.2, pixelsPerUnitY: 26 / 1.2,
    },
  }],
});
