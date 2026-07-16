import { KIND } from "../../protocol.js";
import { SCOUT_PLANE_PARTS, SCOUT_PLANE_RIG_SVG } from "./aircraft_svg.js";
import {
  LOADED_RIFLEMAN_PANZERFAUST_RIG_SVG,
  MACHINE_GUNNER_RIG_SVG,
  RIFLEMAN_RIG_SVG,
} from "./infantry_svg.js";
import { compileSvgRig } from "./svg_importer.js";
import {
  ANTI_TANK_GUN_PARTS,
  ANTI_TANK_GUN_RIG_SVG,
  ARTILLERY_PARTS,
  ARTILLERY_RIG_SVG,
  MORTAR_TEAM_PARTS,
  MORTAR_TEAM_RIG_SVG,
} from "./support_svg.js";
import { TANK_RIG_SVG } from "./tank_svg.js";
import {
  COMMAND_CAR_PARTS,
  COMMAND_CAR_RIG_SVG,
  EKAT_PARTS,
  EKAT_RIG_SVG,
  SCOUT_CAR_PARTS,
  SCOUT_CAR_RIG_SVG,
} from "./vehicle_svg.js";
import { GOLEM_RIG_SVG, WORKER_RIG_SVG } from "./worker_svg.js";

const LOADED_RIFLEMAN_RIG_KEY = "rifleman.panzerfaustLoaded";

const DEFAULT_POOL_PROFILE = Object.freeze({
  familyKey: "liveUnit",
  liveRigShadow: "liveUnitRigShadows",
  shadow: "unitShadows",
  liveRigUnit: "liveUnitRigs",
  unit: "units",
  liveRigOverlay: "liveUnitRigOverlays",
  overlay: "units",
  liveRigEffects: "liveUnitRigEffects",
  effects: "units",
});

const SHOT_REVEAL_POOL_PROFILE = Object.freeze({
  familyKey: "shotReveal",
  liveRigShadow: "liveShotRevealRigShadows",
  shadow: "shotRevealShadows",
  liveRigUnit: "liveShotRevealRigs",
  unit: "shotReveals",
  liveRigOverlay: "liveShotRevealRigOverlays",
  overlay: "shotReveals",
  liveRigEffects: "liveShotRevealRigEffects",
  effects: "shotReveals",
});

const ROUTE_PLAN_CACHE = new Map([
  [DEFAULT_POOL_PROFILE, new Map()],
  [SHOT_REVEAL_POOL_PROFILE, new Map()],
]);

const LIVE_RIG_SOURCES = Object.freeze([
  [KIND.ANTI_TANK_GUN, ANTI_TANK_GUN_RIG_SVG],
  [KIND.ARTILLERY, ARTILLERY_RIG_SVG],
  [KIND.COMMAND_CAR, COMMAND_CAR_RIG_SVG],
  [KIND.EKAT, EKAT_RIG_SVG],
  [KIND.GOLEM, GOLEM_RIG_SVG],
  [KIND.MACHINE_GUNNER, MACHINE_GUNNER_RIG_SVG],
  [KIND.MORTAR_TEAM, MORTAR_TEAM_RIG_SVG],
  [LOADED_RIFLEMAN_RIG_KEY, LOADED_RIFLEMAN_PANZERFAUST_RIG_SVG],
  [KIND.RIFLEMAN, RIFLEMAN_RIG_SVG],
  [KIND.SCOUT_CAR, SCOUT_CAR_RIG_SVG],
  [KIND.SCOUT_PLANE, SCOUT_PLANE_RIG_SVG],
  [KIND.TANK, TANK_RIG_SVG],
  [KIND.WORKER, WORKER_RIG_SVG],
]);

const WORKER_UNIT_PARTS = Object.freeze([
  "part.body",
  "part.busyIndicator",
  "part.facingTick",
]);

const TANK_UNIT_PARTS = Object.freeze([
  "part.track.left",
  "part.track.right",
  "part.tread.left.0",
  "part.tread.left.1",
  "part.tread.left.2",
  "part.tread.left.3",
  "part.tread.left.4",
  "part.tread.left.5",
  "part.tread.left.6",
  "part.tread.left.7",
  "part.tread.left.8",
  "part.tread.right.0",
  "part.tread.right.1",
  "part.tread.right.2",
  "part.tread.right.3",
  "part.tread.right.4",
  "part.tread.right.5",
  "part.tread.right.6",
  "part.tread.right.7",
  "part.tread.right.8",
  "part.hull",
  "part.hull.shadow",
  "part.hull.nose",
  "part.hull.noseShadow",
  "part.barrel",
  "part.coaxBarrel",
  "part.turret",
  "part.noseTick",
  "part.fuelCue.box",
  "part.fuelCue.x1",
  "part.fuelCue.x2",
]);

const TANK_EFFECT_PARTS = Object.freeze([
  "part.tank.flashCone",
  "part.tank.flashCore",
  "part.tank.flashGlow",
]);

const RIFLEMAN_UNIT_PARTS = Object.freeze([
  "part.body",
  "part.head",
  "part.shoulders",
  "part.rifle.barrel",
  "part.rifle.hand",
]);

const MACHINE_GUNNER_UNIT_PARTS = Object.freeze([
  "part.body",
  "part.head",
  "part.shoulders",
  "part.mg.main",
  "part.mg.stock",
  "part.mg.receiver",
  "part.mg.topPlate",
  "part.mg.shroud",
  "part.mg.slot.0",
  "part.mg.slot.1",
  "part.mg.slot.2",
  "part.mg.slot.3",
  "part.mg.muzzleTick",
  "part.mg.grip",
  "part.mg.bipod",
  "part.mg.muzzleCap",
]);

const PANZERFAUST_UNIT_PARTS = Object.freeze([
  "part.body",
  "part.head",
  "part.shoulders",
  "part.pzf.sling",
  "part.pzf.tube",
  "part.pzf.rear",
  "part.pzf.warhead",
  "part.pzf.teamBand",
  "part.pzf.grip",
]);

const LIVE_RIG_PARTS = Object.freeze({
  [KIND.ANTI_TANK_GUN]: Object.freeze({
    shadow: ANTI_TANK_GUN_PARTS.shadow,
    unit: ANTI_TANK_GUN_PARTS.weapon,
  }),
  [KIND.ARTILLERY]: Object.freeze({
    shadow: ARTILLERY_PARTS.shadow,
    unit: ARTILLERY_PARTS.weapon,
  }),
  [KIND.COMMAND_CAR]: COMMAND_CAR_PARTS,
  [KIND.EKAT]: EKAT_PARTS,
  [KIND.GOLEM]: Object.freeze({
    shadow: Object.freeze(["part.shadow"]),
    unit: WORKER_UNIT_PARTS,
  }),
  [KIND.MACHINE_GUNNER]: Object.freeze({
    shadow: Object.freeze(["part.shadow"]),
    unit: MACHINE_GUNNER_UNIT_PARTS,
  }),
  [KIND.MORTAR_TEAM]: Object.freeze({
    shadow: MORTAR_TEAM_PARTS.shadow,
    unit: MORTAR_TEAM_PARTS.weapon,
  }),
  [LOADED_RIFLEMAN_RIG_KEY]: Object.freeze({
    shadow: Object.freeze(["part.shadow"]),
    unit: PANZERFAUST_UNIT_PARTS,
  }),
  [KIND.RIFLEMAN]: Object.freeze({
    shadow: Object.freeze(["part.shadow"]),
    unit: RIFLEMAN_UNIT_PARTS,
  }),
  [KIND.SCOUT_CAR]: SCOUT_CAR_PARTS,
  [KIND.SCOUT_PLANE]: SCOUT_PLANE_PARTS,
  [KIND.TANK]: Object.freeze({
    shadow: Object.freeze(["part.shadow"]),
    unit: TANK_UNIT_PARTS,
    effects: TANK_EFFECT_PARTS,
  }),
  [KIND.WORKER]: Object.freeze({
    shadow: Object.freeze(["part.shadow"]),
    unit: WORKER_UNIT_PARTS,
  }),
});

export function createLiveRigDefinitions() {
  const definitions = new Map();
  for (const [kind, svgText] of LIVE_RIG_SOURCES) {
    const expectedKind = kind === LOADED_RIFLEMAN_RIG_KEY ? KIND.RIFLEMAN : kind;
    const compiled = compileSvgRig(svgText, { expectedKind });
    if (compiled.ok) definitions.set(kind, compiled.definition);
    else console.warn(`RTS live rig disabled for ${kind}: ${JSON.stringify(compiled.errors)}`);
  }
  return definitions;
}

export function liveRigKinds() {
  return LIVE_RIG_SOURCES
    .map(([kind]) => kind)
    .filter((kind) => kind !== LOADED_RIFLEMAN_RIG_KEY);
}

export function liveRigKeyForEntity(entity) {
  return entity?.kind === KIND.RIFLEMAN && entity?.panzerfaustLoaded === true
    ? LOADED_RIFLEMAN_RIG_KEY
    : entity?.kind;
}

export function liveRigDefinitionFor(definitions, kind) {
  return definitions?.get?.(kind) ?? null;
}

export function liveRigRoutesFor(kind, pools = {}) {
  return liveRigRoutePlanFor(kind, pools).routes;
}

export function liveRigRoutePlanFor(kind, pools = {}) {
  const parts = LIVE_RIG_PARTS[kind];
  if (!parts) return EMPTY_ROUTE_PLAN;
  const profile = knownPoolProfile(pools);
  if (profile) {
    const cache = ROUTE_PLAN_CACHE.get(profile);
    let plan = cache.get(kind);
    if (!plan) {
      plan = compileRoutePlan(parts, profile, profile.familyKey);
      cache.set(kind, plan);
    }
    return plan;
  }
  return compileRoutePlan(parts, pools, null);
}

const EMPTY_ROUTE_PLAN = Object.freeze({
  routes: Object.freeze([]),
  allParts: new Set(),
  poolNames: Object.freeze([]),
  familyKey: null,
  shadowRoute: null,
  overlayPoolName: null,
  overlayLayerName: null,
});

function compileRoutePlan(parts, pools, familyKey) {
  const routes = [
    Object.freeze({
      poolName: pools.liveRigShadow || "liveUnitRigShadows",
      layerName: pools.shadow || "unitShadows",
      parts: parts.shadow,
    }),
    Object.freeze({
      poolName: pools.liveRigUnit || "liveUnitRigs",
      layerName: pools.unit || "units",
      parts: parts.unit,
    }),
  ];
  if (Array.isArray(parts.overlay) && parts.overlay.length > 0) {
    routes.push(Object.freeze({
      poolName: pools.liveRigOverlay || "liveUnitRigOverlays",
      layerName: pools.overlay || pools.unit || "units",
      parts: parts.overlay,
    }));
  }
  if (parts.effects?.length) {
    routes.push(Object.freeze({
      poolName: pools.liveRigEffects || "liveUnitRigEffects",
      layerName: pools.effects || pools.unit || "units",
      parts: parts.effects,
    }));
  }
  const allParts = new Set();
  const poolNames = [];
  for (const route of routes) {
    poolNames.push(route.poolName);
    for (const partId of route.parts || []) allParts.add(partId);
  }
  return Object.freeze({
    routes: Object.freeze(routes),
    allParts,
    poolNames: Object.freeze(poolNames),
    familyKey,
    shadowRoute: routes.find((route) => route.parts?.includes?.("part.shadow")) ?? null,
    overlayPoolName: pools.liveRigOverlay || "liveUnitRigOverlays",
    overlayLayerName: pools.overlay || pools.unit || "units",
  });
}

function knownPoolProfile(pools) {
  if (usesProfile(pools, SHOT_REVEAL_POOL_PROFILE) && matchesPoolProfile(pools, SHOT_REVEAL_POOL_PROFILE)) {
    return SHOT_REVEAL_POOL_PROFILE;
  }
  if (matchesPoolProfile(pools, DEFAULT_POOL_PROFILE)) return DEFAULT_POOL_PROFILE;
  return null;
}

function usesProfile(pools, profile) {
  return (
    pools.liveRigShadow === profile.liveRigShadow ||
    pools.shadow === profile.shadow ||
    pools.liveRigUnit === profile.liveRigUnit ||
    pools.unit === profile.unit ||
    pools.liveRigOverlay === profile.liveRigOverlay ||
    pools.overlay === profile.overlay ||
    pools.liveRigEffects === profile.liveRigEffects ||
    pools.effects === profile.effects
  );
}

function matchesPoolProfile(pools, profile) {
  return (
    (pools.liveRigShadow == null || pools.liveRigShadow === profile.liveRigShadow) &&
    (pools.shadow == null || pools.shadow === profile.shadow) &&
    (pools.liveRigUnit == null || pools.liveRigUnit === profile.liveRigUnit) &&
    (pools.unit == null || pools.unit === profile.unit) &&
    (pools.liveRigOverlay == null || pools.liveRigOverlay === profile.liveRigOverlay) &&
    (pools.overlay == null || pools.overlay === profile.overlay) &&
    (pools.liveRigEffects == null || pools.liveRigEffects === profile.liveRigEffects) &&
    (pools.effects == null || pools.effects === profile.effects)
  );
}
