import { PRODUCTION_RASTER_COLOR_TARGET } from "./color_adjustment.js";

export const LIGHTNESS_FRAME_STRIP_KEY = "warrior:lightness";
export const MAGIC_FRAME_STRIP_KEY = "warrior:magic";

function discipleStrip(image, frameCount, firingFrames, iconVisibleBounds, weaponKind, imageVersion = "dao-ranged-1") {
  return Object.freeze({
    enabled: true,
    unit: "warrior",
    image,
    imageVersion,
    frameWidth: 256,
    frameHeight: 256,
    frameCount,
    idleFrame: 0,
    iconVisibleBounds: Object.freeze(iconVisibleBounds),
    movementFrames: Object.freeze([1, 2, 3, 2]),
    // An actual shot starts draw/windup, release, follow-through and recovery. The
    // projectile presentation launches at phase 0.25 (100ms of its 400ms cycle).
    firingFrames: Object.freeze(firingFrames),
    firingWeaponKinds: Object.freeze([weaponKind]),
    fps: 8,
    worldScale: 0.32,
    facingOffset: -Math.PI / 2,
    firingFacingOffset: -Math.PI / 2,
    originForwardPx: 0,
    firingRecoilPx: 0,
    tintSlot: "team-light",
    bakedColorAdjustment: Object.freeze({ brightness: 100, saturation: 100, hue: 100 }),
    targetColorAdjustment: PRODUCTION_RASTER_COLOR_TARGET,
    source: Object.freeze({
      generator: "scripts/art/disciple-dao-pipeline.mjs",
      runtimeStrip: "client" + image.split("?")[0],
    }),
  });
}

// Candidate B: scarf rests at idle, streams while running, and follows the sword swing.
// Original archer source and runtime strips are preserved for future reuse.
export const LIGHTNESS_PNG_FRAME_STRIP = discipleStrip("/assets/rigs/disciple-dao/generated/lightness-melee-b-runtime-strip.png?v=melee-b-wind-2", 8, [4, 5, 6, 7],
  { x: 75, y: 93, w: 131, h: 120 }, "warrior_sword", "melee-b-wind-2");
export const MAGIC_PNG_FRAME_STRIP = discipleStrip("/assets/rigs/disciple-dao/generated/magic-runtime-strip.png?v=dao-ranged-1", 8, [4, 5, 6, 7],
  { x: 63, y: 80, w: 128, h: 113 }, "disciple_orb");
