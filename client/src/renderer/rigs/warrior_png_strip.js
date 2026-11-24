import { withGoldenAttackTrail } from "./golden_attack_strip.js";
import { PRODUCTION_RASTER_COLOR_TARGET } from "./color_adjustment.js";

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const WARRIOR_PNG_FRAME_STRIP = withGoldenAttackTrail(deepFreeze({
  enabled: true,
  unit: "warrior",
  image: "/assets/rigs/warrior-placeholder-pass-01/generated/warrior-runtime-strip.png?v=placeholder-2",
  imageVersion: "placeholder-2",
  frameWidth: 160,
  frameHeight: 160,
  frameCount: 12,
  idleFrame: 0,
  iconVisibleBounds: { x: 3, y: 48, w: 153, h: 64 },
  movementFrames: [1, 2, 3, 4],
  firingFrames: [6, 7, 8, 9, 10, 11],
  firingWeaponKinds: ["warrior_sword"],
  firingFrameHoldPhase: 0.7,
  fps: 8,
  worldScale: 0.32,
  // Match the idle head radius (17.43 px) to the generated poses (10.96 px).
  firingWorldScale: 0.32 * 1.59,
  // Preserve the idle head's world anchor while changing facing axes and scale.
  firingOriginForwardPx: 17.4,
  firingOriginLateralPx: 5.2,
  facingOffset: -Math.PI / 2,
  firingFacingOffset: 0,
  // Idle torso sits 22 source pixels left of the cell midpoint.
  originForwardPx: 22 * 0.32,
  firingRecoilPx: 0,
  tintSlot: "team-light",
  bakedColorAdjustment: { brightness: 100, saturation: 100, hue: 100 },
  targetColorAdjustment: PRODUCTION_RASTER_COLOR_TARGET,
  source: {
    generatedSource: "client/assets/rigs/warrior-placeholder-pass-01/generated/warrior-source.png",
    runtimeStrip: "client/assets/rigs/warrior-placeholder-pass-01/generated/warrior-runtime-strip.png",
  },
}));
