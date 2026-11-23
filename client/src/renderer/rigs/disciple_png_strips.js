import { DAO_WAR_BODY_SCALE, DAO_MASTER_BODY_SCALE } from "../../config.js";
import { PRODUCTION_RASTER_COLOR_TARGET } from "./color_adjustment.js";

export const LIGHTNESS_FRAME_STRIP_KEY = "warrior:lightness";
export const WAR_FRAME_STRIP_KEY = "warrior:war";
export const MAGIC_FRAME_STRIP_KEY = "warrior:magic";
export const LIGHTNESS_MASTER_FRAME_STRIP_KEY = "warrior:lightness:master";
export const WAR_MASTER_FRAME_STRIP_KEY = "warrior:war:master";
export const MAGIC_MASTER_FRAME_STRIP_KEY = "warrior:magic:master";

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
    // Center the torso, rather than the transparent atlas cell, on the entity.
    originLateralPx: -4,
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

// Armored Yari B with a longer reach and an event-driven forward step.
export const WAR_PNG_FRAME_STRIP = Object.freeze({
  ...discipleStrip("/assets/rigs/disciple-dao/generated/war-yari-runtime-strip.png?v=kabuto-dark-hands-3", 7, [3, 4, 5, 6],
    { x: 92, y: 107, w: 74, h: 143 }, "warrior_sword", "kabuto-dark-hands-3"),
  movementFrames: Object.freeze([1, 2]),
  worldScale: 0.8 * DAO_WAR_BODY_SCALE,
  // The Yari torso is centered near source y=146, not the cell midpoint (128).
  originLateralPx: -18 * 0.8 * DAO_WAR_BODY_SCALE,
  // Sword feedback lasts 720 ms; finish this authored clip after 400 ms.
  firingFrameHoldPhase: 400 / 720,
  firingLungePx: 16 * DAO_WAR_BODY_SCALE,
});

function masterStrip(adept, path, bounds) {
  return Object.freeze({
    ...adept,
    image: `/assets/rigs/disciple-dao/generated/${path}-master-runtime-strip.png?v=master-1`,
    imageVersion: "master-1",
    frameCount: 8,
    movementFrames: Object.freeze([1, 2, 3, 2]),
    firingFrames: Object.freeze([4, 5, 6, 7]),
    worldScale: adept.worldScale * DAO_MASTER_BODY_SCALE,
    originLateralPx: adept.originLateralPx * DAO_MASTER_BODY_SCALE,
    firingLungePx: (adept.firingLungePx || 0) * DAO_MASTER_BODY_SCALE,
    iconVisibleBounds: Object.freeze(bounds),
    source: Object.freeze({ generator: "scripts/art/disciple-dao-pipeline.mjs",
      runtimeStrip: `client/assets/rigs/disciple-dao/generated/${path}-master-runtime-strip.png` }),
  });
}
export const LIGHTNESS_MASTER_PNG_FRAME_STRIP = masterStrip(LIGHTNESS_PNG_FRAME_STRIP, "lightness",
  { x: 42, y: 24, w: 175, h: 205 });
export const WAR_MASTER_PNG_FRAME_STRIP = Object.freeze({
  ...masterStrip(WAR_PNG_FRAME_STRIP, "war", { x: 145, y: 155, w: 95, h: 185 }),
  image: "/assets/rigs/disciple-dao/generated/war-master-runtime-strip.png?v=master-torso-2",
  imageVersion: "master-torso-2",
  frameWidth: 384,
  frameHeight: 384,
});
export const MAGIC_MASTER_PNG_FRAME_STRIP = masterStrip(MAGIC_PNG_FRAME_STRIP, "magic",
  { x: 52, y: 53, w: 152, h: 151 });
