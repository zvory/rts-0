import { createRigAnimationStage } from "./animation.js";
import { pngAtlasRouteCoverage } from "./png_runtime.js";

const UNIT_RIG_POOL_NAMES = Object.freeze([
  "liveUnitRigShadows",
  "liveUnitRigs",
  "liveUnitRigOverlays",
  "liveUnitRigEffects",
  "liveShotRevealRigShadows",
  "liveShotRevealRigs",
  "liveShotRevealRigOverlays",
  "liveShotRevealRigEffects",
]);
const UNIT_RIG_POOL_BITS = Object.freeze(Object.fromEntries(
  UNIT_RIG_POOL_NAMES.map((poolName, index) => [poolName, 1 << index]),
));
const ALL_UNIT_RIG_POOLS_MASK = (1 << UNIT_RIG_POOL_NAMES.length) - 1;

const FRAME_STRIP_DRAW_PLAN_CACHE = new WeakMap();
const PNG_DRAW_PLAN_CACHE = new WeakMap();
const ANIMATION_STAGE_CACHE = new WeakMap();

/**
 * @typedef {object} DrawPlanStep
 * @property {"png"|"svg"} runtime
 * @property {import("./live_routing.js").LiveRigRoute} route
 */

/**
 * @typedef {object} DrawPlan
 * @property {ReadonlySet<string>} sampledParts Stable identity used to cache one mutable animation stage.
 * @property {readonly string[]} activePoolNames Complete pool set for synchronous per-frame reconciliation.
 * @property {readonly DrawPlanStep[]} [steps] Ordered PNG/SVG work; omitted for frame strips.
 * @property {import("./live_routing.js").LiveRigRoute|null} [shadowRoute]
 * @property {import("./live_routing.js").LiveRigRoute} [unitRoute]
 */

/**
 * Return the renderer-private mutable stage for a definition and stable part selection.
 * The caller must sample and consume it synchronously; a later sample overwrites its object graph.
 *
 * @param {object} definition
 * @param {ReadonlySet<string>} sampledParts
 * @returns {import("./animation.js").AnimationStage}
 */
export function animationStageFor(definition, sampledParts) {
  let byParts = ANIMATION_STAGE_CACHE.get(definition);
  if (!byParts) {
    byParts = new WeakMap();
    ANIMATION_STAGE_CACHE.set(definition, byParts);
  }
  let stage = byParts.get(sampledParts);
  if (!stage) {
    stage = createRigAnimationStage(definition, { includeParts: sampledParts });
    byParts.set(sampledParts, stage);
  }
  return stage;
}

/** @returns {DrawPlan} */
export function frameStripDrawPlanFor(routePlan) {
  const cached = FRAME_STRIP_DRAW_PLAN_CACHE.get(routePlan);
  if (cached) return cached;
  const shadowRoute = routePlan.shadowRoute;
  const unitRoute = routePlan.routes[1];
  const plan = Object.freeze({
    shadowRoute,
    unitRoute,
    sampledParts: new Set(shadowRoute?.parts || []),
    activePoolNames: Object.freeze([
      ...(shadowRoute ? [shadowRoute.poolName] : []),
      unitRoute.poolName,
    ]),
  });
  FRAME_STRIP_DRAW_PLAN_CACHE.set(routePlan, plan);
  return plan;
}

/** @returns {DrawPlan} */
export function pngDrawPlanFor(definition, atlas, routePlan) {
  let byAtlas = PNG_DRAW_PLAN_CACHE.get(definition);
  if (!byAtlas) {
    byAtlas = new WeakMap();
    PNG_DRAW_PLAN_CACHE.set(definition, byAtlas);
  }
  let byRoutePlan = byAtlas.get(atlas);
  if (!byRoutePlan) {
    byRoutePlan = new WeakMap();
    byAtlas.set(atlas, byRoutePlan);
  }
  const cached = byRoutePlan.get(routePlan);
  if (cached) return cached;

  const sampledParts = new Set();
  const activePoolNames = new Set();
  const steps = [];
  for (const route of routePlan.routes) {
    const coverage = pngAtlasRouteCoverage(definition, atlas, route);
    for (const partId of coverage.animationParts) sampledParts.add(partId);
    for (const partId of coverage.missingParts) sampledParts.add(partId);
    if (coverage.coveredParts.length > 0) {
      const pngRoute = coverage.missingParts.length === 0
        ? route
        : freezeRoute(route.poolName, route.layerName, coverage.coveredParts);
      activePoolNames.add(pngRoute.poolName);
      steps.push(Object.freeze({ runtime: "png", route: pngRoute }));
    }
    if (coverage.missingParts.length > 0) {
      const svgRoute = coverage.coveredParts.length > 0
        ? freezeRoute(routePlan.overlayPoolName, routePlan.overlayLayerName, coverage.missingParts)
        : freezeRoute(route.poolName, route.layerName, coverage.missingParts);
      activePoolNames.add(svgRoute.poolName);
      steps.push(Object.freeze({ runtime: "svg", route: svgRoute }));
    }
  }
  const plan = Object.freeze({
    sampledParts,
    activePoolNames: Object.freeze([...activePoolNames]),
    steps: Object.freeze(steps),
  });
  byRoutePlan.set(routePlan, plan);
  return plan;
}

/**
 * Preserve the original per-frame inactive-pool cleanup. Every known pool is checked each frame;
 * the mask only avoids Map probes for pools present in the active draw plan.
 */
export function reconcileActiveLiveRigPools(renderer, entityId, activePoolNames) {
  const inactiveMask = ALL_UNIT_RIG_POOLS_MASK & ~poolMask(activePoolNames);
  for (let index = 0; index < UNIT_RIG_POOL_NAMES.length; index += 1) {
    if ((inactiveMask & (1 << index)) === 0) continue;
    const poolName = UNIT_RIG_POOL_NAMES[index];
    const pool = renderer._liveRigPools?.[poolName];
    const instance = pool?.get?.(entityId);
    if (!instance) continue;
    instance.destroy?.();
    pool.delete(entityId);
    renderer._seen?.[poolName]?.delete?.(entityId);
    renderer._recordRenderDiagnostic?.(`renderer.rig.instance.destroyed.unused.${poolName}`);
  }
}

function freezeRoute(poolName, layerName, parts) {
  return Object.freeze({ poolName, layerName, parts });
}

function poolMask(poolNames) {
  let mask = 0;
  for (const poolName of poolNames || []) mask |= UNIT_RIG_POOL_BITS[poolName] || 0;
  return mask;
}
