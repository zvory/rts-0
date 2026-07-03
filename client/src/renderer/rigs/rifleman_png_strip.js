function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const RIFLEMAN_PNG_FRAME_STRIP = deepFreeze({
  enabled: true,
  unit: "rifleman",
  image: "/assets/rigs/rifleman-pass-02/rifleman-pass-02-strip.png?v=pass02-balanced",
  imageVersion: "pass02-balanced",
  frameWidth: 96,
  frameHeight: 96,
  frameCount: 6,
  idleFrame: 0,
  movementFrames: [1, 2, 3, 4],
  fps: 12,
  worldScale: 0.34,
  tintSlot: "team-light",
  source: {
    generatedSource: "client/assets/rigs/rifleman-pass-02/generated/rifleman-pass-02-source.png",
    alphaSource: "client/assets/rigs/rifleman-pass-02/generated/rifleman-pass-02-alpha.png",
    runtimeStrip: "client/assets/rigs/rifleman-pass-02/rifleman-pass-02-strip.png",
  },
});
