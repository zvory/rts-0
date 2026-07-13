import { Match } from "./match.js";

export class ReplayViewer extends Match {
  constructor(net, payload, toast, devWatch, audio, statusBadge, diagnostics = null, options = {}) {
    super(net, payload, toast, devWatch, audio, statusBadge, diagnostics, {
      replayViewer: true,
      initialCamera: options.initialCamera,
      hotkeyProfiles: options.hotkeyProfiles,
      settings: options.settings,
      onBackToLobby: options.onBackToLobby,
      unitRangesEnabled: options.unitRangesEnabled,
      onUnitRangesEnabledChange: options.onUnitRangesEnabledChange,
      autoSpectatorEnabled: options.autoSpectatorEnabled,
      onAutoSpectatorEnabledChange: options.onAutoSpectatorEnabledChange,
      capabilities: options.capabilities,
      cameraMaxZoom: options.cameraMaxZoom,
      observerAnalysisOverlayPreferences: options.observerAnalysisOverlayPreferences,
      aiDiagnosticsPanelPreferences: options.aiDiagnosticsPanelPreferences,
    });
  }
}
