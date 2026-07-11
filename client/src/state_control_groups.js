import { admitSelectionIds } from "./command_budget.js";
import { KIND, isBuilding, isUnit } from "./protocol.js";

export function admitControlGroupIds(state, ids, { baseIds = [], entityById = null } = {}) {
  if (state?.controlPolicy?.kind === "lab") {
    return admitLabControlGroupIds(state, ids, { baseIds, entityById });
  }
  const base = ownControllableIds(state, baseIds, entityById);
  const candidates = ownControllableIds(state, ids, entityById);
  return admitSelectionIds(state, candidates, { baseIds: base, entityById });
}

function ownControllableIds(state, ids, entityById = null) {
  const resolveEntity = typeof entityById === "function" ? entityById : (id) => state?._curById?.get(id);
  const out = [];
  const seen = new Set();
  for (const id of ids || []) {
    if (seen.has(id)) continue;
    const entity = resolveEntity(id);
    if (!entity || entity.owner !== state.playerId) continue;
    if (entity.kind === KIND.SCOUT_PLANE) continue;
    if (!isUnit(entity.kind) && !isBuilding(entity.kind)) continue;
    out.push(id);
    seen.add(id);
  }
  return out;
}

function admitLabControlGroupIds(state, ids, { baseIds = [], entityById = null } = {}) {
  const baseEntities = labControlGroupEntities(state, baseIds, new Set(), entityById);
  const baseOwner = singleOwner(baseEntities);
  if (baseEntities.length > 0 && baseOwner == null) {
    return admitSelectionIds(state, [], { baseIds: [], entityById });
  }

  const baseSeen = new Set(baseEntities.map((entity) => entity.id));
  const candidateEntities = labControlGroupEntities(state, ids, baseSeen, entityById);
  const candidateOwner = singleOwner(candidateEntities);
  let owner = baseOwner ?? candidateOwner;
  if (owner == null && candidateEntities.length > 0) {
    return admitSelectionIds(state, [], {
      baseIds: baseEntities.map((entity) => entity.id),
      entityById,
    });
  }
  if (owner != null && typeof state.controlPolicy?.canIssueAs === "function" && !state.controlPolicy.canIssueAs(owner)) {
    return admitSelectionIds(state, [], { baseIds: [], entityById });
  }

  const base = owner == null
    ? []
    : baseEntities.filter((entity) => Number(entity.owner) === owner).map((entity) => entity.id);
  const candidates = owner == null
    ? []
    : candidateEntities.filter((entity) => Number(entity.owner) === owner).map((entity) => entity.id);
  return admitSelectionIds(state, candidates, { baseIds: base, entityById });
}

function labControlGroupEntities(state, ids, seen = new Set(), entityById = null) {
  const resolveEntity = typeof entityById === "function" ? entityById : (id) => state?._curById?.get(id);
  const out = [];
  for (const id of ids || []) {
    if (!Number.isInteger(id) || seen.has(id)) continue;
    const entity = resolveEntity(id);
    const owner = Number(entity?.owner);
    if (!entity || !Number.isInteger(owner) || owner <= 0) continue;
    if (entity.shotReveal || entity.visionOnly) continue;
    if (!isUnit(entity.kind) && !isBuilding(entity.kind)) continue;
    out.push(entity);
    seen.add(id);
  }
  return out;
}

function singleOwner(entities) {
  if (!Array.isArray(entities) || entities.length === 0) return null;
  let owner = null;
  for (const entity of entities) {
    const next = Number(entity?.owner);
    if (!Number.isInteger(next) || next <= 0) return null;
    if (owner == null) owner = next;
    else if (owner !== next) return null;
  }
  return owner;
}
