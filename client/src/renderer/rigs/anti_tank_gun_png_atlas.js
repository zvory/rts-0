// Prototype no-shield anti-tank gun PNG atlas. The SVG rig remains the source
// of anchors, setup visibility, facing, and recoil bindings; this atlas only
// replaces the visible support-gun pixels with generated component sprites.
function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

const PPU = 17.4;
const TRAIL_BACK_OFFSET = -17.1;
const TRAIL_LATERAL_OFFSET = 17;
const TRAIL_ROTATION = Math.PI / 9;
const TRAIL_CENTER_COUNTER_ROTATION = (Math.PI * 5 / 18) - TRAIL_ROTATION;
const TEAM_TINT_ADJUSTMENT = Object.freeze({ brightness: 70, saturation: 90 });

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

const CARRIAGE_BASE_NAMES = Object.freeze([
  "axle",
  ...WHEEL_NAMES,
  // Covered here to suppress the old shield SVG parts for this no-shield pass.
  "shield",
  "shieldStripe",
]);

const CARRIAGE_PACKED_NAMES = Object.freeze([
  ...CARRIAGE_BASE_NAMES,
  "trail.left",
  "trail.right",
]);

const BARREL_ASSEMBLY_NAMES = Object.freeze([
  "barrel",
  "barrelHighlight",
  "muzzleTick",
  "breech",
]);

function atPart(name, suffix) {
  return `part.at.${name}.${suffix}`;
}

function atParts(names, suffix) {
  return names.map((name) => atPart(name, suffix));
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

const CARRIAGE_FRAME = Object.freeze({
  x: 79,
  y: 179,
  w: 349,
  h: 481,
  originX: 314,
  originY: 242,
  pixelsPerUnitX: PPU,
  pixelsPerUnitY: PPU,
});

const BARREL_ASSEMBLY_FRAME = Object.freeze({
  x: 521,
  y: 314,
  w: 679,
  h: 205,
  originX: 286,
  originY: 113,
  pixelsPerUnitX: PPU,
  pixelsPerUnitY: PPU,
});

const LEFT_TRAIL_FRAME = Object.freeze({
  x: 62,
  y: 796,
  w: 528,
  h: 206,
  originX: 500,
  originY: 142,
  pixelsPerUnitX: PPU,
  pixelsPerUnitY: PPU,
});

const RIGHT_TRAIL_FRAME = Object.freeze({
  x: 662,
  y: 793,
  w: 530,
  h: 209,
  originX: 30,
  originY: 145,
  pixelsPerUnitX: -PPU,
  pixelsPerUnitY: -PPU,
});

export const ANTI_TANK_GUN_PNG_RIG_ATLAS = deepFreeze({
  enabled: true,
  unit: "anti_tank_gun",
  image: "/assets/rigs/anti-tank-gun-noshield-lowdetail/anti-tank-gun-noshield-lowdetail-white-v1-alpha.png?v=white-v1-br70-sat90",
  grid: {
    profile: "semantic-components",
    sourceSheet: "client/assets/rigs/anti-tank-gun-noshield-lowdetail/anti-tank-gun-noshield-lowdetail-white-v1-alpha.png",
    generatedSource: "client/assets/rigs/anti-tank-gun-noshield-lowdetail/generated/anti-tank-gun-noshield-lowdetail-white-v1-source.png",
    imageVersion: "white-v1",
    components: {
      carriage: CARRIAGE_FRAME,
      barrelAssembly: BARREL_ASSEMBLY_FRAME,
      leftTrail: LEFT_TRAIL_FRAME,
      rightTrail: RIGHT_TRAIL_FRAME,
    },
  },
  frames: {},
  sprites: [
    sprite(
      "sprite.at.leftTrail.deployed",
      atPart("trail.left", "deployed"),
      [
        atPart("trail.left", "deployed"),
        atPart("brace.left", "deployed"),
      ],
      15,
      RIGHT_TRAIL_FRAME,
      {
        tintSlot: "team-light",
        tintAdjustment: TEAM_TINT_ADJUSTMENT,
        positionOffsetX: TRAIL_BACK_OFFSET,
        positionOffsetY: -TRAIL_LATERAL_OFFSET,
        rotationOffset: TRAIL_CENTER_COUNTER_ROTATION,
        rotationPivotX: RIGHT_TRAIL_FRAME.w * 0.5,
        rotationPivotY: RIGHT_TRAIL_FRAME.h * 0.5,
        rotationPivotReferenceOffset: -TRAIL_ROTATION,
      }
    ),
    sprite(
      "sprite.at.rightTrail.deployed",
      atPart("trail.right", "deployed"),
      [
        atPart("trail.right", "deployed"),
        atPart("brace.right", "deployed"),
      ],
      16,
      LEFT_TRAIL_FRAME,
      {
        tintSlot: "team-light",
        tintAdjustment: TEAM_TINT_ADJUSTMENT,
        positionOffsetX: TRAIL_BACK_OFFSET,
        positionOffsetY: TRAIL_LATERAL_OFFSET,
        rotationOffset: -TRAIL_CENTER_COUNTER_ROTATION,
        rotationPivotX: LEFT_TRAIL_FRAME.w * 0.5,
        rotationPivotY: LEFT_TRAIL_FRAME.h * 0.5,
        rotationPivotReferenceOffset: TRAIL_ROTATION,
      }
    ),
    sprite(
      "sprite.at.carriage.packed",
      atPart("axle", "packed"),
      atParts(CARRIAGE_PACKED_NAMES, "packed"),
      20,
      CARRIAGE_FRAME,
      { tintSlot: "team-light", tintAdjustment: TEAM_TINT_ADJUSTMENT }
    ),
    sprite(
      "sprite.at.carriage.deployed",
      atPart("axle", "deployed"),
      atParts(CARRIAGE_BASE_NAMES, "deployed"),
      21,
      CARRIAGE_FRAME,
      { tintSlot: "team-light", tintAdjustment: TEAM_TINT_ADJUSTMENT }
    ),
    sprite(
      "sprite.at.barrelAssembly.packed",
      atPart("barrel", "packed"),
      atParts(BARREL_ASSEMBLY_NAMES, "packed"),
      30,
      BARREL_ASSEMBLY_FRAME,
      { tintSlot: "team-light-soft", tintAdjustment: TEAM_TINT_ADJUSTMENT }
    ),
    sprite(
      "sprite.at.barrelAssembly.deployed",
      atPart("barrel", "deployed"),
      atParts(BARREL_ASSEMBLY_NAMES, "deployed"),
      31,
      BARREL_ASSEMBLY_FRAME,
      { tintSlot: "team-light-soft", tintAdjustment: TEAM_TINT_ADJUSTMENT }
    ),
  ],
});
