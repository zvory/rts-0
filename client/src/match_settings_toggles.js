import { MOVEMENT_PATH_DIAGNOSTICS } from "./protocol.js";

export function configureMatchDisplayPreferences(match, options = {}) {
  if (!match?.state) return;
  match.onUnitRangesEnabledChange = options.onUnitRangesEnabledChange;
  match.onHealthBarsAlwaysEnabledChange = options.onHealthBarsAlwaysEnabledChange;
  match.preferredColor = options.preferredColor;
  match.onProjectedUnitShadowsEnabledChange = options.onProjectedUnitShadowsEnabledChange;
  match.onExperimentalGraphicsEnabledChange = options.onExperimentalGraphicsEnabledChange;
  match.onExperimentalGraphicsToggle = () => {
    const enabled = !match.state.experimentalGraphicsEnabled;
    applyMatchExperimentalGraphics(match, enabled);
    match.onExperimentalGraphicsEnabledChange?.(enabled);
  };
  match.onUnitRangeToggle = match.toggleUnitRangeOverlays.bind(match);
  match.onHealthBarToggle = () => toggleHealthBars(match);
  match.onProjectedUnitShadowToggle = () => toggleProjectedUnitShadows(match);
  match.state.showUnitRangesEnabled = options.unitRangesEnabled !== false;
  match.state.showHealthBarsAlwaysEnabled = !!options.healthBarsAlwaysEnabled;
  match.state.showProjectedUnitShadowsEnabled = options.projectedUnitShadowsEnabled !== false;
  match.state.experimentalGraphicsEnabled = options.experimentalGraphicsEnabled === true;
}

export function applyMatchExperimentalGraphics(match, enabled) {
  if (!match?.state) return;
  match.state.experimentalGraphicsEnabled = !!enabled;
  if (!enabled) match.state.restoreExperimentalShotTiming?.();
  match.renderer?.setExperimentalGraphicsEnabled?.(!!enabled);
}

export function applyMatchUnitRanges(match, enabled) {
  if (match?.state) match.state.showUnitRangesEnabled = !!enabled;
}

export function applyMatchHealthBars(match, enabled) {
  if (match?.state) match.state.showHealthBarsAlwaysEnabled = !!enabled;
}

export function applyMatchProjectedUnitShadows(match, enabled) {
  if (!match?.state) return;
  match.state.showProjectedUnitShadowsEnabled = !!enabled;
  match.renderer?.setProjectedUnitShadowsEnabled?.(!!enabled);
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

export function toggleProjectedUnitShadows(match) {
  const enabled = !match.state.showProjectedUnitShadowsEnabled;
  applyMatchProjectedUnitShadows(match, enabled);
  match.syncSettingsToggleUi();
  match.onProjectedUnitShadowsEnabledChange?.(enabled);
}
