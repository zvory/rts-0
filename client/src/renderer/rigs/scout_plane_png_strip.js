function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const SCOUT_PLANE_PNG_FRAME_STRIP = deepFreeze({
  enabled: true,
  unit: "scout_plane",
  image: "/assets/rigs/scout-plane-fw189-pass-01/generated/scout-plane-fw189-pass-01-alpha.png?v=pass01-fw189-detailed-team-tint-b69-s73",
  imageVersion: "pass01-fw189-detailed-team-tint-b69-s73",
  frameWidth: 942,
  frameHeight: 1163,
  frameCount: 1,
  idleFrame: 0,
  fps: 12,
  worldScale: 0.065,
  tintSlot: "team-light",
  targetColorAdjustment: {
    brightness: 69,
    saturation: 73,
    hue: 100,
  },
  source: {
    generatedSource: "client/assets/rigs/scout-plane-fw189-pass-01/generated/scout-plane-fw189-pass-01-source.png",
    alphaSource: "client/assets/rigs/scout-plane-fw189-pass-01/generated/scout-plane-fw189-pass-01-alpha.png",
  },
});
