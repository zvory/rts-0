import { admitSelectionIds } from "./command_budget.js";

/** Reconcile the browser-local selection against the latest authoritative entities. */
export function pruneSelection(state, { controlPolicy = null, enforceCommandBudget = false } = {}) {
  if (!state.selection || state.selection.size === 0) return null;

  const live = new Set();
  for (const id of state.selection) {
    const entity = state._curById.get(id);
    if (entity && !entity.shotReveal && !entity.visionOnly) live.add(id);
  }
  if (live.size !== state.selection.size) state.selection = live;
  if (!enforceCommandBudget || live.size === 0) return null;

  const admitted = admitSelectionIds(state, live, { controlPolicy });
  if (admitted.ids.length === live.size) return null;
  state.selection = new Set(admitted.ids);
  return admitted;
}
