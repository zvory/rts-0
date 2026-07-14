import { AUTO_SPECTATOR_MIN_ZOOM, AutoSpectatorDirector } from "./auto_spectator.js";
import { SpectatorControlsPanel } from "./spectator_controls_panel.js";

function availableForMatch(match, payload) {
  return !match.labMetadata && (match.replayViewer || payload?.spectator);
}

export function autoSpectatorCameraMinZoom(match, payload) {
  return availableForMatch(match, payload) ? AUTO_SPECTATOR_MIN_ZOOM : undefined;
}

export function createMatchAutoSpectator(match, payload, options = {}, controlsRoot = null) {
  if (!availableForMatch(match, payload)) return null;
  return new MatchAutoSpectator(match, options, controlsRoot);
}

class MatchAutoSpectator {
  constructor(match, options, controlsRoot) {
    this.director = new AutoSpectatorDirector({
      camera: match.camera,
      state: match.state,
      enabled: options.autoSpectatorEnabled,
      onEnabledChange: options.onAutoSpectatorEnabledChange,
    });
    this.panel = new SpectatorControlsPanel({
      root: controlsRoot,
      state: () => ({ available: true, enabled: this.director.enabled }),
      onToggle: (enabled) => this.setEnabled(enabled),
    });
  }

  get enabled() {
    return this.director.enabled;
  }

  setEnabled(enabled) {
    this.director.setEnabled(enabled);
    this.panel.sync();
  }

  observeSnapshot(snapshot) {
    this.director.observeSnapshot(snapshot);
  }

  update(dt) {
    this.director.update(dt);
  }

  handleViewportChange() {
    this.director.handleViewportChange();
    this.panel.handleViewportChange();
  }

  diagnostics() {
    return this.director.diagnostics();
  }

  destroy() {
    this.panel.destroy();
    this.director.destroy();
  }
}
