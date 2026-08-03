import { PRODUCTION_RASTER_COLOR_TARGET } from "./color_adjustment.js";

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

const GUN_RENDER_SCALE_X = 1.1;
const GUN_RENDER_SCALE_Y = 1.32;
const BODY_FRAME_BASE = Object.freeze({
  w: 656,
  h: 339,
  originX: 328,
  originY: 169.5,
  pixelsPerUnitX: 15.070270270270269,
  pixelsPerUnitY: 15.299999999999997,
});
const GUN_FRAME_BASE = Object.freeze({
  w: 538,
  h: 165,
  originX: 190.31292517006804,
  originY: 77.9746835443038,
  pixelsPerUnitX: 23.42312925170068 / GUN_RENDER_SCALE_X,
  pixelsPerUnitY: 22.278481012658226 / GUN_RENDER_SCALE_Y,
});
const GUN_X_OFFSET = Math.floor((BODY_FRAME_BASE.w - GUN_FRAME_BASE.w) / 2);
const BODY_FRAME = Object.freeze({ x: 0, y: 0, ...BODY_FRAME_BASE });
const GUN_NEUTRAL_FRAME = Object.freeze({
  x: GUN_X_OFFSET,
  y: BODY_FRAME_BASE.h,
  ...GUN_FRAME_BASE,
});

export const SCOUT_CAR_PNG_RIG_ATLAS = deepFreeze({
  enabled: true,
  unit: "scout_car",
  image: "/assets/rigs/scout-car-white-pass-01/generated/scout-car-white-atlas.png?v=white-runtime-tint-pass01",
  iconVisibleBounds: {
    x: 4,
    y: 1,
    w: 650,
    h: 339,
  },
  runtimeColorAdjustment: PRODUCTION_RASTER_COLOR_TARGET,
  viewBox: {
    x: -40,
    y: -32,
    width: 80,
    height: 64,
  },
  grid: {
    layout: "semantic",
    width: 656,
    height: 504,
    sourceSheet: "client/assets/rigs/scout-car-white-pass-01/generated/scout-car-white-alpha.png",
    cells: [
      "sprite.body",
      "sprite.rearMachineGun",
    ],
    imageVersion: "white-runtime-tint-pass01",
  },
  frames: {},
  sprites: [
    {
      id: "sprite.body",
      animationPart: "part.hull",
      sourceParts: [
        "part.hull",
        "part.sideGear.top.fill",
        "part.sideGear.bottom.fill",
        "part.cabin",
        "part.nose",
        "part.darkNose",
        "part.darkSlot.top",
        "part.darkSlot.bottom",
        "part.hoodLine",
        "part.noseTick",
      ],
      tintSlot: "team-light",
      drawOrder: 20,
      frame: BODY_FRAME,
    },
    {
      id: "sprite.rearMachineGun",
      animationPart: "part.gunnerBarrel",
      sourceParts: [
        "part.mount",
        "part.gunnerTorso",
        "part.gunnerHead",
        "part.gunnerHand.left",
        "part.gunnerHand.right",
        "part.gunnerBarrel",
        "part.gunnerReceiver",
        "part.gunnerShroud",
      ],
      tintSlot: "fixed",
      drawOrder: 40,
      frame: GUN_NEUTRAL_FRAME,
    },
  ],
});
