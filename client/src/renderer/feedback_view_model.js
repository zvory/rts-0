const EMPTY_ARRAY = Object.freeze([]);

/**
 * Build the narrow renderer feedback read model.
 *
 * The renderer feedback layer consumes this shape instead of the full GameState
 * so placement, previews, command markers, selected-entity overlays, and
 * transient effect markers stay behind one explicit boundary.
 *
 * @param {object} state GameState-compatible browser model.
 * @param {{clientIntent?:object|null, entities?:Array<object>, selectedEntities?:Array<object>, now?:number}=} options
 * @returns {object}
 */
export function buildRendererFeedbackView(
  state,
  { clientIntent = null, entities = EMPTY_ARRAY, selectedEntities = null, now = defaultNow() } = {},
) {
  const selected = Array.isArray(selectedEntities)
    ? selectedEntities
    : typeof state?.selectedEntities === "function"
    ? arrayOrEmpty(state.selectedEntities())
    : EMPTY_ARRAY;
  const entityLookup = buildEntityLookup(entities, selected);
  const intent = clientIntent || null;

  const commandFeedback = liveArray(intent, "liveCommandFeedback", now);
  const smokeCanisters = liveArray(state, "liveSmokeCanisters", now);
  const mortarLaunches = liveArray(state, "liveMortarLaunches", now);
  const mortarShells = liveArray(state, "liveMortarShells", now);
  const mortarTargets = liveArray(state, "liveMortarTargets", now);
  const mortarImpacts = liveArray(state, "liveMortarImpacts", now);
  const artilleryTargets = liveArray(state, "liveArtilleryTargets", now);
  const artilleryLaunches = liveArray(state, "liveArtilleryLaunches", now);
  const artilleryImpacts = liveArray(state, "liveArtilleryImpacts", now);
  const muzzleFlashes = liveArray(state, "liveMuzzleFlashes", now);

  return {
    playerId: state?.playerId,
    map: state?.map || null,
    placement: intent?.placement || null,
    commandFeedback,
    selectedEntities: () => selected,
    debugPathOverlaysEnabled: !!state?.debugPathOverlaysEnabled,
    showAllDebugPathOverlays: !!state?.showAllDebugPathOverlays,
    antiTankGunSetupPreview: intent?.antiTankGunSetupPreview || null,
    abilityTargetPreview: intent?.abilityTargetPreview || null,
    abilityObjects: arrayOrEmpty(state?.abilityObjects),
    smokes: arrayOrEmpty(state?.smokes),
    smokeCanisters,
    mortarLaunches,
    mortarShells,
    mortarTargets,
    mortarImpacts,
    artilleryTargets,
    artilleryLaunches,
    artilleryImpacts,
    resourceMiningPreview: intent?.resourceMiningPreview || null,
    muzzleFlashes,
    liveCommandFeedback: () => commandFeedback,
    liveSmokeCanisters: () => smokeCanisters,
    liveMortarLaunches: () => mortarLaunches,
    liveMortarShells: () => mortarShells,
    liveMortarTargets: () => mortarTargets,
    liveMortarImpacts: () => mortarImpacts,
    liveArtilleryTargets: () => artilleryTargets,
    liveArtilleryLaunches: () => artilleryLaunches,
    liveArtilleryImpacts: () => artilleryImpacts,
    liveMuzzleFlashes: () => muzzleFlashes,
    entityById(id) {
      return entityLookup.get(id);
    },
    isOwnOwner(owner) {
      if (typeof state?.isOwnOwner === "function") return state.isOwnOwner(owner);
      return Number(owner) === state?.playerId;
    },
    isAllyOwner(owner) {
      if (typeof state?.isAllyOwner === "function") return state.isAllyOwner(owner);
      return false;
    },
  };
}

function liveArray(state, method, now) {
  return typeof state?.[method] === "function" ? arrayOrEmpty(state[method](now)) : EMPTY_ARRAY;
}

function arrayOrEmpty(value) {
  return Array.isArray(value) ? value : EMPTY_ARRAY;
}

function buildEntityLookup(entities, selectedEntities) {
  const lookup = new Map();
  addEntities(lookup, entities);
  addEntities(lookup, selectedEntities);
  return lookup;
}

function addEntities(lookup, entities) {
  if (!Array.isArray(entities)) return;
  for (const entity of entities) {
    if (entity && entity.id != null) lookup.set(entity.id, entity);
  }
}

function defaultNow() {
  return typeof performance !== "undefined" && typeof performance.now === "function"
    ? performance.now()
    : Date.now();
}
