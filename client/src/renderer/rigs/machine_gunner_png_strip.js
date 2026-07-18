function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const MACHINE_GUNNER_PNG_FRAME_STRIP = deepFreeze({
  enabled: true,
  unit: "machine_gunner",
  image: "/assets/rigs/machine-gunner-pass-01/machine-gunner-pass-01-strip.png?v=pass01-halfres-rgba8",
  imageVersion: "pass01-halfres-rgba8",
  frameWidth: 64,
  frameHeight: 64,
  frameCount: 15,
  idleFrame: 0,
  movementFrames: [0, 1, 2, 3, 4, 5],
  setupFrames: [6, 7, 8, 9, 10, 11],
  deployedFrame: 11,
  firingFrames: [12, 13, 14],
  fps: 12,
  worldScale: 0.84,
  movementWorldScale: 0.612,
  movementFacingOffset: -Math.PI / 2,
  tintSlot: "team-light",
  bakedColorAdjustment: {
    brightness: 100,
    saturation: 100,
    hue: 100,
  },
  targetColorAdjustment: {
    brightness: 145,
    saturation: 118,
    hue: 100,
  },
  packedFacing: "body",
  setupForwardAngle: Math.PI / 2,
  source: {
    carrySource: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-pass-01-carry-source.png",
    carryAlpha: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-pass-01-carry-alpha.png",
    deploySource: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-pass-01-deploy-source.png",
    deployAlpha: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-pass-01-deploy-alpha.png",
    fireRecoilSource: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-pass-01-fire-recoil-source.png",
    fireRecoilAlpha: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-pass-01-fire-recoil-alpha.png",
    fireRecoilStrip: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-pass-01-fire-recoil-strip.png",
    runtimeStrip: "client/assets/rigs/machine-gunner-pass-01/machine-gunner-pass-01-strip.png",
  },
});
