// Prototype modular A-19 artillery PNG atlas. The SVG rig remains the source
// of setup visibility, carriage/weapon facing, recoil, muzzle flash, and
// anchors. This atlas replaces only the visible carriage, trails, and weapon
// assembly pixels. Both trails use the same owner-team tint as the carriage.
function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

const CARRIAGE_PPU_X = 14.22;
const CARRIAGE_PPU_Y = 13.75;
const TRAIL_PPU = 15;
const PACKED_TRAIL_PPU = 30;
const BARREL_PPU_X = 11.9;
const BARREL_PPU_Y = 11.9;
const TRAIL_ROOT_X = -3.024;
const TRAIL_BACK_SHIFT_RATIO = 0.5;
const TRAIL_LATERAL_SHIFT_RATIO = 0.1;
const DEPLOYED_TRAIL_ROTATION = 0.54;
const PACKED_TRAIL_ROTATION = 0.22;
const TEAM_TINT_ADJUSTMENT = Object.freeze({ brightness: 82, saturation: 90 });

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
]);

const BARREL_ASSEMBLY_NAMES = Object.freeze([
  "cradleLink",
  "cradle",
  "breechBase",
  "barrel",
  "barrelHighlight",
  "breech",
  "breechShield",
]);

function artilleryPart(name, suffix) {
  return `part.art.${name}.${suffix}`;
}

function artilleryParts(names, suffix) {
  return names.map((name) => artilleryPart(name, suffix));
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
    positionOffsetX: options.positionOffsetX ?? 0,
    positionOffsetY: options.positionOffsetY ?? 0,
    tintAdjustment: options.tintAdjustment ?? null,
  };
}

const LEFT_TRAIL_FRAME = Object.freeze({
  x: 88,
  y: 195,
  w: 541,
  h: 217,
  originX: 491,
  originY: 118,
  pixelsPerUnitX: TRAIL_PPU,
  pixelsPerUnitY: TRAIL_PPU,
});

const RIGHT_TRAIL_FRAME = Object.freeze({
  x: 87,
  y: 602,
  w: 541,
  h: 216,
  originX: 491,
  originY: 111,
  pixelsPerUnitX: TRAIL_PPU,
  pixelsPerUnitY: TRAIL_PPU,
});

const LEFT_TRAIL_PACKED_FRAME = Object.freeze({
  ...LEFT_TRAIL_FRAME,
  pixelsPerUnitX: PACKED_TRAIL_PPU,
  pixelsPerUnitY: PACKED_TRAIL_PPU,
});

const RIGHT_TRAIL_PACKED_FRAME = Object.freeze({
  ...RIGHT_TRAIL_FRAME,
  pixelsPerUnitX: PACKED_TRAIL_PPU,
  pixelsPerUnitY: PACKED_TRAIL_PPU,
});

const CARRIAGE_FRAME = Object.freeze({
  x: 751,
  y: 112,
  w: 485,
  h: 465,
  originX: 239,
  originY: 238,
  pixelsPerUnitX: CARRIAGE_PPU_X,
  pixelsPerUnitY: CARRIAGE_PPU_Y,
});

const BARREL_ASSEMBLY_FRAME = Object.freeze({
  x: 664,
  y: 636,
  w: 835,
  h: 244,
  originX: 291,
  originY: 123,
  pixelsPerUnitX: BARREL_PPU_X,
  pixelsPerUnitY: BARREL_PPU_Y,
});

function trailSprite(side, suffix, frame, rotationOffset) {
  const deployed = suffix === "deployed";
  const names = deployed ? [`trail.${side}`, `foot.${side}`] : [`trail.${side}`];
  const trailLength = frame.w / Math.abs(frame.pixelsPerUnitX);
  const trailWidth = frame.h / Math.abs(frame.pixelsPerUnitY);
  const lateralDirection = side === "left" ? -1 : 1;
  return sprite(
    `sprite.art.${side}Trail.${suffix}`,
    artilleryPart(`trail.${side}`, suffix),
    artilleryParts(names, suffix),
    side === "left" ? 15 : 16,
    frame,
    {
      tintSlot: "team-light",
      tintAdjustment: TEAM_TINT_ADJUSTMENT,
      positionOffsetX: TRAIL_ROOT_X - trailLength * TRAIL_BACK_SHIFT_RATIO,
      positionOffsetY: lateralDirection * trailWidth * TRAIL_LATERAL_SHIFT_RATIO,
      rotationOffset,
    },
  );
}

export const ARTILLERY_PNG_RIG_ATLAS = deepFreeze({
  enabled: true,
  unit: "artillery",
  image: "/assets/rigs/artillery-a19-pass-03/generated/artillery-a19-components-pass-03-alpha.png?v=a19-pass03-d485-spaced-team-tint",
  grid: {
    profile: "semantic-components",
    sourceSheet: "client/assets/rigs/artillery-a19-pass-03/generated/artillery-a19-components-pass-03-alpha.png",
    generatedSource: "client/assets/rigs/artillery-a19-pass-03/generated/artillery-a19-components-pass-03-source.png",
    imageVersion: "a19-pass03-d485-spaced-team-tint",
    components: {
      leftTrail: LEFT_TRAIL_FRAME,
      rightTrail: RIGHT_TRAIL_FRAME,
      carriage: CARRIAGE_FRAME,
      barrelAssembly: BARREL_ASSEMBLY_FRAME,
    },
  },
  frames: {},
  sprites: [
    trailSprite("left", "packed", LEFT_TRAIL_PACKED_FRAME, PACKED_TRAIL_ROTATION),
    trailSprite("right", "packed", RIGHT_TRAIL_PACKED_FRAME, -PACKED_TRAIL_ROTATION),
    trailSprite("left", "deployed", LEFT_TRAIL_FRAME, DEPLOYED_TRAIL_ROTATION),
    trailSprite("right", "deployed", RIGHT_TRAIL_FRAME, -DEPLOYED_TRAIL_ROTATION),
    sprite(
      "sprite.art.carriage.packed",
      artilleryPart("axle", "packed"),
      artilleryParts(CARRIAGE_NAMES, "packed"),
      20,
      CARRIAGE_FRAME,
      { tintSlot: "team-light", tintAdjustment: TEAM_TINT_ADJUSTMENT },
    ),
    sprite(
      "sprite.art.carriage.deployed",
      artilleryPart("axle", "deployed"),
      artilleryParts(CARRIAGE_NAMES, "deployed"),
      21,
      CARRIAGE_FRAME,
      { tintSlot: "team-light", tintAdjustment: TEAM_TINT_ADJUSTMENT },
    ),
    sprite(
      "sprite.art.barrelAssembly.packed",
      artilleryPart("barrel", "packed"),
      artilleryParts(BARREL_ASSEMBLY_NAMES, "packed"),
      30,
      BARREL_ASSEMBLY_FRAME,
      { tintSlot: "team-light-soft", tintAdjustment: TEAM_TINT_ADJUSTMENT },
    ),
    sprite(
      "sprite.art.barrelAssembly.deployed",
      artilleryPart("barrel", "deployed"),
      artilleryParts(BARREL_ASSEMBLY_NAMES, "deployed"),
      31,
      BARREL_ASSEMBLY_FRAME,
      { tintSlot: "team-light-soft", tintAdjustment: TEAM_TINT_ADJUSTMENT },
    ),
  ],
});
