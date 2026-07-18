function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP = deepFreeze({
  enabled: true,
  unit: "rifleman",
  image: "/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/panzerfaust-composited/rifleman-panzerfaust-composited-strip.png?v=panzerfaust-composited-dim70-1",
  imageVersion: "panzerfaust-composited-dim70-1",
  frameWidth: 160,
  frameHeight: 112,
  frameCount: 4,
  idleFrame: 0,
  movementFrames: [1, 2, 3],
  firingFrames: [],
  fps: 8,
  worldScale: 0.34,
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
    launcherSource: "client/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/panzerfaust-back/idle-runtime.png",
    launcherLayer: "client/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/panzerfaust-composited/launcher-main-layer.png",
    runtimeStrip: "client/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/generated/white/panzerfaust-composited/rifleman-panzerfaust-composited-strip.png",
  },
});
