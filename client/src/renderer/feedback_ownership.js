export function feedbackOwner(state, owner) {
  if (typeof state?.isFeedbackOwner === "function") return state.isFeedbackOwner(owner);
  if (typeof state?.isOwnOwner === "function") return state.isOwnOwner(owner);
  return Number(owner) === state?.playerId;
}

export function ownOrAllyOwner(state, owner) {
  if (feedbackOwner(state, owner)) return true;
  if (typeof state?.isOwnOwner === "function" && state.isOwnOwner(owner)) return true;
  if (typeof state?.isAllyOwner === "function" && state.isAllyOwner(owner)) return true;
  return Number(owner) === state?.playerId;
}
