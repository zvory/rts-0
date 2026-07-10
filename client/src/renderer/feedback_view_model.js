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
  const missToasts = liveArray(state, "liveMissToasts", now);

  return {
    playerId: state?.playerId,
    feedbackOwnerId: controlOwner.feedbackOwnerId,
    feedbackOwnerIds: controlOwner.feedbackOwnerIds,
    issueAsOwnerId: controlOwner.issueAsOwnerId,
    map: state?.map || null,
    placement: intent?.placement || null,
    labToolPreview: intent?.labToolPreview || null,
    labMapDraftOverlay: intent?.labMapDraftOverlay || null,
    commandFeedback,
    attackTargetPreview: intent?.attackTargetPreview || null,
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
    missToasts,
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
    liveMissToasts: () => missToasts,
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
      if (controlOwner.feedbackOwnerIds.length > 0) {
        return controlOwner.feedbackOwnerIds.some((feedbackOwnerId) =>
          isAllyForPlayer(state?.players, feedbackOwnerId, owner));
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
  const labFeedbackOwnerIds = isLabPolicy ? labFeedbackOwners(policy, selected) : EMPTY_ARRAY;
  const feedbackOwnerIds = labFeedbackOwnerIds.length > 0
    ? labFeedbackOwnerIds
    : feedbackOwnerId == null
      ? EMPTY_ARRAY
      : [feedbackOwnerId];
  const feedbackOwnerSet = new Set(feedbackOwnerIds);

  return {
    feedbackOwnerId,
    feedbackOwnerIds,
    issueAsOwnerId,
    showSelectedFieldOfFireEnabled: isLabPolicy && feedbackOwnerIds.length > 0,
    canControlOwner(owner) {
      if (typeof policy?.canControlOwner === "function") return !!policy.canControlOwner(owner, state);
      return feedbackOwnerId != null && Number(owner) === feedbackOwnerId;
    },
    isFeedbackOwner(owner) {
      return feedbackOwnerSet.has(Number(owner));
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

function labFeedbackOwners(policy, selected) {
  const owners = typeof policy?.selectedOwners === "function"
    ? policy.selectedOwners(selected)
    : selectedOwners(selected);
  return owners
    .map(normalizeOwner)
    .filter((owner) => owner != null && labCanIssueAs(policy, owner));
}

function selectedOwners(selected) {
  const owners = new Set();
  for (const entity of selected || []) {
    const owner = normalizeOwner(entity?.owner);
    if (owner != null) owners.add(owner);
  }
  return Array.from(owners).sort((a, b) => a - b);
}

function labCanIssueAs(policy, owner) {
  return typeof policy?.canIssueAs === "function" ? !!policy.canIssueAs(owner) : owner != null;
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
