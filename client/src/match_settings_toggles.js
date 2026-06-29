import { MOVEMENT_PATH_DIAGNOSTICS } from "./protocol.js";

export function applyInitialUnitRanges(state, enabled) {
  if (state) state.showUnitRangesEnabled = enabled !== false;
}

export function applyMatchUnitRanges(match, enabled) {
  if (match?.state) match.state.showUnitRangesEnabled = !!enabled;
}

export function toggleDebugPaths(match) {
  if (match.capabilities.diagnostics.movementPaths === MOVEMENT_PATH_DIAGNOSTICS.NONE) {
    match.syncSettingsToggleUi();
    return;
  }
  match.state.debugPathOverlaysEnabled = !match.state.debugPathOverlaysEnabled;
  match.syncSettingsToggleUi();
}

export function toggleUnitRanges(match) {
  match.state.showUnitRangesEnabled = !match.state.showUnitRangesEnabled;
  match.syncSettingsToggleUi();
  match.onUnitRangesEnabledChange?.(match.state.showUnitRangesEnabled);
}
