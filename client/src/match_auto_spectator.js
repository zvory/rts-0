import { AUTO_SPECTATOR_MIN_ZOOM, AutoSpectatorDirector } from "./auto_spectator.js";

function availableForMatch(match, payload) {
  return !match.labMetadata && (match.replayViewer || payload?.spectator);
}

export function autoSpectatorCameraMinZoom(match, payload) {
  return availableForMatch(match, payload) ? AUTO_SPECTATOR_MIN_ZOOM : undefined;
}

export function createMatchAutoSpectator(match, payload, options = {}) {
  if (!availableForMatch(match, payload)) return null;
  return new AutoSpectatorDirector({
    camera: match.camera,
    state: match.state,
    enabled: options.autoSpectatorEnabled,
    onEnabledChange: options.onAutoSpectatorEnabledChange,
  });
}
