import { KIND, SETUP } from "../protocol.js";

const EMPTY_ARRAY = Object.freeze([]);

/**
 * Build the narrow renderer feedback read model.
 *
 * The renderer feedback layer consumes this shape instead of the full GameState
 * so placement, previews, command markers, selected-entity overlays, and
 * transient effect markers stay behind one explicit boundary.
 *
 * @param {object} state GameState-compatible browser model.
 * @param {{clientIntent?:object|null, controlPolicy?:object|null, previewSurface?:string|null, entities?:Array<object>, selectedEntities?:Array<object>, rememberedEnemyAntiTankGunThreats?:Array<object>, observerView?:object|null, now?:number}=} options
 * @returns {object}
 */
export function buildRendererFeedbackView(
  state,
  {
    clientIntent = null,
    controlPolicy = null,
    previewSurface = null,
    entities = EMPTY_ARRAY,
    selectedEntities = null,
    rememberedEnemyAntiTankGunThreats = EMPTY_ARRAY,
    observerView = null,
    now = defaultNow(),
  } = {},
) {
  const selectedStateEntities = Array.isArray(selectedEntities)
    ? selectedEntities
    : typeof state?.selectedEntities === "function"
    ? arrayOrEmpty(state.selectedEntities())
    : EMPTY_ARRAY;
  const selected = selectRenderedEntities(entities, selectedStateEntities);
  const entityLookup = buildEntityLookup(entities, selected);
  const enemyAntiTankGunThreats = visibleEnemyAntiTankGunThreats(state, entities, {
    allowSpectator: observerView?.mode === "player",
    rememberedThreats: rememberedEnemyAntiTankGunThreats,
    observerView,
  });
  const intent = clientIntent || null;
  const controlOwner = buildControlOwnerReadModel(state, selected, controlPolicy);

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
    placement: previewSurface ? null : intent?.placement || null,
    formationMovePreview: previewSurface ? null : intent?.formationMovePreview || null,
    labToolPreview: previewSurface ? null : intent?.labToolPreview || null,
    labRuler: labRulerView(intent?.labRuler, !!previewSurface),
    commandFeedback,
    attackTargetPreview: previewSurface ? null : intent?.attackTargetPreview || null,
    selectedEntities: () => selected,
    enemyAntiTankGunThreats: () => enemyAntiTankGunThreats,
    showUnitRangesEnabled: state?.showUnitRangesEnabled !== false,
    showSelectedFieldOfFireEnabled: controlOwner.showSelectedFieldOfFireEnabled,
    debugPathOverlaysEnabled: !!state?.debugPathOverlaysEnabled,
    showAllDebugPathOverlays: !!state?.showAllDebugPathOverlays,
    antiTankGunSetupPreview: previewSurface && intent?.antiTankGunSetupPreview?.source !== previewSurface
      ? null : intent?.antiTankGunSetupPreview || null,
    abilityTargetPreview: previewSurface ? null : intent?.abilityTargetPreview || null,
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
    resourceMiningPreview: previewSurface ? null : intent?.resourceMiningPreview || null,
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

function visibleEnemyAntiTankGunThreats(
  state,
  entities,
  { allowSpectator = false, rememberedThreats = EMPTY_ARRAY, observerView = null } = {},
) {
  const perspectivePlayerId = resolveThreatPerspectivePlayerId({
    players: state?.players,
    playerId: state?.playerId,
    observerView,
    allowObserverPerspective: allowSpectator,
  });
  if (
    !Array.isArray(entities) ||
    (state?.spectator && !allowSpectator) ||
    perspectivePlayerId == null
  ) return EMPTY_ARRAY;
  const liveThreats = entities.filter((entity) =>
    entity?.kind === KIND.ANTI_TANK_GUN &&
    entity?.setupState === SETUP.DEPLOYED &&
    isThreatEnemyOwner(state?.players, perspectivePlayerId, entity?.owner));
  const liveIds = new Set(liveThreats.map((entity) => Number(entity.id)));
  const staleThreats = arrayOrEmpty(rememberedThreats).filter((memory) =>
    !liveIds.has(Number(memory?.id)) &&
    isThreatEnemyOwner(state?.players, perspectivePlayerId, memory?.owner));
  if (liveThreats.length === 0 && staleThreats.length === 0) return EMPTY_ARRAY;
  return [
    ...liveThreats.map((entity) => ({ ...entity, threatMemory: false })),
    ...staleThreats.map((memory) => ({ ...memory, threatMemory: true })),
  ];
}

function resolveThreatPerspectivePlayerId({
  players = EMPTY_ARRAY,
  playerId = null,
  observerView = null,
  allowObserverPerspective = false,
} = {}) {
  if (allowObserverPerspective && observerView?.mode === "player") {
    const observedPlayerId = normalizeOwner(observerView.playerId);
    if (teamIdForPlayer(players, observedPlayerId) != null) return observedPlayerId;
  }
  const localPlayerId = normalizeOwner(playerId);
  if (teamIdForPlayer(players, localPlayerId) != null) return localPlayerId;
  return null;
}

function isThreatEnemyOwner(players, perspectivePlayerId, owner) {
  const ownerId = normalizeOwner(owner);
  if (ownerId == null || ownerId === perspectivePlayerId) return false;
  const ownTeam = teamIdForPlayer(players, perspectivePlayerId);
  const ownerTeam = teamIdForPlayer(players, ownerId);
  return ownTeam != null && ownerTeam != null && ownTeam !== ownerTeam;
}

function labRulerView(ruler, suppressCursor) {
  if (!ruler) return null;
  const view = {
    start: ruler.start || null,
    end: ruler.end || null,
    cursor: suppressCursor ? null : ruler.cursor || null,
  };
  return view.start || view.end || view.cursor ? view : null;
}

function buildControlOwnerReadModel(state, selected, policy = null) {
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

function selectRenderedEntities(entities, selectedEntities) {
  if (!Array.isArray(entities) || entities.length === 0 || selectedEntities.length === 0) {
    return selectedEntities;
  }
  const renderedById = new Map();
  addEntities(renderedById, entities);
  let remapped = false;
  const selected = selectedEntities.map((entity) => {
    const rendered = renderedById.get(entity?.id);
    if (!rendered) return entity;
    if (rendered !== entity) remapped = true;
    return rendered;
  });
  return remapped ? selected : selectedEntities;
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
  if (!Array.isArray(players)) return null;
  return players.find((player) => Number(player?.id) === Number(id))?.teamId ?? null;
}

function defaultNow() {
  return typeof performance !== "undefined" && typeof performance.now === "function"
    ? performance.now()
    : Date.now();
}
