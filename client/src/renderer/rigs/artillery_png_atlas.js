// Prototype modular A-19 artillery PNG atlas. The SVG rig remains the source
// of setup visibility, carriage/weapon facing, recoil, muzzle flash, and
// anchors. This atlas replaces only the visible carriage, trails, and weapon
// assembly pixels. The support-arm borders and purple left-arm tint are
// intentional alignment-review diagnostics.
function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

const CARRIAGE_PPU = 13.75;
const TRAIL_PPU = 15;
const PACKED_TRAIL_PPU = 30;
const BARREL_PPU = 11.28;
const TRAIL_ROOT_X = -3.024;
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
  x: 112,
  y: 244,
  w: 539,
  h: 203,
  originX: 486,
  originY: 81,
  pixelsPerUnitX: TRAIL_PPU,
  pixelsPerUnitY: TRAIL_PPU,
});

const RIGHT_TRAIL_FRAME = Object.freeze({
  x: 112,
  y: 590,
  w: 539,
  h: 205,
  originX: 486,
  originY: 82,
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
  x: 743,
  y: 134,
  w: 440,
  h: 437,
  originX: 247,
  originY: 216,
  pixelsPerUnitX: CARRIAGE_PPU,
  pixelsPerUnitY: CARRIAGE_PPU,
});

const BARREL_ASSEMBLY_FRAME = Object.freeze({
  x: 687,
  y: 639,
  w: 794,
  h: 213,
  originX: 254,
  originY: 120,
  pixelsPerUnitX: BARREL_PPU,
  pixelsPerUnitY: BARREL_PPU,
});

function trailSprite(side, suffix, frame, rotationOffset, tintSlot) {
  const deployed = suffix === "deployed";
  const names = deployed ? [`trail.${side}`, `foot.${side}`] : [`trail.${side}`];
  return sprite(
    `sprite.art.${side}Trail.${suffix}`,
    artilleryPart(`trail.${side}`, suffix),
    artilleryParts(names, suffix),
    side === "left" ? 15 : 16,
    frame,
    {
      tintSlot,
      positionOffsetX: TRAIL_ROOT_X,
      rotationOffset,
    },
  );
}

export const ARTILLERY_PNG_RIG_ATLAS = deepFreeze({
  enabled: true,
  unit: "artillery",
  image: "/assets/rigs/artillery-a19-pass-01/generated/artillery-a19-components-pass-01-alpha-debug.png?v=a19-pass01-purple-left-frame-debug",
  grid: {
    profile: "semantic-components-debug",
    sourceSheet: "client/assets/rigs/artillery-a19-pass-01/generated/artillery-a19-components-pass-01-alpha-debug.png",
    generatedSource: "client/assets/rigs/artillery-a19-pass-01/generated/artillery-a19-components-pass-01-source.png",
    imageVersion: "a19-pass01-purple-left-frame-debug",
    diagnostics: {
      leftTrailTint: "#a05cff",
      trailFrameStroke: "#000000",
    },
    components: {
      leftTrail: LEFT_TRAIL_FRAME,
      rightTrail: RIGHT_TRAIL_FRAME,
      carriage: CARRIAGE_FRAME,
      barrelAssembly: BARREL_ASSEMBLY_FRAME,
    },
  },
  frames: {},
  sprites: [
    trailSprite("left", "packed", LEFT_TRAIL_PACKED_FRAME, PACKED_TRAIL_ROTATION, "#a05cff"),
    trailSprite("right", "packed", RIGHT_TRAIL_PACKED_FRAME, -PACKED_TRAIL_ROTATION, "fixed"),
    trailSprite("left", "deployed", LEFT_TRAIL_FRAME, DEPLOYED_TRAIL_ROTATION, "#a05cff"),
    trailSprite("right", "deployed", RIGHT_TRAIL_FRAME, -DEPLOYED_TRAIL_ROTATION, "fixed"),
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
