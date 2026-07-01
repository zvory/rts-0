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
  const controlOwner = buildControlOwnerReadModel(state, selected);

  const commandFeedback = liveArray(intent, "liveCommandFeedback", now);
  const smokeCanisters = liveArray(state, "liveSmokeCanisters", now);
  const mortarLaunches = liveArray(state, "liveMortarLaunches", now);
  const mortarShells = liveArray(state, "liveMortarShells", now);
  const mortarTargets = liveArray(state, "liveMortarTargets", now);
  const mortarImpacts = liveArray(state, "liveMortarImpacts", now);
  const artilleryTargets = liveArray(state, "liveArtilleryTargets", now);
  const artilleryLaunches = liveArray(state, "liveArtilleryLaunches", now);
  const artilleryImpacts = liveArray(state, "liveArtilleryImpacts", now);
  const panzerfaustShots = liveArray(state, "livePanzerfaustShots", now);
  const panzerfaustImpacts = liveArray(state, "livePanzerfaustImpacts", now);
  const muzzleFlashes = liveArray(state, "liveMuzzleFlashes", now);

  return {
    playerId: state?.playerId,
    feedbackOwnerId: controlOwner.feedbackOwnerId,
    issueAsOwnerId: controlOwner.issueAsOwnerId,
    map: state?.map || null,
    placement: intent?.placement || null,
    commandFeedback,
    selectedEntities: () => selected,
    showUnitRangesEnabled: state?.showUnitRangesEnabled !== false,
    showSelectedFieldOfFireEnabled: controlOwner.showSelectedFieldOfFireEnabled,
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
    panzerfaustShots,
    panzerfaustImpacts,
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
    livePanzerfaustShots: () => panzerfaustShots,
    livePanzerfaustImpacts: () => panzerfaustImpacts,
    liveMuzzleFlashes: () => muzzleFlashes,
    entityById(id) {
      return entityLookup.get(id);
    },
    canControlOwner(owner) {
      return controlOwner.canControlOwner(owner);
    },
    isFeedbackOwner(owner) {
      return controlOwner.isFeedbackOwner(owner);
    },
    isOwnOwner(owner) {
      return controlOwner.isFeedbackOwner(owner);
    },
    isAllyOwner(owner) {
      if (controlOwner.feedbackOwnerId != null) {
        return isAllyForPlayer(state?.players, controlOwner.feedbackOwnerId, owner);
      }
      if (typeof state?.isAllyOwner === "function") return state.isAllyOwner(owner);
      return false;
    },
  };
}

function buildControlOwnerReadModel(state, selected) {
  const policy = state?.controlPolicy || null;
  const isLabPolicy = policy?.kind === "lab";
  const issueAsOwnerId = typeof policy?.issueAsOwnerForSelection === "function"
    ? normalizeOwner(policy.issueAsOwnerForSelection(selected))
    : null;
  const policyFeedbackOwner = typeof policy?.feedbackOwnerForSelection === "function"
    ? normalizeOwner(policy.feedbackOwnerForSelection(selected, state))
    : typeof policy?.feedbackOwner === "function"
      ? normalizeOwner(policy.feedbackOwner(state))
      : null;
  const fallbackOwner = defaultFeedbackOwner(state);
  const feedbackOwnerId = policyFeedbackOwner ?? fallbackOwner;

  return {
    feedbackOwnerId,
    issueAsOwnerId,
    showSelectedFieldOfFireEnabled: isLabPolicy && policyFeedbackOwner != null,
    canControlOwner(owner) {
      if (typeof policy?.canControlOwner === "function") return !!policy.canControlOwner(owner, state);
      return feedbackOwnerId != null && Number(owner) === feedbackOwnerId;
    },
    isFeedbackOwner(owner) {
      return feedbackOwnerId != null && Number(owner) === feedbackOwnerId;
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

function normalizeOwner(owner) {
  const value = Number(owner);
  return Number.isInteger(value) && value > 0 ? value : null;
}

function defaultFeedbackOwner(state) {
  if (state?.spectator) return null;
  return normalizeOwner(state?.playerId);
}

function isAllyForPlayer(players, playerId, owner) {
  const ownerId = Number(owner);
  if (!Array.isArray(players) || !Number.isInteger(ownerId) || ownerId === 0 || ownerId === playerId) {
    return false;
  }
  const ownTeam = teamIdForPlayer(players, playerId);
  const ownerTeam = teamIdForPlayer(players, ownerId);
  return ownTeam != null && ownerTeam != null && ownTeam !== 0 && ownTeam === ownerTeam;
}

function teamIdForPlayer(players, id) {
  return players.find((player) => Number(player?.id) === Number(id))?.teamId ?? null;
}

function defaultNow() {
  return typeof performance !== "undefined" && typeof performance.now === "function"
    ? performance.now()
    : Date.now();
}
