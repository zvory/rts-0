import { PRODUCTION_RASTER_COLOR_TARGET } from "./color_adjustment.js";
import { ROCKET_LAUNCHER_PARTS } from "./vehicle_svg.js";

// Team-tint the truck body and preserve the existing facing animation.
export const ROCKET_TRUCK_PREVIEW_ATLAS = Object.freeze({
  enabled: true,
  unit: "rocket_launcher",
  image: "/assets/rigs/kriegsia-team-color/generated/rocket_launcher-atlas.png?v=team-paint-1",
  runtimeColorAdjustment: PRODUCTION_RASTER_COLOR_TARGET,
  sheetRows: 4,
  teamPaint: { baseRow: 2, paintRow: 3, renderRows: 1 },
  viewBox: { x: -36, y: -28, width: 72, height: 56 },
  iconComposition: { sprites: ["sprite.truck"] },
  grid: { layout: "semantic", width: 1774, height: 887 },
  sprites: [{
    id: "sprite.truck",
    animationPart: "part.hull",
    sourceParts: ROCKET_LAUNCHER_PARTS.unit,
    tintSlot: "fixed",
    drawOrder: 20,
    frame: {
      x: 0, y: 0, w: 1774, h: 887,
      originX: 887, originY: 455,
      pixelsPerUnitX: 26 / 1.2, pixelsPerUnitY: 26 / 1.2,
    },
  }],
});
