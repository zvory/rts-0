import {
  BASE_COMMAND_SUPPLY_CAP,
  COMMAND_CAR_SUPPLY_CAP_BONUS,
  STATS,
} from "./config.js";
import { KIND, STATE, isBuilding, isUnit } from "./protocol.js";

export const COMMAND_BUDGET_OVERFLOW_NOTICE = "Command supply exceeded";

export function commandBudgetForEntities(entities) {
  let used = 0;
  let cap = BASE_COMMAND_SUPPLY_CAP;
  for (const entity of entities || []) {
    if (!selectableCountsForCommandBudget(entity)) continue;
    const weight = commandWeight(entity.kind);
    used += weight;
    if (entity.kind === KIND.COMMAND_CAR) cap += commandCarCapBonus(weight);
  }
  return { used, cap, over: used > cap };
}

export function admitSelectionIds(state, ids, { baseIds = [] } = {}) {
  const base = uniqueLiveSelectionEntities(state, baseIds);
  const candidates = uniqueLiveSelectionEntities(state, ids, new Set(base.map((entity) => entity.id)));

  if (!shouldBudgetSelection(state, base, candidates)) {
    return {
      ids: base.concat(candidates).map((entity) => entity.id),
      overflow: false,
      ...commandBudgetForEntities(base.concat(candidates)),
    };
  }

  const admitted = base.slice();
  const admittedIds = new Set(admitted.map((entity) => entity.id));
  const commandCars = candidates.filter((entity) => entity.kind === KIND.COMMAND_CAR);
  const orderedCandidates = commandCars.concat(
    candidates.filter((entity) => entity.kind !== KIND.COMMAND_CAR),
  );
  let budget = commandBudgetForEntities(admitted);
  let overflow = false;

  for (const entity of orderedCandidates) {
    if (admittedIds.has(entity.id)) continue;
    const weight = commandWeight(entity.kind);
    const nextCap = budget.cap + (entity.kind === KIND.COMMAND_CAR ? commandCarCapBonus(weight) : 0);
    const nextUsed = budget.used + weight;
    if (nextUsed <= nextCap) {
      admitted.push(entity);
      admittedIds.add(entity.id);
      budget = { used: nextUsed, cap: nextCap, over: false };
    } else {
      overflow = true;
    }
  }

  return { ids: admitted.map((entity) => entity.id), overflow, ...budget };
}

export function commandWithinBudget(state, command, { ownerId = null, ignoreCommandLimits = false } = {}) {
  if (ignoreCommandLimits) return { ok: true, used: 0, cap: Number.POSITIVE_INFINITY };

  const units = Array.isArray(command?.units) ? command.units : null;
  if (!units) return { ok: true, used: 0, cap: BASE_COMMAND_SUPPLY_CAP };

  const expectedOwner = Number(ownerId);
  const hasOwnerOverride = Number.isInteger(expectedOwner) && expectedOwner > 0;
  const seen = new Set();
  const entities = [];
  for (const id of units) {
    if (!Number.isInteger(id) || seen.has(id)) continue;
    seen.add(id);
    const entity = typeof state?.entityById === "function" ? state.entityById(id) : null;
    const ownerMatches = hasOwnerOverride
      ? Number(entity?.owner) === expectedOwner
      : !!state?.isOwnOwner?.(entity?.owner);
    if (!entity || !ownerMatches || !isUnit(entity.kind) || entity.state === STATE.CONSTRUCT) {
      continue;
    }
    entities.push(entity);
  }

  const budget = commandBudgetForEntities(entities);
  return { ok: !budget.over, ...budget };
}

export function commandWeight(kind) {
  const supply = STATS[kind]?.supply;
  return Number.isFinite(supply) && supply > 0 ? supply : 1;
}

function commandCarCapBonus(weight) {
  return COMMAND_CAR_SUPPLY_CAP_BONUS + weight;
}

function selectableCountsForCommandBudget(entity) {
  return !!entity && (isUnit(entity.kind) || isBuilding(entity.kind));
}

function uniqueLiveSelectionEntities(state, ids, seen = new Set()) {
  const out = [];
  for (const id of ids || []) {
    if (!Number.isInteger(id) || seen.has(id)) continue;
    const entity = typeof state?.entityById === "function" ? state.entityById(id) : null;
    if (!entity || entity.shotReveal || entity.visionOnly) continue;
    if (entity.kind === KIND.SCOUT_PLANE && !allowsScoutPlaneInspection(state)) continue;
    seen.add(id);
    out.push(entity);
  }
  return out;
}

function allowsScoutPlaneInspection(state) {
  return state?.controlPolicy?.kind === "lab" || !!state?.spectator;
}

function shouldBudgetSelection(state, base, candidates) {
  if (!state) return false;
  if (state.controlPolicy?.kind === "lab") {
    return false;
  }
  if (state.spectator) return false;
  const all = base.concat(candidates);
  if (all.length === 0) return true;
  return all.every(
    (entity) =>
      selectableCountsForCommandBudget(entity) &&
      typeof state.isOwnOwner === "function" &&
      state.isOwnOwner(entity.owner),
  );
}
