// Prototype wheeled mortar PNG atlas. The SVG rig remains the source of
// anchors, setup visibility, facing, and recoil bindings; this atlas replaces
// the visible weapon pixels with generated carriage and tube component sprites.
function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

const PPU = 17.8;
const TEAM_TINT_ADJUSTMENT = Object.freeze({ brightness: 78, saturation: 92 });

const WHEEL_NAMES = Object.freeze([
  "wheel.left.body",
  "wheel.left.tread.0",
  "wheel.left.tread.1",
  "wheel.left.cross.0",
  "wheel.left.cross.1",
  "wheel.left.cross.2",
  "wheel.left.hub",
  "wheel.right.body",
  "wheel.right.tread.0",
  "wheel.right.tread.1",
  "wheel.right.cross.0",
  "wheel.right.cross.1",
  "wheel.right.cross.2",
  "wheel.right.hub",
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
  x: 74,
  y: 164,
  w: 612,
  h: 391,
});

const CARRIAGE_FRAME = Object.freeze({
  x: 870,
  y: 164,
  w: 413,
  h: 391,
  originX: 351,
  originY: 195.5,
  pixelsPerUnitX: PPU,
  pixelsPerUnitY: PPU,
});

const TUBE_FRAME = Object.freeze({
  x: 1613,
  y: 292,
  w: 490,
  h: 132,
  originX: 130,
  originY: 66,
  pixelsPerUnitX: PPU,
  pixelsPerUnitY: PPU,
});

function carriageCropFrame(x, y, w, h) {
  return Object.freeze({
    x,
    y,
    w,
    h,
    originX: CARRIAGE_FRAME.originX - (x - CARRIAGE_FRAME.x),
    originY: CARRIAGE_FRAME.originY - (y - CARRIAGE_FRAME.y),
    pixelsPerUnitX: PPU,
    pixelsPerUnitY: PPU,
  });
}

const TOP_TIRE_FRAME = carriageCropFrame(1072, 164, 146, 52);
const BOTTOM_TIRE_FRAME = carriageCropFrame(1072, 472, 146, 56);

export const MORTAR_TEAM_PNG_RIG_ATLAS = deepFreeze({
  enabled: true,
  unit: "mortar_team",
  image: "/assets/rigs/mortar-png-pass-01/generated/mortar-m2-wheeled-pass-01-alpha.png?v=m2-wheeled-pass01-long-thin-tube",
  grid: {
    profile: "three-cell-components",
    sourceSheet: "client/assets/rigs/mortar-png-pass-01/generated/mortar-m2-wheeled-pass-01-alpha.png",
    generatedSource: "client/assets/rigs/mortar-png-pass-01/generated/mortar-m2-wheeled-pass-01-source.png",
    imageVersion: "m2-wheeled-pass01-long-thin-tube",
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
      { tintSlot: "team-light", tintAdjustment: TEAM_TINT_ADJUSTMENT }
    ),
    sprite(
      "sprite.mortar.tire.top.packed",
      mortarPart("axle", "packed"),
      mortarParts(WHEEL_NAMES, "packed"),
      22,
      TOP_TIRE_FRAME
    ),
    sprite(
      "sprite.mortar.tire.bottom.packed",
      mortarPart("axle", "packed"),
      mortarParts(WHEEL_NAMES, "packed"),
      23,
      BOTTOM_TIRE_FRAME
    ),
    sprite(
      "sprite.mortar.carriage.deployed",
      mortarPart("axle", "deployed"),
      mortarParts(CARRIAGE_NAMES, "deployed"),
      21,
      CARRIAGE_FRAME,
      { tintSlot: "team-light", tintAdjustment: TEAM_TINT_ADJUSTMENT }
    ),
    sprite(
      "sprite.mortar.tire.top.deployed",
      mortarPart("axle", "deployed"),
      mortarParts(WHEEL_NAMES, "deployed"),
      24,
      TOP_TIRE_FRAME
    ),
    sprite(
      "sprite.mortar.tire.bottom.deployed",
      mortarPart("axle", "deployed"),
      mortarParts(WHEEL_NAMES, "deployed"),
      25,
      BOTTOM_TIRE_FRAME
    ),
    sprite(
      "sprite.mortar.tube.packed",
      mortarPart("tube", "packed"),
      mortarParts(TUBE_NAMES, "packed"),
      30,
      TUBE_FRAME,
      { tintSlot: "team-light", tintAdjustment: TEAM_TINT_ADJUSTMENT }
    ),
    sprite(
      "sprite.mortar.tube.deployed",
      mortarPart("tube", "deployed"),
      mortarParts(TUBE_NAMES, "deployed"),
      31,
      TUBE_FRAME,
      { tintSlot: "team-light", tintAdjustment: TEAM_TINT_ADJUSTMENT }
    ),
  ],
});
