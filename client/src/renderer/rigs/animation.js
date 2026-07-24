import { STATS } from "../../config.js";
import { KIND, SETUP, STATE } from "../../protocol.js";
import { angleLerp, clamp01, hexToInt, polar, recoilVector, smoothstep01, tankBodyVisual, weaponRecoilOffset } from "../shared.js";
import { normalizedPartSet } from "./part_selection.js";

const RIG_CONTEXT_READY = Symbol("rtsRigContextReady");
const RIG_ANIMATION_STAGE = Symbol("rtsRigAnimationStage");
const ARTILLERY_VISUAL_SCALE = 0.75;

const BINDING_VISIBLE = 0;
const BINDING_TINT_SLOT = 1;
const BINDING_TRANSFORM_X = 2;
const BINDING_TRANSFORM_Y = 3;
const BINDING_TRANSFORM_ROTATION = 4;
const BINDING_TRANSFORM_SCALE_X = 5;
const BINDING_TRANSFORM_SCALE_Y = 6;
const BINDING_LOCAL_X = 7;
const BINDING_LOCAL_Y = 8;
const BINDING_GEOMETRY_SCALE_X = 9;
const BINDING_GEOMETRY_SCALE_Y = 10;
const BINDING_ALPHA = 11;

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
  const recoilWeaponKind = typeof state.weaponRecoilKind === "function"
    ? state.weaponRecoilKind(entity.id)
    : undefined;
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
    recoilWeaponKind,
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
    visualScale: entity.kind === KIND.ARTILLERY ? ARTILLERY_VISUAL_SCALE : 1,
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
  const visualScale = finite(context.visualScale, 1);
  return {
    x: entity.x + (anchor.x * c - anchor.y * s) * visualScale,
    y: entity.y + (anchor.x * s + anchor.y * c) * visualScale,
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
  const stage = createRigAnimationStage(definition, options);
  return sampleRigAnimationInto(stage, entity, renderContext);
}

/**
 * @typedef {object} AnimationStage
 * @property {object} definition Definition identity this stage was compiled from.
 * @property {object[]} partPlans Immutable baselines paired with reusable mutable part states.
 * @property {object[]} bindings Precompiled numeric animation operations.
 * @property {{context: object|null, parts: Object<string, object>}} output Mutable sampled result.
 */

/**
 * Compile a renderer-private staging object. Sampling overwrites the same part-state graph, so a
 * stage must only be sampled and consumed synchronously. The public sampleRigAnimation API creates
 * a fresh stage and therefore continues to return independently stable snapshots.
 *
 * @returns {AnimationStage}
 */
export function createRigAnimationStage(definition, options = {}) {
  const includeParts = normalizedPartSet(options.includeParts);
  const partPlans = [];
  const partIndexById = new Map();
  const parts = {};
  for (const part of definition.parts || []) {
    if (includeParts && !includeParts.has(part.id)) continue;
    const index = partPlans.length;
    const plan = {
      id: part.id,
      transformX: part.transform.x,
      transformY: part.transform.y,
      transformRotation: part.transform.rotation,
      transformScaleX: part.transform.scaleX,
      transformScaleY: part.transform.scaleY,
      pivotX: part.pivot.x,
      pivotY: part.pivot.y,
      alpha: finite(part.paint?.opacity, 1),
      tintSlot: part.tintSlot,
      paint: part.paint,
    };
    const state = {
      id: part.id,
      transform: {
        x: plan.transformX,
        y: plan.transformY,
        rotation: plan.transformRotation,
        scaleX: plan.transformScaleX,
        scaleY: plan.transformScaleY,
      },
      localOffset: { x: 0, y: 0 },
      geometryScale: { x: 1, y: 1 },
      pivot: { x: plan.pivotX, y: plan.pivotY },
      alpha: plan.alpha,
      visible: true,
      tintSlot: plan.tintSlot,
      paint: plan.paint,
    };
    plan.state = state;
    partPlans.push(plan);
    partIndexById.set(part.id, index);
    parts[part.id] = state;
  }

  const bindings = [];
  for (const binding of definition.animations || []) {
    const partIndex = partIndexById.get(binding.partId);
    if (partIndex === undefined) continue;
    bindings.push({
      partIndex,
      operation: bindingOperation(binding.property),
      input: binding.input,
      factor: binding.factor,
      offset: binding.offset,
    });
  }

  const output = { context: null, parts };
  return {
    [RIG_ANIMATION_STAGE]: true,
    definition,
    partPlans,
    bindings,
    output,
  };
}

export function sampleRigAnimationInto(stage, entity, renderContext = {}) {
  if (!stage?.[RIG_ANIMATION_STAGE]) {
    throw new TypeError("sampleRigAnimationInto requires a rig animation stage");
  }
  const inputContext = renderContext || {};
  const context = inputContext[RIG_CONTEXT_READY]
    ? inputContext
    : {
      ...createRigRenderContext(entity, { now: inputContext.now ?? 0 }),
      ...inputContext,
    };
  for (const plan of stage.partPlans) {
    const sampled = plan.state;
    sampled.transform.x = plan.transformX;
    sampled.transform.y = plan.transformY;
    sampled.transform.rotation = plan.transformRotation;
    sampled.transform.scaleX = plan.transformScaleX;
    sampled.transform.scaleY = plan.transformScaleY;
    sampled.localOffset.x = 0;
    sampled.localOffset.y = 0;
    sampled.geometryScale.x = 1;
    sampled.geometryScale.y = 1;
    sampled.alpha = plan.alpha;
    sampled.visible = true;
    sampled.tintSlot = plan.tintSlot;
  }

  for (const binding of stage.bindings) {
    const sampled = stage.partPlans[binding.partIndex].state;
    applyBinding(sampled, binding, inputValue(binding.input, context));
  }

  stage.output.context = context;
  return stage.output;
}

function applyBinding(sampled, binding, input) {
  if (binding.operation === BINDING_VISIBLE) {
    sampled.visible = Boolean(input);
    return;
  }
  if (binding.operation === BINDING_TINT_SLOT) {
    if (input) sampled.tintSlot = String(input);
    return;
  }

  const value = numericInput(input) * binding.factor + binding.offset;
  if (!Number.isFinite(value)) return;
  if (binding.operation === BINDING_TRANSFORM_X) sampled.transform.x += value;
  else if (binding.operation === BINDING_TRANSFORM_Y) sampled.transform.y += value;
  else if (binding.operation === BINDING_TRANSFORM_ROTATION) sampled.transform.rotation += value;
  else if (binding.operation === BINDING_TRANSFORM_SCALE_X) sampled.transform.scaleX += value;
  else if (binding.operation === BINDING_TRANSFORM_SCALE_Y) sampled.transform.scaleY += value;
  else if (binding.operation === BINDING_LOCAL_X) sampled.localOffset.x += value;
  else if (binding.operation === BINDING_LOCAL_Y) sampled.localOffset.y += value;
  else if (binding.operation === BINDING_GEOMETRY_SCALE_X) sampled.geometryScale.x += value;
  else if (binding.operation === BINDING_GEOMETRY_SCALE_Y) sampled.geometryScale.y += value;
  else if (binding.operation === BINDING_ALPHA) sampled.alpha = clamp01(value);
}

function bindingOperation(property) {
  if (property === "visible") return BINDING_VISIBLE;
  if (property === "tintSlot") return BINDING_TINT_SLOT;
  if (property === "transform.x") return BINDING_TRANSFORM_X;
  if (property === "transform.y") return BINDING_TRANSFORM_Y;
  if (property === "transform.rotation") return BINDING_TRANSFORM_ROTATION;
  if (property === "transform.scaleX") return BINDING_TRANSFORM_SCALE_X;
  if (property === "transform.scaleY") return BINDING_TRANSFORM_SCALE_Y;
  if (property === "transform.localX") return BINDING_LOCAL_X;
  if (property === "transform.localY") return BINDING_LOCAL_Y;
  if (property === "geometry.scaleX") return BINDING_GEOMETRY_SCALE_X;
  if (property === "geometry.scaleY") return BINDING_GEOMETRY_SCALE_Y;
  if (property === "alpha") return BINDING_ALPHA;
  return -1;
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
  if (kind === KIND.RIFLEMAN || kind === KIND.PANZERFAUST) return facing - 0.2;
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
