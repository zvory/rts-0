import { KIND, SETUP, STATE } from "../../protocol.js";
import { clamp01, hexToInt, weaponRecoilOffset } from "../shared.js";

const TRANSFORM_PROPERTIES = new Set([
  "transform.x",
  "transform.y",
  "transform.rotation",
  "transform.scaleX",
  "transform.scaleY",
]);

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
} = {}) {
  const facing = finite(entity.facing, 0);
  const weaponFacing = finite(entity.weaponFacing, facing);
  const recoilProgress = typeof state.weaponRecoil === "function"
    ? clamp01(state.weaponRecoil(entity.id, entity.kind, now))
    : clamp01(entity.recoilProgress ?? 0);
  const ownUnit = entity.owner === state.playerId;
  const oil = state.resources ? state.resources.oil : null;
  return {
    now,
    teamColor: colorByOwner.get(entity.owner) ?? hexToInt(entity.teamColor),
    recoilProgress,
    recoilPx: weaponRecoilOffset(entity.kind, recoilProgress),
    setupVisual: setupVisual ?? defaultSetupVisual(entity),
    vehicleMotion: vehicleMotion ?? defaultVehicleMotion(),
    selected: Boolean(selected),
    damaged: finite(entity.maxHp, 0) > 0 && finite(entity.hp, 0) < finite(entity.maxHp, 0),
    shotRevealAlpha: clamp01(shotRevealAlpha),
    visibility,
    mapTileSize: finite(map?.tileSize, 32),
    facing,
    weaponFacing,
    busy: isBusy(entity),
    breakthroughTicks: finite(entity.breakthroughTicks, 0),
    lowOil: ownUnit && typeof oil === "number" && oil > 0 && oil <= 5,
    oilStarved: ownUnit && oil === 0 && (entity.state === STATE.MOVE || entity.state === STATE.ATTACK),
  };
}

export function sampleRigAnimation(definition, entity, renderContext = {}) {
  const context = {
    ...createRigRenderContext(entity, { now: renderContext.now ?? 0 }),
    ...renderContext,
  };
  const parts = {};
  for (const part of definition.parts || []) {
    parts[part.id] = {
      id: part.id,
      transform: { ...part.transform },
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
  return { prongFactor: 0, barrel: false };
}

function defaultVehicleMotion() {
  return { leftPhase: 0, rightPhase: 0, leftDir: 0, rightDir: 0, activity: 0, lowOil: false, oilStarved: false };
}

function isBusy(entity) {
  return entity.kind === KIND.WORKER && (entity.latchedNode || entity.state === STATE.BUILD);
}

function finite(value, fallback) {
  return Number.isFinite(value) ? value : fallback;
}
