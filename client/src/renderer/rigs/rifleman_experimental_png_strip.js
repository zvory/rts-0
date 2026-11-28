import { RIFLEMAN_EXPERIMENTAL_GEOMETRY } from "./rifleman_experimental_geometry.js";

export const RIFLEMAN_EXPERIMENTAL_FRAME_STRIP = Object.freeze({
  enabled: true, unit: "rifleman",
  image: "/assets/rigs/rifleman-experimental-v3/neutral-atlas.png?v=experimental-3",
  imageVersion: "experimental-3",
  frameWidth: 128, frameHeight: 128, frameColumns: 16, frameCount: 256, idleFrame: 0,
  directional: Object.freeze({ firstFacing: Math.PI / 2, directionCount: 16,
    runStart: 2, runFrames: 6, firingStart: 8, firingFrames: 8 }),
  firingWeaponKinds: Object.freeze(["rifleman_rifle"]),
  poseSequence: true,
  poseGeometry: RIFLEMAN_EXPERIMENTAL_GEOMETRY,
  fps: 11, anchorX: 0.5, anchorY: 112 / 128, worldScale: 0.45,
  groundContactShadow: true,
  teamPaint: Object.freeze({
    baseImage: "/assets/rigs/rifleman-experimental-v3/fixed-atlas.png?v=experimental-3",
    paintImage: "/assets/rigs/rifleman-experimental-v3/paint-atlas.png?v=experimental-3",
  }),
  targetColorAdjustment: Object.freeze({ brightness: 100, saturation: 100, hue: 100 }),
});

export function experimentalRiflemanMuzzle(attacker, scale = 1) {
  const facing = Number.isFinite(attacker.weaponFacing) ? attacker.weaponFacing : attacker.facing || 0;
  const direction = ((Math.round((facing - Math.PI / 2) / (Math.PI / 8)) % 16) + 16) % 16;
  const muzzle = RIFLEMAN_EXPERIMENTAL_GEOMETRY.muzzleByDirection[direction];
  return { x: attacker.x + muzzle.x * 0.45 * scale, y: attacker.y + muzzle.y * 0.45 * scale };
}
