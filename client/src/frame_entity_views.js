const EMPTY_ARRAY = Object.freeze([]);

/**
 * Build the short-lived entity arrays shared by one requestAnimationFrame pass.
 *
 * The returned object is intentionally frame-local: consumers should read the
 * arrays during the current frame and then discard the object.
 *
 * @param {object} state GameState-compatible browser model.
 * @param {{alpha?:number}=} options
 * @returns {{
 *   alpha:number,
 *   interpolatedEntities:Array<object>,
 *   currentEntities:Array<object>,
 *   authoritativeEntities:Array<object>,
 *   fogSourceEntities:Array<object>,
 *   selectedEntities:Array<object>,
 *   debug:{entityVariantBuildCalls:number,entityTraversals:number,entitiesInterpolatedCalls:number,selectedEntitiesCalls:number}
 * }}
 */
export function buildFrameEntityViews(state, { alpha = 1 } = {}) {
  const calls = {
    entityVariantBuilds: 0,
    entityTraversals: 0,
    entitiesInterpolated: 0,
    selectedEntities: 0,
  };
  const frameAlpha = normalizeAlpha(alpha);
  const variants = entityVariants(state, frameAlpha, calls);
  const { interpolatedEntities, currentEntities, authoritativeEntities } = variants;
  const selectedEntities = selectedEntitiesForFrame(state, calls);
  return Object.freeze({
    version: 1,
    alpha: frameAlpha,
    interpolatedEntities,
    currentEntities,
    authoritativeEntities,
    fogSourceEntities: fogSourceEntitiesForState(state, authoritativeEntities),
    selectedEntities,
    debug: Object.freeze({
      entityVariantBuildCalls: calls.entityVariantBuilds,
      entityTraversals: calls.entityTraversals,
      entitiesInterpolatedCalls: calls.entitiesInterpolated,
      selectedEntitiesCalls: calls.selectedEntities,
    }),
  });
}

function entityVariants(state, alpha, calls) {
  if (typeof state?.entityVariants === "function") {
    calls.entityVariantBuilds += 1;
    const value = state.entityVariants(alpha) || {};
    calls.entityTraversals += Number.isInteger(value.entityTraversals)
      ? Math.max(0, value.entityTraversals)
      : 0;
    return {
      interpolatedEntities: arrayOrEmpty(value.interpolatedEntities),
      currentEntities: arrayOrEmpty(value.currentEntities),
      authoritativeEntities: arrayOrEmpty(value.authoritativeEntities),
    };
  }
  const interpolatedEntities = entitiesInterpolated(state, alpha, undefined, calls);
  const currentEntities = alpha === 1
    ? interpolatedEntities
    : entitiesInterpolated(state, 1, undefined, calls);
  const authoritativeEntities = entitiesInterpolated(
    state,
    1,
    { includePrediction: false },
    calls,
  );
  return { interpolatedEntities, currentEntities, authoritativeEntities };
}

function arrayOrEmpty(value) {
  return Array.isArray(value) ? value : EMPTY_ARRAY;
}

function entitiesInterpolated(state, alpha, options, calls) {
  if (typeof state?.entitiesInterpolated !== "function") return EMPTY_ARRAY;
  calls.entitiesInterpolated += 1;
  const value = options === undefined
    ? state.entitiesInterpolated(alpha)
    : state.entitiesInterpolated(alpha, options);
  return Array.isArray(value) ? value : EMPTY_ARRAY;
}

function selectedEntitiesForFrame(state, calls) {
  if (typeof state?.selectedEntities !== "function") return EMPTY_ARRAY;
  calls.selectedEntities += 1;
  const value = state.selectedEntities();
  return Array.isArray(value) ? value : EMPTY_ARRAY;
}

function fogSourceEntitiesForState(state, entities) {
  const all = (Array.isArray(entities) ? entities : EMPTY_ARRAY)
    .filter((entity) => entity && !entity.shotReveal && !entity.visionOnly);
  if (state?.spectator) return all.filter((entity) => entity.owner !== 0);
  const playerId = state?.playerId;
  return all.filter((entity) => entity.owner === playerId);
}

function normalizeAlpha(value) {
  const n = Number(value);
  if (!Number.isFinite(n)) return 1;
  if (n <= 0) return 0;
  if (n >= 1) return 1;
  return n;
}
