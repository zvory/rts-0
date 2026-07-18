export function applyInteractSelection(match, entityIds) {
  match.clientIntent?.closeCommandCardMenu?.();
  match.state.setSelection(entityIds, { controlPolicy: match.controlPolicy });
  match.clientIntent?.clearPlannedOrdersOutsideSelection?.(match.state.selection || []);
}

export function selectedInteractEntityIds(state, maximum) {
  const selected = typeof state?.selectedEntities === "function"
    ? state.selectedEntities()
    : null;
  if (Array.isArray(selected)) {
    return selected.map((entity) => entity.id).slice(0, maximum);
  }
  return state?.selection instanceof Set
    ? [...state.selection].slice(0, maximum)
    : [];
}
