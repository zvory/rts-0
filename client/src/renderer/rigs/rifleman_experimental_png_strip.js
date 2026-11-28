export const RIFLEMAN_EXPERIMENTAL_FRAME_STRIP = Object.freeze({
  enabled: true,
  unit: "rifleman",
  image: "/assets/rigs/rifleman-experimental/neutral-atlas.png?v=experimental-2",
  imageVersion: "experimental-2",
  frameWidth: 128,
  frameHeight: 128,
  frameColumns: 16,
  frameCount: 144,
  idleFrame: 0,
  directional: Object.freeze({
    firstFacing: Math.PI / 2, directionCount: 16,
    runStart: 16, runFrames: 4, firingStart: 80, firingFrames: 4,
  }),
  firingWeaponKinds: Object.freeze(["rifleman_rifle"]),
  fps: 10,
  anchorX: 0.5,
  anchorY: 112 / 128,
  worldScale: 0.45,
  groundContactShadow: true,
  teamPaint: Object.freeze({
    baseImage: "/assets/rigs/rifleman-experimental/fixed-atlas.png?v=experimental-2",
    paintImage: "/assets/rigs/rifleman-experimental/paint-atlas.png?v=experimental-2",
  }),
  targetColorAdjustment: Object.freeze({ brightness: 100, saturation: 100, hue: 100 }),
});

// Authored oblique muzzle envelope in world pixels, relative to the planted feet.
export function experimentalRiflemanMuzzle(attacker, scale = 1) {
  const facing = Number.isFinite(attacker.weaponFacing) ? attacker.weaponFacing : attacker.facing || 0;
  const step = Math.PI / 8;
  const angle = Math.PI / 2 + Math.round((facing - Math.PI / 2) / step) * step;
  return { x: attacker.x + Math.cos(angle) * 24 * scale,
    y: attacker.y + (-30 + Math.sin(angle) * 15) * scale };
}
