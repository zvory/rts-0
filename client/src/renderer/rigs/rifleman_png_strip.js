function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const RIFLEMAN_PNG_FRAME_STRIP = deepFreeze({
  enabled: true,
  unit: "rifleman",
  image: "/assets/rigs/rifleman-pass-02/generated/rifleman-down-rifle-iteration/rifleman-down-rifle-strip.png?v=down-rifle-iteration-3-backward-recoil",
  imageVersion: "down-rifle-iteration-3-backward-recoil",
  frameWidth: 160,
  frameHeight: 112,
  frameCount: 7,
  idleFrame: 0,
  movementFrames: [1, 2, 3, 4],
  firingFrames: [5, 6],
  firingFrameHoldPhase: 0.28,
  fps: 12,
  worldScale: 0.34,
  tintSlot: "team-light",
  bakedColorAdjustment: {
    brightness: 170,
    saturation: 118,
    hue: 100,
  },
  source: {
    generatedSource: "client/assets/rigs/rifleman-pass-02/generated/rifleman-down-rifle-iteration/rifleman-down-rifle-source-imagegen.png",
    alphaSource: "client/assets/rigs/rifleman-pass-02/generated/rifleman-down-rifle-iteration/rifleman-down-rifle-alpha.png",
    runtimeStrip: "client/assets/rigs/rifleman-pass-02/generated/rifleman-down-rifle-iteration/rifleman-down-rifle-strip.png",
  },
});
