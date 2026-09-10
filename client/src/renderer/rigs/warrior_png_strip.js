import { PRODUCTION_RASTER_COLOR_TARGET } from "./color_adjustment.js";

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const WARRIOR_PNG_FRAME_STRIP = deepFreeze({
  enabled: true,
  unit: "warrior",
  image: "/assets/rigs/warrior-placeholder-pass-01/generated/warrior-runtime-strip.png?v=placeholder-1",
  imageVersion: "placeholder-1",
  frameWidth: 160,
  frameHeight: 160,
  frameCount: 6,
  idleFrame: 0,
  iconVisibleBounds: { x: 3, y: 48, w: 153, h: 64 },
  movementFrames: [1, 2, 3, 4],
  firingFrames: [5],
  firingWeaponKinds: ["warrior_sword"],
  firingFrameHoldPhase: 0.7,
  fps: 8,
  worldScale: 0.32,
  originForwardPx: 13,
  firingRecoilPx: 0,
  tintSlot: "team-light",
  bakedColorAdjustment: { brightness: 100, saturation: 100, hue: 100 },
  targetColorAdjustment: PRODUCTION_RASTER_COLOR_TARGET,
  source: {
    generatedSource: "client/assets/rigs/warrior-placeholder-pass-01/generated/warrior-source.png",
    runtimeStrip: "client/assets/rigs/warrior-placeholder-pass-01/generated/warrior-runtime-strip.png",
  },
});
