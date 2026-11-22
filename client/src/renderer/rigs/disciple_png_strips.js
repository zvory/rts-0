import { PRODUCTION_RASTER_COLOR_TARGET } from "./color_adjustment.js";

export const LIGHTNESS_FRAME_STRIP_KEY = "warrior:lightness";
export const MAGIC_FRAME_STRIP_KEY = "warrior:magic";

function discipleStrip(dao, frameCount, firingFrames, iconVisibleBounds, weaponKind) {
  return Object.freeze({
    enabled: true,
    unit: "warrior",
    image: `/assets/rigs/disciple-dao/generated/${dao}-runtime-strip.png?v=dao-ranged-1`,
    imageVersion: "dao-ranged-1",
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
      runtimeStrip: `client/assets/rigs/disciple-dao/generated/${dao}-runtime-strip.png`,
    }),
  });
}

export const LIGHTNESS_PNG_FRAME_STRIP = discipleStrip("lightness", 9, [5, 6, 7, 8],
  { x: 49, y: 71, w: 159, h: 126 }, "disciple_arrow");
export const MAGIC_PNG_FRAME_STRIP = discipleStrip("magic", 8, [4, 5, 6, 7],
  { x: 59, y: 66, w: 133, h: 118 }, "disciple_orb");
