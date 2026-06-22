import { LAB_ROLE } from "./protocol.js";

export function createLabControlPolicy({ labClient = null, metadata = null } = {}) {
  const policy = {
    kind: "lab",
    labClient,
    metadata,
    unlimitedSelection: true,
    isOperator() {
      return currentRole(policy) === LAB_ROLE.OPERATOR;
    },
    unlimitedSelectionEnabled() {
      return !!policy.unlimitedSelection;
    },
    setUnlimitedSelection(enabled) {
      policy.unlimitedSelection = !!enabled;
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
      return { ok: true, playerId: owner };
    },
    issueCommand(command, { state = null, toast = null } = {}) {
      const decision = policy.canIssueGameplayCommand(command, state);
      if (!decision.ok) {
        toast?.(decision.reason);
        return { sent: false, predicted: false, blocked: "labPolicy", reason: decision.reason };
      }
      if (!policy.labClient || typeof policy.labClient.request !== "function") {
        const reason = "Lab controls are not connected.";
        toast?.(reason);
        return { sent: false, predicted: false, blocked: "labClient", reason };
      }
      return policy.labClient.request({
        op: "issueCommandAs",
        playerId: decision.playerId,
        cmd: command,
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
    unlimitedSelectionEnabled() {
      return false;
    },
    setUnlimitedSelection() {},
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
    destroy() {},
  };
}
