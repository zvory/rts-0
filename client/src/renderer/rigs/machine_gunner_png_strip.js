function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const MACHINE_GUNNER_PNG_FRAME_STRIP = deepFreeze({
  enabled: true,
  unit: "machine_gunner",
  image: "/assets/rigs/machine-gunner-pass-01/machine-gunner-pass-01-strip.png?v=white-chroma-v1-rgba8",
  imageVersion: "white-chroma-v1-rgba8",
  frameWidth: 128,
  frameHeight: 128,
  frameCount: 15,
  idleFrame: 0,
  iconVisibleBounds: {
    x: 6,
    y: 23,
    w: 115,
    h: 81,
  },
  movementFrames: [0, 1, 2, 3, 4, 5],
  setupFrames: [6, 7, 8, 9, 10, 11],
  deployedFrame: 11,
  firingFrames: [12, 13, 14],
  fps: 12,
  worldScale: 0.306,
  movementWorldScale: 0.306,
  movementFacingOffset: -Math.PI / 2,
  tintSlot: "team-light",
  bakedColorAdjustment: {
    brightness: 100,
    saturation: 100,
    hue: 100,
  },
  targetColorAdjustment: {
    brightness: 70,
    saturation: 100,
    hue: 100,
  },
  packedFacing: "body",
  setupForwardAngle: Math.PI / 2,
  source: {
    carrySource: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-white-carry-source.png",
    carryAlpha: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-white-carry-alpha.png",
    deploySource: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-white-deploy-source.png",
    deployAlpha: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-white-deploy-alpha.png",
    fireSource: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-white-fire-source.png",
    fireAlpha: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-white-fire-alpha.png",
    runtimeStrip: "client/assets/rigs/machine-gunner-pass-01/machine-gunner-pass-01-strip.png",
  },
});
