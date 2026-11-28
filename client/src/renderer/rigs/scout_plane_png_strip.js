import { PRODUCTION_RASTER_COLOR_TARGET } from "./color_adjustment.js";

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const SCOUT_PLANE_PNG_FRAME_STRIP = deepFreeze({
  enabled: true,
  unit: "scout_plane",
  image: "/assets/rigs/kriegsia-team-color/generated/scout_plane-atlas.png?v=team-paint-1",
  sheetRows: 4,
  teamPaint: { baseRow: 2, paintRow: 3, renderRows: 1 },
  imageVersion: "team-paint-1",
  frameWidth: 942,
  frameHeight: 1163,
  frameCount: 1,
  idleFrame: 0,
  iconVisibleBounds: {
    x: 24,
    y: 24,
    w: 894,
    h: 1115,
  },
  fps: 12,
  worldScale: 0.065,
  shadowHeightTiles: 3,
  tintSlot: "team-light",
  targetColorAdjustment: PRODUCTION_RASTER_COLOR_TARGET,
  source: {
    runtimeStrip: "client/assets/rigs/kriegsia-team-color/generated/scout_plane-atlas.png",
    generatedSource: "client/assets/rigs/scout-plane-fw189-pass-01/generated/scout-plane-fw189-pass-01-source.png",
    alphaSource: "client/assets/rigs/scout-plane-fw189-pass-01/generated/scout-plane-fw189-pass-01-alpha.png",
  },
});
