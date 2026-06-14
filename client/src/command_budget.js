import {
  BASE_COMMAND_SUPPLY_CAP,
  COMMAND_CAR_SUPPLY_CAP_BONUS,
  STATS,
} from "./config.js";
import { KIND, STATE, isUnit } from "./protocol.js";

export const COMMAND_BUDGET_OVERFLOW_NOTICE = "Command supply exceeded";

export function commandBudgetForEntities(entities) {
  let used = 0;
  let cap = BASE_COMMAND_SUPPLY_CAP;
  for (const entity of entities || []) {
    if (!entity || !isUnit(entity.kind)) continue;
    used += commandWeight(entity.kind);
    if (entity.kind === KIND.COMMAND_CAR) cap += COMMAND_CAR_SUPPLY_CAP_BONUS;
  }
  return { used, cap, over: used > cap };
}

export function commandWithinBudget(state, command) {
  const units = Array.isArray(command?.units) ? command.units : null;
  if (!units) return { ok: true, used: 0, cap: BASE_COMMAND_SUPPLY_CAP };

  const seen = new Set();
  const entities = [];
  for (const id of units) {
    if (!Number.isInteger(id) || seen.has(id)) continue;
    seen.add(id);
    const entity = typeof state?.entityById === "function" ? state.entityById(id) : null;
    if (!entity || !state?.isOwnOwner?.(entity.owner) || !isUnit(entity.kind) || entity.state === STATE.CONSTRUCT) {
      continue;
    }
    entities.push(entity);
  }

  const budget = commandBudgetForEntities(entities);
  return { ok: !budget.over, ...budget };
}

function commandWeight(kind) {
  const supply = STATS[kind]?.supply;
  return Number.isFinite(supply) && supply > 0 ? supply : 1;
}
