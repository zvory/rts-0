// Production wheeled mortar PNG atlas. Raster-native metadata supplies anchors,
// setup visibility, facing, and recoil bindings.
import { PRODUCTION_RASTER_COLOR_TARGET } from "./color_adjustment.js";

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

const PPU = 17.8;

const WHEEL_LEFT_NAMES = Object.freeze([
  "wheel.left.body",
  "wheel.left.tread.0",
  "wheel.left.tread.1",
  "wheel.left.cross.0",
  "wheel.left.cross.1",
  "wheel.left.cross.2",
  "wheel.left.hub",
]);

const WHEEL_RIGHT_NAMES = Object.freeze([
  "wheel.right.body",
  "wheel.right.tread.0",
  "wheel.right.tread.1",
  "wheel.right.cross.0",
  "wheel.right.cross.1",
  "wheel.right.cross.2",
  "wheel.right.hub",
]);

const WHEEL_NAMES = Object.freeze([
  ...WHEEL_LEFT_NAMES,
  ...WHEEL_RIGHT_NAMES,
]);

const CARRIAGE_NAMES = Object.freeze([
  "axle",
  ...WHEEL_NAMES,
  "trail",
  "leg.left.dark",
  "leg.right.dark",
  "leg.left",
  "leg.right",
  "base",
  "body",
]);

const TUBE_NAMES = Object.freeze([
  "tube",
  "tubeHighlight",
  "muzzle",
]);

function mortarPart(name, suffix) {
  return `part.mortar.${name}.${suffix}`;
}

function mortarParts(names, suffix) {
  return names.map((name) => mortarPart(name, suffix));
}

function sprite(id, animationPart, sourceParts, drawOrder, frame, options = {}) {
  return {
    id,
    animationPart,
    sourceParts,
    tintSlot: options.tintSlot ?? "fixed",
    drawOrder,
    frame,
    rotationOffset: options.rotationOffset ?? 0,
    rotationPivotX: options.rotationPivotX ?? null,
    rotationPivotY: options.rotationPivotY ?? null,
    rotationPivotReferenceOffset: options.rotationPivotReferenceOffset ?? 0,
    positionOffsetX: options.positionOffsetX ?? 0,
    positionOffsetY: options.positionOffsetY ?? 0,
    tintAdjustment: options.tintAdjustment ?? null,
  };
}

const ASSEMBLED_REFERENCE_FRAME = Object.freeze({
  x: 24,
  y: 78,
  w: 334,
  h: 210,
});

const CARRIAGE_FRAME = Object.freeze({
  x: 431,
  y: 79,
  w: 225,
  h: 207,
  originX: 179.5,
  originY: 104.5,
  pixelsPerUnitX: PPU / 2,
  pixelsPerUnitY: PPU / 2,
});

const TUBE_FRAME = Object.freeze({
  x: 804,
  y: 142,
  w: 257,
  h: 83,
  originX: 67.5,
  originY: 41.5,
  pixelsPerUnitX: PPU / 2,
  pixelsPerUnitY: PPU / 2,
});

function carriageCropFrame(x, y, w, h) {
  return Object.freeze({
    x,
    y,
    w,
    h,
    originX: CARRIAGE_FRAME.originX - (x - CARRIAGE_FRAME.x),
    originY: CARRIAGE_FRAME.originY - (y - CARRIAGE_FRAME.y),
    pixelsPerUnitX: CARRIAGE_FRAME.pixelsPerUnitX,
    pixelsPerUnitY: CARRIAGE_FRAME.pixelsPerUnitY,
  });
}

const LEFT_TIRE_FRAME = carriageCropFrame(568, 80, 87, 35);
const RIGHT_TIRE_FRAME = carriageCropFrame(568, 247, 87, 38);

export const MORTAR_TEAM_PNG_RIG_ATLAS = deepFreeze({
  enabled: true,
  unit: "mortar_team",
  image: "/assets/rigs/kriegsia-team-color/generated/mortar_team-atlas.png?v=team-paint-1",
  runtimeColorAdjustment: PRODUCTION_RASTER_COLOR_TARGET,
  sheetRows: 4,
  teamPaint: { baseRow: 2, paintRow: 3, renderRows: 1 },
  iconVisibleBounds: {
    x: 24,
    y: 78,
    w: 334,
    h: 210,
  },
  grid: {
    profile: "four-component-cells",
    width: 1150,
    height: 362,
    sourceSheet: "client/assets/rigs/mortar-bulky-white/generated/mortar-bulky-white-atlas.png",
    generatedSource: "client/assets/rigs/mortar-bulky-white/generated/mortar-bulky-white-source.png",
    imageVersion: "team-paint-1",
    cells: [
      "reference.assembled",
      "sprite.carriage",
      "sprite.tube",
    ],
    components: {
      assembledReference: ASSEMBLED_REFERENCE_FRAME,
      carriage: CARRIAGE_FRAME,
      tube: TUBE_FRAME,
    },
  },
  frames: {},
  sprites: [
    sprite(
      "sprite.mortar.carriage.packed",
      mortarPart("axle", "packed"),
      mortarParts(CARRIAGE_NAMES, "packed"),
      20,
      CARRIAGE_FRAME,
      { tintSlot: "fixed" }
    ),
    sprite(
      "sprite.mortar.tire.left.packed",
      mortarPart("axle", "packed"),
      mortarParts(WHEEL_LEFT_NAMES, "packed"),
      22,
      LEFT_TIRE_FRAME
    ),
    sprite(
      "sprite.mortar.tire.right.packed",
      mortarPart("axle", "packed"),
      mortarParts(WHEEL_RIGHT_NAMES, "packed"),
      23,
      RIGHT_TIRE_FRAME
    ),
    sprite(
      "sprite.mortar.carriage.deployed",
      mortarPart("axle", "deployed"),
      mortarParts(CARRIAGE_NAMES, "deployed"),
      21,
      CARRIAGE_FRAME,
      { tintSlot: "fixed" }
    ),
    sprite(
      "sprite.mortar.tire.left.deployed",
      mortarPart("axle", "deployed"),
      mortarParts(WHEEL_LEFT_NAMES, "deployed"),
      24,
      LEFT_TIRE_FRAME
    ),
    sprite(
      "sprite.mortar.tire.right.deployed",
      mortarPart("axle", "deployed"),
      mortarParts(WHEEL_RIGHT_NAMES, "deployed"),
      25,
      RIGHT_TIRE_FRAME
    ),
    sprite(
      "sprite.mortar.tube.packed",
      mortarPart("tube", "packed"),
      mortarParts(TUBE_NAMES, "packed"),
      30,
      TUBE_FRAME,
      { tintSlot: "fixed" }
    ),
    sprite(
      "sprite.mortar.tube.deployed",
      mortarPart("tube", "deployed"),
      mortarParts(TUBE_NAMES, "deployed"),
      31,
      TUBE_FRAME,
      { tintSlot: "fixed" }
    ),
  ],
});
