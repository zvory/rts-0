import { KIND } from "../../protocol.js";
import { ANTI_TANK_GUN_PNG_RIG_ATLAS } from "./anti_tank_gun_png_atlas.js";
import { ARTILLERY_PNG_RIG_ATLAS } from "./artillery_png_atlas.js";
import { MORTAR_TEAM_PNG_RIG_ATLAS } from "./mortar_team_png_atlas.js";
import { SCOUT_CAR_PNG_RIG_ATLAS } from "./scout_car_png_atlas.js";
import { TANK_PNG_RIG_ATLAS } from "./tank_png_atlas.js";

export const LOADED_RIFLEMAN_RIG_KEY = "rifleman.panzerfaustLoaded";

const IDENTITY_TRANSFORM = Object.freeze({ x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1 });
const ZERO_POINT = Object.freeze({ x: 0, y: 0 });
const INVISIBLE_PAINT = Object.freeze({
  fill: null,
  stroke: null,
  strokeWidth: null,
  opacity: 1,
  fillOpacity: 1,
  strokeOpacity: 1,
});
const PLACEHOLDER_GEOMETRY = Object.freeze({ type: "rect", x: 0, y: 0, width: 1, height: 1 });

function deepFreeze(value) {
  if (!value || typeof value !== "object" || Object.isFrozen(value)) return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

function part(id, drawOrder, options = {}) {
  return {
    id,
    drawOrder,
    geometry: options.geometry || PLACEHOLDER_GEOMETRY,
    transform: options.transform || IDENTITY_TRANSFORM,
    pivot: options.pivot || ZERO_POINT,
    tintSlot: options.tintSlot || "fixed",
    paint: options.paint || INVISIBLE_PAINT,
  };
}

function paint({ fill = null, stroke = null, strokeWidth = null, opacity = 1, fillOpacity = 1, strokeOpacity = 1 } = {}) {
  return { fill, stroke, strokeWidth, opacity, fillOpacity, strokeOpacity };
}

function ellipseShadow(id, cx, cy, rx, ry, opacity = 0.28) {
  return part(id, 0, {
    geometry: { type: "ellipse", cx, cy, rx, ry },
    paint: paint({ fill: "#000000", opacity }),
  });
}

function polygonShadow(id, rx, ry, drop, digits = 6) {
  const points = [];
  for (let index = 0; index < 24; index += 1) {
    const angle = Math.PI * 2 * index / 24;
    points.push({
      x: rounded(Math.cos(angle) * rx, digits),
      y: rounded(Math.sin(angle) * ry, digits),
    });
  }
  return part(id, 0, {
    geometry: { type: "polygon", points },
    transform: { ...IDENTITY_TRANSFORM, y: drop },
    paint: paint({ fill: "#000000", opacity: 0.28 }),
  });
}

function rounded(value, digits) {
  const result = Number(value.toFixed(digits));
  return Object.is(result, -0) ? 0 : result;
}

function binding(partId, input, property, factor = 1, offset = 0) {
  return { partId, input, property, factor, offset };
}

function bindingsFor(parts, specs) {
  return parts.flatMap((partId) => specs.map(([input, property, factor = 1, offset = 0]) => (
    binding(partId, input, property, factor, offset)
  )));
}

function animatedRasterParts(atlas, optionsByPart = {}) {
  const parts = new Map();
  for (const sprite of atlas.sprites || []) {
    if (!parts.has(sprite.animationPart)) {
      parts.set(sprite.animationPart, part(
        sprite.animationPart,
        sprite.drawOrder || 0,
        optionsByPart[sprite.animationPart],
      ));
    }
  }
  return [...parts.values()];
}

function atlasSourceParts(atlas) {
  return Object.freeze([...new Set((atlas.sprites || []).flatMap((sprite) => sprite.sourceParts || []))]);
}

function definition({ id, kind, parts, anchors, bounds, animations }) {
  return deepFreeze({
    id,
    kind,
    schemaVersion: 1,
    parts,
    anchors,
    bounds,
    animations,
    requiredRuntimeInputs: [...new Set(animations.map((entry) => entry.input))],
  });
}

const SHADOW_FACING = Object.freeze([["facing", "transform.rotation"]]);
const SHADOW_RECOIL = Object.freeze([
  ["recoilKickX", "transform.x"],
  ["recoilKickY", "transform.y"],
]);
const WEAPON_MOTION = Object.freeze([
  ["weaponVisualFacing", "transform.rotation"],
  ...SHADOW_RECOIL,
]);

const RIFLEMAN_DEFINITION = definition({
  id: "rifleman.raster",
  kind: KIND.RIFLEMAN,
  parts: [ellipseShadow("part.shadow", 0, 3.15, 9, 5.4)],
  anchors: { origin: { x: 0, y: 0 }, selection: { x: 0, y: 0 }, hp: { x: 0, y: -17 }, muzzle: { x: 16.38, y: 0 } },
  bounds: { selection: { x: -13, y: -13, width: 26, height: 26 }, hp: { x: -11, y: -18, width: 22, height: 6 } },
  animations: [],
});

const PANZERFAUST_DEFINITION = definition({
  id: "rifleman.panzerfaust-loaded.raster",
  kind: KIND.RIFLEMAN,
  parts: [ellipseShadow("part.shadow", 0, 3.15, 9, 5.4)],
  anchors: { origin: { x: 0, y: 0 }, selection: { x: 0, y: 0 }, hp: { x: 0, y: -17 }, muzzle: { x: 27.8, y: 0 } },
  bounds: { selection: { x: -13, y: -13, width: 26, height: 26 }, hp: { x: -11, y: -18, width: 22, height: 6 } },
  animations: [],
});

const MACHINE_GUNNER_DEFINITION = definition({
  id: "machine-gunner.raster",
  kind: KIND.MACHINE_GUNNER,
  parts: [ellipseShadow("part.shadow", 0, 3.5, 10, 6)],
  anchors: { origin: { x: 0, y: 0 }, selection: { x: 0, y: 0 }, hp: { x: 0, y: -18 }, muzzle: { x: 24.6, y: 0 }, bipod: { x: 17.2, y: 0 } },
  bounds: { selection: { x: -15, y: -15, width: 30, height: 30 }, hp: { x: -12, y: -20, width: 24, height: 6 } },
  animations: [],
});

const SCOUT_PLANE_DEFINITION = definition({
  id: "scout-plane.raster",
  kind: KIND.SCOUT_PLANE,
  parts: [ellipseShadow("part.shadow", -1.5, 5.5, 20, 8, 0.22)],
  anchors: { origin: { x: 0, y: 0 }, selection: { x: 0, y: 0 }, hp: { x: 0, y: -22 } },
  bounds: { selection: { x: -25, y: -18, width: 52, height: 36 }, hp: { x: -14, y: -25, width: 28, height: 5 } },
  animations: bindingsFor(["part.shadow"], SHADOW_FACING),
});

const AT_ANIMATION_PARTS = animatedRasterParts(ANTI_TANK_GUN_PNG_RIG_ATLAS);
const AT_PACKED = Object.freeze(["part.at.axle.packed", "part.at.barrel.packed"]);
const AT_DEPLOYED = Object.freeze([
  "part.at.trail.left.deployed",
  "part.at.trail.right.deployed",
  "part.at.axle.deployed",
  "part.at.barrel.deployed",
]);
const AT_CARRIAGE = Object.freeze([
  "part.at.trail.left.deployed",
  "part.at.trail.right.deployed",
  "part.at.axle.packed",
  "part.at.axle.deployed",
]);
const AT_BARRELS = Object.freeze(["part.at.barrel.packed", "part.at.barrel.deployed"]);
const AT_ANIMATIONS = [
  ...bindingsFor(["part.shadow"], [...SHADOW_FACING, ...SHADOW_RECOIL]),
  ...bindingsFor(AT_CARRIAGE, [...WEAPON_MOTION, ["weaponRecoilX", "transform.x", 0.12], ["weaponRecoilY", "transform.y", 0.12]]),
  ...bindingsFor(AT_BARRELS, [...WEAPON_MOTION, ["weaponRecoilX", "transform.x"], ["weaponRecoilY", "transform.y"]]),
  ...bindingsFor(AT_PACKED, [["setupVisual", "alpha", -1, 1]]),
  ...bindingsFor(AT_DEPLOYED, [["setupVisual", "alpha"]]),
];
const ANTI_TANK_GUN_DEFINITION = definition({
  id: "anti-tank-gun.raster",
  kind: KIND.ANTI_TANK_GUN,
  parts: [polygonShadow("part.shadow", 25, 16, 5.6, 6), ...AT_ANIMATION_PARTS],
  anchors: { origin: { x: 0, y: 0 }, selection: { x: 0, y: 0 }, hp: { x: 0, y: -28 }, muzzle: { x: 38, y: 0 }, wheel: { x: -3.2, y: 8.4 } },
  bounds: { selection: { x: -25, y: -17, width: 50, height: 34 }, hp: { x: -16, y: -33, width: 32, height: 5 } },
  animations: AT_ANIMATIONS,
});

const MORTAR_ANIMATION_PARTS = animatedRasterParts(MORTAR_TEAM_PNG_RIG_ATLAS);
const MORTAR_PACKED = Object.freeze(["part.mortar.axle.packed", "part.mortar.tube.packed"]);
const MORTAR_DEPLOYED = Object.freeze(["part.mortar.axle.deployed", "part.mortar.tube.deployed"]);
const MORTAR_CARRIAGE = Object.freeze(["part.mortar.axle.packed", "part.mortar.axle.deployed"]);
const MORTAR_TUBES = Object.freeze(["part.mortar.tube.packed", "part.mortar.tube.deployed"]);
const MORTAR_ANIMATIONS = [
  ...bindingsFor(["part.shadow"], SHADOW_RECOIL),
  ...bindingsFor(MORTAR_CARRIAGE, [...WEAPON_MOTION, ["weaponRecoilX", "transform.x", 0.18], ["weaponRecoilY", "transform.y", 0.18]]),
  ...bindingsFor(MORTAR_TUBES, [...WEAPON_MOTION, ["weaponRecoilX", "transform.x"], ["weaponRecoilY", "transform.y"]]),
  ...bindingsFor(MORTAR_PACKED, [["setupVisual", "alpha", -1, 1]]),
  ...bindingsFor(MORTAR_DEPLOYED, [["setupVisual", "alpha"]]),
  ...bindingsFor(["part.mortar.basePlate.deployed"], [
    ["weaponVisualFacing", "transform.rotation"],
    ["setupVisual", "geometry.scaleX", 1, -1],
    ["setupVisual", "geometry.scaleY", 1, -1],
    ["setupVisual", "alpha"],
  ]),
];
const MORTAR_TEAM_DEFINITION = definition({
  id: "mortar-team.raster",
  kind: KIND.MORTAR_TEAM,
  parts: [ellipseShadow("part.shadow", 0, 6.3, 18, 10.8), ...MORTAR_ANIMATION_PARTS],
  anchors: { origin: { x: 0, y: 0 }, selection: { x: 0, y: 0 }, hp: { x: 0, y: -26 }, muzzle: { x: 13.32, y: 0 }, bipod: { x: 3.96, y: 0 } },
  bounds: { selection: { x: -21, y: -21, width: 42, height: 42 }, hp: { x: -13, y: -31, width: 26, height: 5 } },
  animations: MORTAR_ANIMATIONS,
});

const ARTILLERY_FLASH_PARTS = Object.freeze([
  part("part.art.flashCone", 51, {
    geometry: { type: "polygon", points: [{ x: -2.7, y: 0 }, { x: 9, y: -4.5 }, { x: 9, y: 4.5 }] },
    paint: paint({ fill: "#ffd84a", fillOpacity: 0.78, stroke: "#d8d0b0", strokeWidth: 2.2, strokeOpacity: 0.62 }),
  }),
  part("part.art.flashCore", 52, {
    geometry: { type: "circle", cx: 0, cy: 0, r: 3.5 },
    paint: paint({ fill: "#fff2d0", fillOpacity: 0.9, stroke: "#d8d0b0", strokeWidth: 2.2, strokeOpacity: 0.62 }),
  }),
  part("part.art.flashGlow", 53, {
    geometry: { type: "circle", cx: 0, cy: 0, r: 6 },
    paint: paint({ fill: "#fff06a", fillOpacity: 0.58, stroke: "#d8d0b0", strokeWidth: 2.2, strokeOpacity: 0.62 }),
  }),
]);
const ARTILLERY_ANIMATION_PARTS = animatedRasterParts(ARTILLERY_PNG_RIG_ATLAS);
const ARTILLERY_PACKED = Object.freeze([
  "part.art.trail.left.packed",
  "part.art.trail.right.packed",
  "part.art.axle.packed",
  "part.art.barrel.packed",
]);
const ARTILLERY_DEPLOYED = Object.freeze([
  "part.art.trail.left.deployed",
  "part.art.trail.right.deployed",
  "part.art.axle.deployed",
  "part.art.barrel.deployed",
]);
const ARTILLERY_CARRIAGE = Object.freeze([
  "part.art.trail.left.packed",
  "part.art.trail.right.packed",
  "part.art.trail.left.deployed",
  "part.art.trail.right.deployed",
  "part.art.axle.packed",
  "part.art.axle.deployed",
]);
const ARTILLERY_BARRELS = Object.freeze(["part.art.barrel.packed", "part.art.barrel.deployed"]);
const ARTILLERY_FLASH_IDS = Object.freeze(ARTILLERY_FLASH_PARTS.map((entry) => entry.id));
const ARTILLERY_ANIMATIONS = [
  ...bindingsFor(["part.shadow"], SHADOW_RECOIL),
  ...bindingsFor(ARTILLERY_CARRIAGE, [
    ["carriageVisualFacing", "transform.rotation"],
    ...SHADOW_RECOIL,
    ["weaponRecoilX", "transform.x", 0.42],
    ["weaponRecoilY", "transform.y", 0.42],
  ]),
  ...bindingsFor(ARTILLERY_BARRELS, [
    ["weaponFacing", "transform.rotation"],
    ...SHADOW_RECOIL,
    ["weaponRecoilX", "transform.x", 1.35],
    ["weaponRecoilY", "transform.y", 1.35],
  ]),
  ...bindingsFor(ARTILLERY_PACKED, [["setupVisual", "alpha", -1, 1]]),
  ...bindingsFor(ARTILLERY_DEPLOYED, [["setupVisual", "alpha"]]),
  ...bindingsFor(ARTILLERY_FLASH_IDS, [
    ["weaponFacing", "transform.rotation"],
    ...SHADOW_RECOIL,
    ["weaponRecoilX", "transform.x", 1.35],
    ["weaponRecoilY", "transform.y", 1.35],
    ["weaponFacingCos", "transform.x", 54.432],
    ["weaponFacingSin", "transform.y", 54.432],
    ["recoilPx", "alpha", 0.1],
  ]),
  binding("part.art.flashCone", "recoilPx", "geometry.scaleX", 0.1),
  binding("part.art.flashCone", "recoilPx", "geometry.scaleY", 0.084444444444),
  ...bindingsFor(["part.art.flashCore"], [["recoilPx", "geometry.scaleX", 0.08], ["recoilPx", "geometry.scaleY", 0.08]]),
  ...bindingsFor(["part.art.flashGlow"], [["recoilPx", "geometry.scaleX", 0.066666666667], ["recoilPx", "geometry.scaleY", 0.066666666667]]),
];
const ARTILLERY_DEFINITION = definition({
  id: "artillery.raster",
  kind: KIND.ARTILLERY,
  parts: [ellipseShadow("part.shadow", 0, 8.925983, 25.502809, 15.301685), ...ARTILLERY_ANIMATION_PARTS, ...ARTILLERY_FLASH_PARTS],
  anchors: { origin: { x: 0, y: 0 }, selection: { x: 0, y: 0 }, hp: { x: 0, y: -33 }, muzzle: { x: 45.864, y: 0 }, wheel: { x: -4.536, y: 11.808 } },
  bounds: { selection: { x: -30, y: -22, width: 60, height: 44 }, hp: { x: -16, y: -38, width: 32, height: 5 } },
  animations: ARTILLERY_ANIMATIONS,
});

const SCOUT_CAR_ANIMATION_PARTS = animatedRasterParts(SCOUT_CAR_PNG_RIG_ATLAS, {
  "part.gunnerBarrel": { paint: paint({ opacity: 0.98 }) },
});
const SCOUT_CAR_ANIMATIONS = [
  ...bindingsFor(["part.shadow", "part.hull"], SHADOW_FACING),
  ...bindingsFor(["part.gunnerBarrel"], [
    ["weaponFacing", "transform.rotation"],
    ["scoutGunnerX", "transform.x"],
    ["scoutGunnerY", "transform.y"],
  ]),
];
const SCOUT_CAR_DEFINITION = definition({
  id: "scout-car.raster",
  kind: KIND.SCOUT_CAR,
  parts: [polygonShadow("part.shadow", 24.4, 14.8, 5.18, 3), ...SCOUT_CAR_ANIMATION_PARTS],
  anchors: { origin: { x: 0, y: 0 }, selection: { x: 0, y: 0 }, hp: { x: 0, y: -24 }, muzzle: { x: 15.912, y: 0 } },
  bounds: { selection: { x: -24, y: -16, width: 48, height: 32 }, hp: { x: -14, y: -28, width: 28, height: 6 } },
  animations: SCOUT_CAR_ANIMATIONS,
});

const TANK_NATIVE_PARTS = Object.freeze([
  part("part.coaxBarrel", 26),
  part("part.tank.flashCone", 29, {
    geometry: { type: "polygon", points: [{ x: -1.35, y: 0 }, { x: 4.5, y: -2.25 }, { x: 4.5, y: 2.25 }] },
    paint: paint({ fill: "#ffd84a", fillOpacity: 0.78, stroke: "#d8d0b0", strokeWidth: 1.1, strokeOpacity: 0.62 }),
  }),
  part("part.tank.flashCore", 30, {
    geometry: { type: "circle", cx: 0, cy: 0, r: 1.75 },
    paint: paint({ fill: "#fff2d0", fillOpacity: 0.9, stroke: "#d8d0b0", strokeWidth: 1.1, strokeOpacity: 0.62 }),
  }),
  part("part.tank.flashGlow", 31, {
    geometry: { type: "circle", cx: 0, cy: 0, r: 3 },
    paint: paint({ fill: "#fff06a", fillOpacity: 0.58, stroke: "#d8d0b0", strokeWidth: 1.1, strokeOpacity: 0.62 }),
  }),
  part("part.fuelCue.box", 32, {
    geometry: { type: "rect", x: -23.2, y: -20.9, width: 8, height: 5 },
    paint: paint({ stroke: "#c9b56a", strokeWidth: 2, opacity: 0.75 }),
  }),
  part("part.fuelCue.x1", 33, {
    geometry: { type: "line", from: { x: -22.2, y: -19.4 }, to: { x: -16.2, y: -16.4 }, strokeWidth: 2 },
    paint: paint({ stroke: "#d47a5f", strokeWidth: 2, opacity: 0.95 }),
  }),
  part("part.fuelCue.x2", 34, {
    geometry: { type: "line", from: { x: -16.2, y: -19.4 }, to: { x: -22.2, y: -16.4 }, strokeWidth: 2 },
    paint: paint({ stroke: "#d47a5f", strokeWidth: 2, opacity: 0.95 }),
  }),
]);
const TANK_FLASH_IDS = Object.freeze(["part.tank.flashCone", "part.tank.flashCore", "part.tank.flashGlow"]);
const TANK_ANIMATION_PARTS = animatedRasterParts(TANK_PNG_RIG_ATLAS, {
  "part.barrel": { paint: paint({ opacity: 0.95 }) },
});
const TANK_ANIMATIONS = [
  ...bindingsFor(["part.shadow", "part.track.left", "part.track.right", "part.hull"], [...SHADOW_FACING, ...SHADOW_RECOIL]),
  ...bindingsFor(["part.turret", "part.coaxBarrel"], [["weaponFacing", "transform.rotation"], ...SHADOW_RECOIL]),
  ...bindingsFor(["part.barrel"], [["weaponFacing", "transform.rotation"], ["recoilPx", "transform.scaleX", -0.0301204819], ...SHADOW_RECOIL]),
  ...bindingsFor(TANK_FLASH_IDS, [
    ["weaponFacing", "transform.rotation"],
    ...SHADOW_RECOIL,
    ["weaponRecoilX", "transform.x"],
    ["weaponRecoilY", "transform.y"],
    ["weaponFacingCos", "transform.x", 42],
    ["weaponFacingSin", "transform.y", 42],
    ["recoilPx", "alpha", 0.111111111111],
  ]),
  binding("part.tank.flashCone", "recoilPx", "geometry.scaleX", 0.333333333333),
  binding("part.tank.flashCone", "recoilPx", "geometry.scaleY", 0.281481481481),
  ...bindingsFor(["part.tank.flashCore"], [["recoilPx", "geometry.scaleX", 0.266666666667], ["recoilPx", "geometry.scaleY", 0.266666666667]]),
  ...bindingsFor(["part.tank.flashGlow"], [["recoilPx", "geometry.scaleX", 0.222222222222], ["recoilPx", "geometry.scaleY", 0.222222222222]]),
  ...bindingsFor(["part.fuelCue.box"], [["facing", "transform.rotation"], ["fuelCueVisible", "visible"], ...SHADOW_RECOIL]),
  ...bindingsFor(["part.fuelCue.x1", "part.fuelCue.x2"], [["facing", "transform.rotation"], ["oilStarved", "visible"], ...SHADOW_RECOIL]),
];
const TANK_DEFINITION = definition({
  id: "tank.raster",
  kind: KIND.TANK,
  parts: [polygonShadow("part.shadow", 29.7, 18.9, 6.615, 3), ...TANK_ANIMATION_PARTS, ...TANK_NATIVE_PARTS],
  anchors: { origin: { x: 0, y: 0 }, selection: { x: 0, y: 2 }, hp: { x: 0, y: -26 }, muzzle: { x: 33.2, y: 0 }, coaxMuzzle: { x: 16.6, y: -5.55 }, turret: { x: 1, y: 0 } },
  bounds: { selection: { x: -29.2, y: -19.4, width: 58.4, height: 38.8 }, hp: { x: -16, y: -31, width: 32, height: 5 } },
  animations: TANK_ANIMATIONS,
});

const RASTER_DEFINITION_ENTRIES = Object.freeze([
  [KIND.ANTI_TANK_GUN, ANTI_TANK_GUN_DEFINITION],
  [KIND.ARTILLERY, ARTILLERY_DEFINITION],
  [KIND.MACHINE_GUNNER, MACHINE_GUNNER_DEFINITION],
  [KIND.MORTAR_TEAM, MORTAR_TEAM_DEFINITION],
  [LOADED_RIFLEMAN_RIG_KEY, PANZERFAUST_DEFINITION],
  [KIND.RIFLEMAN, RIFLEMAN_DEFINITION],
  [KIND.SCOUT_CAR, SCOUT_CAR_DEFINITION],
  [KIND.SCOUT_PLANE, SCOUT_PLANE_DEFINITION],
  [KIND.TANK, TANK_DEFINITION],
]);
const RASTER_RIG_KEYS = new Set(RASTER_DEFINITION_ENTRIES.map(([kind]) => kind));

const TANK_UNIT_OVERLAYS = Object.freeze(["part.fuelCue.box", "part.fuelCue.x1", "part.fuelCue.x2"]);
const RASTER_PART_ENTRIES = Object.freeze([
  [KIND.ANTI_TANK_GUN, { shadow: ["part.shadow"], unit: atlasSourceParts(ANTI_TANK_GUN_PNG_RIG_ATLAS) }],
  [KIND.ARTILLERY, { shadow: ["part.shadow"], unit: [...atlasSourceParts(ARTILLERY_PNG_RIG_ATLAS), ...ARTILLERY_FLASH_IDS] }],
  [KIND.MACHINE_GUNNER, { shadow: ["part.shadow"], unit: ["raster.frame"] }],
  [KIND.MORTAR_TEAM, { shadow: ["part.shadow"], unit: atlasSourceParts(MORTAR_TEAM_PNG_RIG_ATLAS) }],
  [LOADED_RIFLEMAN_RIG_KEY, { shadow: ["part.shadow"], unit: ["raster.frame"] }],
  [KIND.RIFLEMAN, { shadow: ["part.shadow"], unit: ["raster.frame"] }],
  [KIND.SCOUT_CAR, { shadow: ["part.shadow"], unit: atlasSourceParts(SCOUT_CAR_PNG_RIG_ATLAS) }],
  [KIND.SCOUT_PLANE, { shadow: ["part.shadow"], unit: ["raster.frame"] }],
  [KIND.TANK, {
    shadow: ["part.shadow"],
    unit: [...atlasSourceParts(TANK_PNG_RIG_ATLAS), ...TANK_UNIT_OVERLAYS],
    effects: TANK_FLASH_IDS,
  }],
]);

export const RASTER_RIG_DEFINITIONS = new Map(RASTER_DEFINITION_ENTRIES);
export const RASTER_RIG_PARTS = Object.freeze(Object.fromEntries(
  RASTER_PART_ENTRIES.map(([kind, routes]) => [kind, deepFreeze(routes)]),
));

export function rasterRigKinds() {
  return RASTER_DEFINITION_ENTRIES.map(([kind]) => kind);
}

export function isRasterRigKey(kind) {
  return RASTER_RIG_KEYS.has(kind);
}
