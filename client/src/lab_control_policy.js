import { DEFAULT_FACTION_ID, LAB_ROLE } from "./protocol.js";
import {
  COMMAND_BUDGET_OVERFLOW_NOTICE,
  commandWithinBudget,
} from "./command_budget.js";

export function createLabControlPolicy({ labClient = null, metadata = null } = {}) {
  const policy = {
    kind: "lab",
    labClient,
    metadata,
    ignoreCommandLimits: true,
    isOperator() {
      return currentRole(policy) === LAB_ROLE.OPERATOR;
    },
    ignoreCommandLimitsEnabled() {
      return !!policy.ignoreCommandLimits;
    },
    setIgnoreCommandLimits(enabled) {
      policy.ignoreCommandLimits = !!enabled;
    },
    canInspectEntity(entity) {
      return !!entity && Number(entity.owner) !== 0 && !entity.shotReveal && !entity.visionOnly;
    },
    canSelectEntity(entity) {
      return policy.canInspectEntity(entity);
    },
    canUseSetupTools() {
      return policy.isOperator();
    },
    canUseCommandSurface() {
      return policy.isOperator();
    },
    canIssueAs(playerId) {
      return policy.isOperator() && Number(playerId) > 0;
    },
    commandOwnerForSelection(selection, state = null) {
      const owner = policy.issueAsOwnerForSelection(selection);
      return policy.canIssueAs(owner) ? owner : null;
    },
    commandOwner(state = null) {
      return policy.commandOwnerForSelection(selectedEntitiesForPolicy(state), state);
    },
    commandResources(state = null, owner = policy.commandOwner(state)) {
      return resourcesForOwner(state, owner);
    },
    commandFactionId(state = null, owner = policy.commandOwner(state)) {
      return factionIdForOwner(state, owner);
    },
    commandUpgrades(state = null, owner = policy.commandOwner(state)) {
      return upgradesForOwner(state, owner);
    },
    isCommandOwner(owner, state = null) {
      const commandOwner = policy.commandOwner(state);
      return commandOwner != null && Number(owner) === commandOwner;
    },
    isCommandAllyOwner(owner, state = null) {
      const commandOwner = policy.commandOwner(state);
      return ownersRelatedByTeam(state, commandOwner, owner, "ally");
    },
    isCommandEnemyOwner(owner, state = null) {
      const commandOwner = policy.commandOwner(state);
      return ownersRelatedByTeam(state, commandOwner, owner, "enemy");
    },
    selectedOwners(selection) {
      const owners = new Set();
      for (const entity of selection || []) {
        const owner = Number(entity?.owner);
        if (Number.isInteger(owner) && owner !== 0) owners.add(owner);
      }
      return Array.from(owners).sort((a, b) => a - b);
    },
    issueAsOwnerForSelection(selection) {
      const owners = policy.selectedOwners(selection);
      return owners.length === 1 ? owners[0] : null;
    },
    feedbackOwnerForSelection(selection) {
      const owner = policy.issueAsOwnerForSelection(selection);
      return policy.canIssueAs(owner) ? owner : null;
    },
    feedbackOwner(state = null) {
      return policy.feedbackOwnerForSelection(selectedEntitiesForPolicy(state));
    },
    isFeedbackOwner(owner, state = null) {
      const feedbackOwner = policy.feedbackOwner(state);
      return feedbackOwner != null && Number(owner) === feedbackOwner;
    },
    canControlOwner(owner, state = null) {
      const ownerId = Number(owner);
      if (!Number.isInteger(ownerId) || ownerId === 0) return false;
      if (!policy.isOperator()) return false;
      const selection = typeof state?.selectedEntities === "function" ? state.selectedEntities() : [];
      if (!selection || selection.length === 0) return true;
      return policy.issueAsOwnerForSelection(selection) === ownerId;
    },
    canIssueGameplayCommand(command, state) {
      if (!policy.isOperator()) return { ok: false, reason: "Only the lab operator can issue commands." };
      const owner = policy.issueAsOwnerForSelection(
        typeof state?.selectedEntities === "function" ? state.selectedEntities() : [],
      );
      if (owner == null) {
        return { ok: false, reason: "Select entities from exactly one owner to issue gameplay commands." };
      }
      if (!command || typeof command !== "object") {
        return { ok: false, reason: "No gameplay command is selected." };
      }
      if (!policy.ignoreCommandLimitsEnabled()) {
        const budget = commandWithinBudget(state, command, { ownerId: owner });
        if (!budget.ok) {
          return {
            ok: false,
            reason: COMMAND_BUDGET_OVERFLOW_NOTICE,
            blocked: "commandBudget",
            budget,
          };
        }
      }
      return { ok: true, playerId: owner };
    },
    issueCommand(command, { state = null, toast = null } = {}) {
      const decision = policy.canIssueGameplayCommand(command, state);
      if (!decision.ok) {
        toast?.(decision.reason);
        return {
          sent: false,
          predicted: false,
          blocked: decision.blocked || "labPolicy",
          reason: decision.reason,
          budget: decision.budget,
        };
      }
      if (!policy.labClient || typeof policy.labClient.request !== "function") {
        const reason = "Lab controls are not connected.";
        toast?.(reason);
        return { sent: false, predicted: false, blocked: "labClient", reason };
      }
      // Lab recipients are spectator-shaped; issueCommandAs is the privileged server-validated
      // operator path instead of spoofing the WebSocket sender's active player seat.
      return policy.labClient.request({
        op: "issueCommandAs",
        playerId: decision.playerId,
        cmd: command,
        ignoreCommandLimits: policy.ignoreCommandLimitsEnabled(),
      }).then((result) => {
        if (!result.ok) toast?.(result.error || "Lab command rejected.");
        return { sent: result.ok, predicted: false, result, playerId: decision.playerId };
      });
    },
    destroy() {},
  };
  return policy;
}

function currentRole(policy) {
  return policy.labClient?.state?.role || policy.metadata?.role || "";
}

export function createDefaultControlPolicy() {
  return {
    kind: "match",
    isOperator() {
      return false;
    },
    canInspectEntity(entity) {
      return !!entity;
    },
    canSelectEntity(entity, state = null) {
      if (!entity) return false;
      if (state?.spectator) return Number(entity.owner) !== 0;
      return typeof state?.isOwnOwner === "function"
        ? state.isOwnOwner(entity.owner)
        : Number(entity.owner) === state?.playerId;
    },
    canUseSetupTools() {
      return false;
    },
    canUseCommandSurface(state = null) {
      return state == null ? true : !state.spectator;
    },
    selectedOwners() {
      return [];
    },
    issueAsOwnerForSelection() {
      return null;
    },
    commandOwner(state = null) {
      return defaultFeedbackOwner(state);
    },
    commandOwnerForSelection(_selection, state = null) {
      return defaultFeedbackOwner(state);
    },
    commandResources(state = null) {
      return state?.resources || emptyResources();
    },
    commandFactionId(state = null) {
      return state?.localFactionId || factionIdForOwner(state, defaultFeedbackOwner(state));
    },
    commandUpgrades(state = null) {
      return Array.isArray(state?.upgrades) ? state.upgrades : [];
    },
    feedbackOwnerForSelection(_selection, state = null) {
      return defaultFeedbackOwner(state);
    },
    feedbackOwner(state = null) {
      return defaultFeedbackOwner(state);
    },
    isFeedbackOwner(owner, state = null) {
      return typeof state?.isOwnOwner === "function"
        ? state.isOwnOwner(owner)
        : Number(owner) === defaultFeedbackOwner(state);
    },
    ignoreCommandLimitsEnabled() {
      return false;
    },
    setIgnoreCommandLimits() {},
    canControlOwner(owner, state = null) {
      return typeof state?.isOwnOwner === "function"
        ? state.isOwnOwner(owner)
        : Number(owner) === state?.playerId;
    },
    canIssueGameplayCommand() {
      return { ok: false, reason: "Not in lab mode." };
    },
    canIssueAs() {
      return false;
    },
    isCommandOwner(owner, state = null) {
      return typeof state?.isOwnOwner === "function"
        ? state.isOwnOwner(owner)
        : Number(owner) === defaultFeedbackOwner(state);
    },
    isCommandAllyOwner(owner, state = null) {
      return typeof state?.isAllyOwner === "function" ? state.isAllyOwner(owner) : false;
    },
    isCommandEnemyOwner(owner, state = null) {
      return typeof state?.isEnemyOwner === "function"
        ? state.isEnemyOwner(owner)
        : fallbackEnemyOwner(defaultFeedbackOwner(state), owner);
    },
    destroy() {},
  };
}

function selectedEntitiesForPolicy(state) {
  return typeof state?.selectedEntities === "function" ? state.selectedEntities() : [];
}

function defaultFeedbackOwner(state) {
  const owner = Number(state?.playerId);
  return Number.isInteger(owner) && owner > 0 ? owner : null;
}

function emptyResources() {
  return { steel: 0, oil: 0, supplyUsed: 0, supplyCap: 0 };
}

function resourcesForOwner(state, owner) {
  const ownerId = positiveOwner(owner);
  if (ownerId == null) return emptyResources();
  const row = rowForOwner(state?.playerResources, ownerId);
  if (row) return row;
  if (Number(state?.playerId) === ownerId && state?.resources) return state.resources;
  return emptyResources();
}

function factionIdForOwner(state, owner) {
  const ownerId = positiveOwner(owner);
  const player = playerForOwner(state, ownerId);
  if (typeof player?.factionId === "string" && player.factionId.length > 0) return player.factionId;
  if (Number(state?.playerId) === ownerId && typeof state?.localFactionId === "string") {
    return state.localFactionId;
  }
  return DEFAULT_FACTION_ID;
}

function upgradesForOwner(state, owner) {
  const ownerId = positiveOwner(owner);
  if (ownerId == null) return [];
  const perOwner = upgradeArrayForOwner(state?.playerUpgrades, ownerId) ||
    upgradeArrayForOwner(state?.upgradesByPlayer, ownerId) ||
    upgradeArrayForOwner(state?.completedResearchByPlayer, ownerId);
  if (perOwner) return perOwner;
  if (Number(state?.playerId) === ownerId && Array.isArray(state?.upgrades)) return state.upgrades;
  return [];
}

function upgradeArrayForOwner(source, ownerId) {
  const value = ownerValue(source, ownerId);
  if (Array.isArray(value)) return value;
  if (Array.isArray(value?.upgrades)) return value.upgrades;
  if (Array.isArray(value?.completed)) return value.completed;
  if (Array.isArray(value?.completedResearch)) return value.completedResearch;
  return null;
}

function rowForOwner(rows, ownerId) {
  if (!Array.isArray(rows)) return null;
  return rows.find((row) => Number(row?.id ?? row?.playerId ?? row?.owner) === ownerId) ||
    rows[ownerId - 1] ||
    null;
}

function ownerValue(source, ownerId) {
  if (!source) return null;
  if (source instanceof Map) return source.get(ownerId) || source.get(String(ownerId)) || null;
  if (Array.isArray(source)) return rowForOwner(source, ownerId);
  if (typeof source === "object") return source[ownerId] || source[String(ownerId)] || null;
  return null;
}

function ownersRelatedByTeam(state, commandOwner, owner, expected) {
  const commandOwnerId = positiveOwner(commandOwner);
  const ownerId = positiveOwner(owner);
  if (commandOwnerId == null || ownerId == null || ownerId === commandOwnerId) return false;
  const commandTeam = teamForOwner(state, commandOwnerId);
  const ownerTeam = teamForOwner(state, ownerId);
  if (commandTeam != null && ownerTeam != null) {
    return expected === "ally" ? commandTeam === ownerTeam : commandTeam !== ownerTeam;
  }
  return expected === "enemy" ? ownerId !== commandOwnerId : false;
}

function fallbackEnemyOwner(commandOwner, owner) {
  const commandOwnerId = positiveOwner(commandOwner);
  const ownerId = positiveOwner(owner);
  return commandOwnerId != null && ownerId != null && ownerId !== commandOwnerId;
}

function teamForOwner(state, ownerId) {
  if (typeof state?.teamIdForPlayer === "function") {
    const team = Number(state.teamIdForPlayer(ownerId));
    return Number.isInteger(team) && team > 0 ? team : null;
  }
  const player = playerForOwner(state, ownerId);
  const team = Number(player?.teamId);
  return Number.isInteger(team) && team > 0 ? team : null;
}

function playerForOwner(state, ownerId) {
  if (ownerId == null) return null;
  if (typeof state?.playerById === "function") return state.playerById(ownerId);
  return (state?.players || []).find((player) => Number(player?.id) === ownerId) || null;
}

function positiveOwner(owner) {
  const ownerId = Number(owner);
  return Number.isInteger(ownerId) && ownerId > 0 ? ownerId : null;
}
