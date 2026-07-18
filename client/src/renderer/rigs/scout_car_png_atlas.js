function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

const TEAM_FRAME_COLORS = Object.freeze([
  "#0072b2",
  "#d55e00",
  "#009e73",
  "#cc79a7",
  "#56b4e9",
  "#e69f00",
  "#f0e442",
  "#7e57c2",
]);

const ATLAS_PADDING = 1;
const ATLAS_COLUMNS = 2;
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
const TEAM_CELL_WIDTH = BODY_FRAME_BASE.w;
const TEAM_CELL_HEIGHT = BODY_FRAME_BASE.h + GUN_FRAME_BASE.h;
const GUN_X_OFFSET = Math.floor((BODY_FRAME_BASE.w - GUN_FRAME_BASE.w) / 2);

function paletteFrame(color, part) {
  const index = TEAM_FRAME_COLORS.indexOf(color);
  const col = index % ATLAS_COLUMNS;
  const row = Math.floor(index / ATLAS_COLUMNS);
  const cellX = ATLAS_PADDING + col * (TEAM_CELL_WIDTH + ATLAS_PADDING);
  const cellY = ATLAS_PADDING + row * (TEAM_CELL_HEIGHT + ATLAS_PADDING);
  if (part === "body") {
    return {
      x: cellX,
      y: cellY,
      ...BODY_FRAME_BASE,
    };
  }
  return {
    x: cellX + GUN_X_OFFSET,
    y: cellY + BODY_FRAME_BASE.h,
    ...GUN_FRAME_BASE,
  };
}

function paletteFrames(part) {
  const frames = {};
  for (const color of TEAM_FRAME_COLORS) frames[color] = paletteFrame(color, part);
  return frames;
}

const BODY_PALETTE_FRAMES = paletteFrames("body");
const GUN_NEUTRAL_FRAME = paletteFrame("#0072b2", "rearMachineGun");

export const SCOUT_CAR_PNG_RIG_ATLAS = deepFreeze({
  enabled: true,
  unit: "scout_car",
  image: "/assets/rigs/scout-car-pass-02-team/generated/scout-car-pass-02-team-atlas.png?v=pass02-team-halfres-rgba8",
  runtimeColorAdjustment: {
    brightness: 90,
    saturation: 90,
    hue: 100,
  },
  viewBox: {
    x: -40,
    y: -32,
    width: 80,
    height: 64,
  },
  grid: {
    layout: "semantic",
    width: 1315,
    height: 2021,
    sourceSheet: "client/assets/rigs/scout-car-pass-02-team/generated/scout-car-pass-02-team-atlas.png",
    cells: [
      "sprite.body",
      "sprite.rearMachineGun",
    ],
    palette: TEAM_FRAME_COLORS,
    imageVersion: "pass02-team-halfres-rgba8",
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
      tintSlot: "fixed",
      drawOrder: 20,
      frame: BODY_PALETTE_FRAMES["#0072b2"],
      paletteFrames: BODY_PALETTE_FRAMES,
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
