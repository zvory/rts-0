function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const COMMAND_CAR_PNG_RIG_ATLAS = deepFreeze({
  enabled: true,
  unit: "command_car",
  image: "/assets/rigs/command-car-packed-radio-preview/generated/command-car-packed-radio-stars-30-atlas-v4.png?v=stars-30-preview-4",
  runtimeColorAdjustment: {
    brightness: 90,
    saturation: 90,
    hue: 100,
  },
  viewBox: { x: -36, y: -28, width: 72, height: 56 },
  grid: {
    layout: "semantic",
    width: 1623,
    height: 1684,
    sourceSheet: "command-car-packed-radio-stars-30-alpha-v4.png",
    cells: ["sprite.fixed", "sprite.paint"],
    imageVersion: "stars-30-preview-4",
  },
  iconComposition: {
    sprites: ["sprite.fixed", "sprite.paint"],
  },
  frames: {},
  sprites: [
    {
      id: "sprite.fixed",
      animationPart: "part.hull",
      sourceParts: [
        "part.hull",
        "part.sideGear.top.fill",
        "part.sideGear.bottom.fill",
        "part.cabin",
        "part.darkNose",
        "part.darkSlot.top",
        "part.darkSlot.bottom",
        "part.windshield",
        "part.noseTick",
        "part.badge.top",
        "part.badge.bottom",
      ],
      tintSlot: "fixed",
      drawOrder: 20,
      frame: {
        x: 0,
        y: 0,
        w: 1623,
        h: 842,
        originX: 811.5,
        originY: 421,
        pixelsPerUnitX: 37.67,
        pixelsPerUnitY: 37.67,
      },
    },
    {
      id: "sprite.paint",
      animationPart: "part.hull",
      sourceParts: ["part.hull"],
      tintSlot: "team-light",
      drawOrder: 21,
      frame: {
        x: 0,
        y: 842,
        w: 1623,
        h: 842,
        originX: 811.5,
        originY: 421,
        pixelsPerUnitX: 37.67,
        pixelsPerUnitY: 37.67,
      },
    },
  ],
});
