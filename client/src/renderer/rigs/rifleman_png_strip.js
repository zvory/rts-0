function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const RIFLEMAN_PNG_FRAME_STRIP = deepFreeze({
  enabled: true,
  unit: "rifleman",
  image: "/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/recoil-pass-01/rifleman-recoil-review-strip.png?v=recoil-review-1",
  imageVersion: "recoil-review-1",
  frameWidth: 160,
  frameHeight: 112,
  frameCount: 5,
  idleFrame: 0,
  movementFrames: [1, 2, 3],
  firingFrames: [4],
  firingFrameHoldPhase: 0.2,
  fps: 8,
  worldScale: 0.34,
  originForwardPx: 10,
  firingRecoilPx: 4,
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
  source: {
    generatedSource: "client/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/no-pack/idle-source.png",
    alphaSource: "client/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/no-pack/idle-alpha.png",
    runtimeStrip: "client/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/recoil-pass-01/rifleman-recoil-review-strip.png",
  },
});
