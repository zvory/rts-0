const READ_QUERIES = Object.freeze([
  "isOperator",
  "canInspectEntity",
  "canSelectEntity",
  "canUseSetupTools",
  "canUseCommandSurface",
  "canIssueAs",
  "commandOwnerForSelection",
  "commandOwner",
  "commandResources",
  "commandFactionId",
  "commandUpgrades",
  "isCommandOwner",
  "isCommandAllyOwner",
  "isCommandEnemyOwner",
  "selectedOwners",
  "issueAsOwnerForSelection",
  "feedbackOwnerForSelection",
  "feedbackOwner",
  "isFeedbackOwner",
  "canControlOwner",
]);

/** Expose policy decisions without exposing mutable Lab settings or command authority. */
export function createControlPolicyProjection(policy) {
  const projection = { kind: policy?.kind || "match" };
  for (const name of READ_QUERIES) {
    projection[name] = (...args) => policy?.[name]?.(...args);
  }
  return Object.freeze(projection);
}
