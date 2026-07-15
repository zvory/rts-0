import { STATS } from "../../config.js";
import { KIND, SETUP, STATE } from "../../protocol.js";
import { angleLerp, clamp01, hexToInt, polar, recoilVector, smoothstep01, tankBodyVisual, weaponRecoilOffset } from "../shared.js";

const TRANSFORM_PROPERTIES = new Set([
  "transform.x",
  "transform.y",
  "transform.rotation",
  "transform.scaleX",
  "transform.scaleY",
]);
const LOCAL_TRANSFORM_PROPERTIES = new Set(["transform.localX", "transform.localY"]);
const GEOMETRY_SCALE_PROPERTIES = new Set(["geometry.scaleX", "geometry.scaleY"]);
const RIG_CONTEXT_READY = Symbol("rtsRigContextReady");

export function createRigRenderContext(entity, {
  state = {},
  colorByOwner = new Map(),
  now = performance.now(),
  setupVisual = null,
  vehicleMotion = null,
  selected = false,
  visibility = "visible",
  shotRevealAlpha = 1,
  map = null,
  occupiedTrench = false,
} = {}) {
  const facing = finite(entity.facing, 0);
  const weaponFacing = finite(entity.weaponFacing, facing);
  const recoilProgress = typeof state.weaponRecoil === "function"
    ? clamp01(state.weaponRecoil(entity.id, entity.kind, now))
    : clamp01(entity.recoilProgress ?? 0);
  const recoilPhase = typeof state.weaponRecoilPhase === "function"
    ? clamp01(state.weaponRecoilPhase(entity.id, entity.kind, now))
    : clamp01(entity.recoilPhase ?? (recoilProgress > 0 ? 1 - recoilProgress : 0));
  const recoilPx = weaponRecoilOffset(entity.kind, recoilProgress);
  const setup = setupVisual ?? defaultSetupVisual(entity);
  const deploy = clamp01(setup.prongFactor);
  const weaponVisualFacing = visualWeaponFacing(entity.kind, facing, weaponFacing, deploy);
  const carriageVisualFacing = visualCarriageFacing(entity.kind, facing, weaponFacing, deploy, weaponVisualFacing);
  const weaponRecoil = recoilPx > 0 ? polar(weaponVisualFacing + Math.PI, recoilPx) : { x: 0, y: 0 };
  const scoutGunner = scoutGunnerOffsets(entity, facing, weaponFacing, recoilPx);
  const recoilKickFactor = entity.kind === KIND.TANK
    ? 0.85
    : entity.kind === KIND.ARTILLERY
      ? 0.65
      : entity.kind === KIND.ANTI_TANK_GUN
        ? 0
        : entity.kind === KIND.MORTAR_TEAM
          ? 0.28
          : 0;
  const recoilKick = recoilPx > 0 && recoilKickFactor > 0
    ? {
      x: Math.cos(weaponFacing + Math.PI) * recoilPx * recoilKickFactor,
      y: Math.sin(weaponFacing + Math.PI) * recoilPx * recoilKickFactor,
    }
    : { x: 0, y: 0 };
  const ownUnit = entity.owner === state.playerId;
  const oil = state.resources ? state.resources.oil : null;
  const lowOil = ownUnit && typeof oil === "number" && oil > 0 && oil <= 5;
  const oilStarved = ownUnit && oil === 0 && (entity.state === STATE.MOVE || entity.state === STATE.ATTACK);
  const context = {
    now,
    teamColor: colorByOwner.get(entity.owner) ?? hexToInt(entity.teamColor),
    recoilProgress,
    recoilPhase,
    recoilPx,
    recoilKickX: recoilKick.x,
    recoilKickY: recoilKick.y,
    setupVisual: setupVisual ?? defaultSetupVisual(entity),
    vehicleMotion: vehicleMotion ?? defaultVehicleMotion(),
    selected: Boolean(selected),
    damaged: finite(entity.maxHp, 0) > 0 && finite(entity.hp, 0) < finite(entity.maxHp, 0),
    shotRevealAlpha: clamp01(shotRevealAlpha),
    visibility,
    occupiedTrench: Boolean(occupiedTrench),
    mapTileSize: finite(map?.tileSize, 32),
    facing,
    weaponFacing,
    weaponFacingCos: Math.cos(weaponFacing),
    weaponFacingSin: Math.sin(weaponFacing),
    weaponVisualFacing,
    carriageVisualFacing,
    weaponVisualDoubleCos: Math.cos(weaponVisualFacing * 2),
    weaponVisualDoubleSin: Math.sin(weaponVisualFacing * 2),
    weaponRecoilX: weaponRecoil.x,
    weaponRecoilY: weaponRecoil.y,
    scoutGunnerX: scoutGunner.gunner.x,
    scoutGunnerY: scoutGunner.gunner.y,
    scoutMountX: scoutGunner.mount.x,
    scoutMountY: scoutGunner.mount.y,
    setupVisible: deploy > 0.02,
    setupMostlyDeployed: deploy > 0.55,
    setupBarrelVisible: Boolean(setup.barrel || deploy > 0.75),
    busy: isBusy(entity),
    breakthroughTicks: finite(entity.breakthroughTicks, 0),
    lowOil,
    oilStarved,
    fuelCueVisible: lowOil || oilStarved,
    panzerfaustLoaded: entity.panzerfaustLoaded !== false,
  };
  Object.defineProperty(context, RIG_CONTEXT_READY, { value: true });
  return context;
}

export function transformedRigAnchorPoint(definition, entity, anchorName, renderOptions = {}) {
  const anchor = definition?.anchors?.[anchorName];
  if (!anchor || !Number.isFinite(anchor.x) || !Number.isFinite(anchor.y)) return null;
  if (!Number.isFinite(entity?.x) || !Number.isFinite(entity?.y)) return null;
  const context = renderOptions?.[RIG_CONTEXT_READY]
    ? renderOptions
    : createRigRenderContext(entity, renderOptions);
  const partId = rigAnchorPartId(entity.kind, anchorName);
  if (partId) {
    const sampled = sampleRigAnimation(definition, entity, context);
    const part = sampled.parts?.[partId];
    if (part) return transformedRigPartPoint(entity, anchor, part);
  }
  const rotation = rigAnchorRotation(entity.kind, anchorName, context);
  const c = Math.cos(rotation);
  const s = Math.sin(rotation);
  return {
    x: entity.x + anchor.x * c - anchor.y * s,
    y: entity.y + anchor.x * s + anchor.y * c,
  };
}

function scoutGunnerOffsets(entity, facing, weaponFacing, recoilPx) {
  if (entity.kind !== KIND.SCOUT_CAR) {
    return {
      gunner: { x: 0, y: 0 },
      mount: { x: 0, y: 0 },
    };
  }
  const body = tankBodyVisual(STATS[entity.kind] || {});
  const c = Math.cos(facing);
  const s = Math.sin(facing);
  const anchorX = -body.halfLen * 0.42;
  const mountX = -body.halfLen * 0.32;
  const recoil = recoilVector(weaponFacing, recoilPx);
  return {
    gunner: {
      x: anchorX * c + recoil.x,
      y: anchorX * s + recoil.y,
    },
    mount: {
      x: mountX * c,
      y: mountX * s,
    },
  };
}

export function sampleRigAnimation(definition, entity, renderContext = {}, options = {}) {
  const inputContext = renderContext || {};
  const context = inputContext[RIG_CONTEXT_READY]
    ? inputContext
    : {
      ...createRigRenderContext(entity, { now: inputContext.now ?? 0 }),
      ...inputContext,
    };
  const parts = {};
  const includeParts = normalizedPartSet(options.includeParts);
  for (const part of definition.parts || []) {
    if (includeParts && !includeParts.has(part.id)) continue;
    parts[part.id] = {
      id: part.id,
      transform: { ...part.transform },
      localOffset: { x: 0, y: 0 },
      geometryScale: { x: 1, y: 1 },
      pivot: { ...part.pivot },
      alpha: finite(part.paint?.opacity, 1),
      visible: true,
      tintSlot: part.tintSlot,
      paint: part.paint,
    };
  }

  for (const binding of definition.animations || []) {
    const sampled = parts[binding.partId];
    if (!sampled) continue;
    applyBinding(sampled, binding, inputValue(binding.input, context));
  }

  return { context, parts };
}

function normalizedPartSet(includeParts) {
  if (includeParts == null) return null;
  if (includeParts instanceof Set) return includeParts;
  if (typeof includeParts === "string") return new Set([includeParts]);
  return new Set(includeParts);
}

function applyBinding(sampled, binding, input) {
  if (binding.property === "visible") {
    sampled.visible = Boolean(input);
    return;
  }
  if (binding.property === "tintSlot") {
    if (input) sampled.tintSlot = String(input);
    return;
  }

  const value = numericInput(input) * binding.factor + binding.offset;
  if (!Number.isFinite(value)) return;
  if (TRANSFORM_PROPERTIES.has(binding.property)) {
    const key = binding.property.slice("transform.".length);
    sampled.transform[key] = sampled.transform[key] + value;
  } else if (LOCAL_TRANSFORM_PROPERTIES.has(binding.property)) {
    const key = binding.property === "transform.localX" ? "x" : "y";
    sampled.localOffset[key] = sampled.localOffset[key] + value;
  } else if (GEOMETRY_SCALE_PROPERTIES.has(binding.property)) {
    const key = binding.property === "geometry.scaleX" ? "x" : "y";
    sampled.geometryScale[key] = sampled.geometryScale[key] + value;
  } else if (binding.property === "alpha") {
    sampled.alpha = clamp01(value);
  }
}

function inputValue(input, context) {
  if (input === "setupVisual") return context.setupVisual?.prongFactor ?? 0;
  if (input === "vehicleMotion") return context.vehicleMotion?.activity ?? 0;
  return context[input];
}

function numericInput(value) {
  if (typeof value === "number") return value;
  if (typeof value === "boolean") return value ? 1 : 0;
  return 0;
}

function defaultSetupVisual(entity) {
  if (entity.setupState === SETUP.DEPLOYED) return { prongFactor: 1, barrel: entity.state !== STATE.MOVE };
  if (entity.setupState === SETUP.TEARING_DOWN) return { prongFactor: 1, barrel: false };
  return { prongFactor: 0, barrel: false };
}

function defaultVehicleMotion() {
  return { leftPhase: 0, rightPhase: 0, leftDir: 0, rightDir: 0, activity: 0, lowOil: false, oilStarved: false };
}

function visualWeaponFacing(kind, facing, weaponFacing, deploy) {
  const t = smoothstep01(deploy);
  if (kind === KIND.RIFLEMAN) return facing - 0.2;
  if (kind === KIND.MACHINE_GUNNER) return angleLerp(facing + 0.86, weaponFacing, t);
  if (kind === KIND.ANTI_TANK_GUN) return angleLerp(facing, weaponFacing, t);
  if (kind === KIND.MORTAR_TEAM) return angleLerp(facing, weaponFacing + Math.PI / 4 - Math.PI * 0.22, t);
  return weaponFacing;
}

function visualCarriageFacing(kind, facing, weaponFacing, deploy, fallback) {
  if (kind === KIND.ARTILLERY) return deploy > 0.02 ? weaponFacing : facing;
  if (kind === KIND.ANTI_TANK_GUN || kind === KIND.MORTAR_TEAM) return fallback;
  return facing;
}

function rigAnchorRotation(kind, anchorName, context) {
  if (anchorName === "muzzle" || anchorName === "coaxMuzzle") {
    return context.weaponVisualFacing;
  }
  if (kind === KIND.TANK && anchorName === "turret") return context.weaponVisualFacing;
  return context.facing;
}

function rigAnchorPartId(kind, anchorName) {
  if (kind !== KIND.TANK) return null;
  if (anchorName === "muzzle") return "part.barrel";
  if (anchorName === "coaxMuzzle") return "part.coaxBarrel";
  if (anchorName === "turret") return "part.turret";
  return null;
}

function transformedRigPartPoint(entity, point, part) {
  const rotation = finite(part.transform?.rotation, 0);
  const c = Math.cos(rotation);
  const s = Math.sin(rotation);
  const localOffset = rotateLocalOffset(part.localOffset, rotation);
  const localX = (point.x * finite(part.geometryScale?.x, 1) - finite(part.pivot?.x, 0))
    * finite(part.transform?.scaleX, 1);
  const localY = (point.y * finite(part.geometryScale?.y, 1) - finite(part.pivot?.y, 0))
    * finite(part.transform?.scaleY, 1);
  const partX = finite(part.transform?.x, 0) + localOffset.x;
  const partY = finite(part.transform?.y, 0) + localOffset.y;
  return {
    x: entity.x + partX + localX * c - localY * s,
    y: entity.y + partY + localX * s + localY * c,
  };
}

function rotateLocalOffset(offset, rotation) {
  const x = finite(offset?.x, 0);
  const y = finite(offset?.y, 0);
  if (x === 0 && y === 0) return { x: 0, y: 0 };
  const c = Math.cos(rotation);
  const s = Math.sin(rotation);
  return { x: x * c - y * s, y: x * s + y * c };
}

function isBusy(entity) {
  return (
    (entity.kind === KIND.WORKER || entity.kind === KIND.GOLEM) &&
    (entity.latchedNode || entity.state === STATE.BUILD)
  );
}

function finite(value, fallback) {
  return Number.isFinite(value) ? value : fallback;
}
