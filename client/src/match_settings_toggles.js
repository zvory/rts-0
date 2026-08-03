import { MOVEMENT_PATH_DIAGNOSTICS } from "./protocol.js";

export function configureMatchDisplayPreferences(match, options = {}) {
  if (!match?.state) return;
  match.onUnitRangesEnabledChange = options.onUnitRangesEnabledChange;
  match.onHealthBarsAlwaysEnabledChange = options.onHealthBarsAlwaysEnabledChange;
  match.onUnitRangeToggle = match.toggleUnitRangeOverlays.bind(match);
  match.onHealthBarToggle = () => toggleHealthBars(match);
  match.state.showUnitRangesEnabled = options.unitRangesEnabled !== false;
  match.state.showHealthBarsAlwaysEnabled = !!options.healthBarsAlwaysEnabled;
}

export function applyMatchUnitRanges(match, enabled) {
  if (match?.state) match.state.showUnitRangesEnabled = !!enabled;
}

export function applyMatchHealthBars(match, enabled) {
  if (match?.state) match.state.showHealthBarsAlwaysEnabled = !!enabled;
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

export function toggleHealthBars(match) {
  match.state.showHealthBarsAlwaysEnabled = !match.state.showHealthBarsAlwaysEnabled;
  match.syncSettingsToggleUi();
  match.onHealthBarsAlwaysEnabledChange?.(match.state.showHealthBarsAlwaysEnabled);
}
