function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const MACHINE_GUNNER_PNG_FRAME_STRIP = deepFreeze({
  enabled: true,
  unit: "machine_gunner",
  image: "/assets/rigs/machine-gunner-pass-01/machine-gunner-pass-01-strip.png?v=pass01-setup",
  imageVersion: "pass01-setup",
  frameWidth: 128,
  frameHeight: 128,
  frameCount: 12,
  idleFrame: 0,
  movementFrames: [0, 1, 2, 3, 4, 5],
  setupFrames: [6, 7, 8, 9, 10, 11],
  deployedFrame: 11,
  fps: 12,
  worldScale: 0.42,
  tintSlot: "team-light",
  packedFacing: "body",
  setupForwardAngle: Math.PI / 2,
  source: {
    carrySource: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-pass-01-carry-source.png",
    carryAlpha: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-pass-01-carry-alpha.png",
    deploySource: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-pass-01-deploy-source.png",
    deployAlpha: "client/assets/rigs/machine-gunner-pass-01/generated/machine-gunner-pass-01-deploy-alpha.png",
    runtimeStrip: "client/assets/rigs/machine-gunner-pass-01/machine-gunner-pass-01-strip.png",
  },
});
