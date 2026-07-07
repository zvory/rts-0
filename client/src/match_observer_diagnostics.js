import { AiDiagnosticsPanel, shouldMountAiDiagnosticsPanel } from "./ai_diagnostics_panel.js";
import { ObserverAnalysisOverlay, shouldMountObserverAnalysisOverlay } from "./observer_analysis_overlay.js";

export class MatchObserverDiagnostics {
  constructor({
    root,
    capabilities,
    observerAnalysisOverlayPreferences = null,
    aiDiagnosticsPanelPreferences = null,
    getEntities = () => [],
    getCameraBounds = () => null,
    getPlayers = () => [],
  }) {
    this.observerAnalysisOverlay = shouldMountObserverAnalysisOverlay({ capabilities })
      ? new ObserverAnalysisOverlay({
        root,
        preferences: observerAnalysisOverlayPreferences || undefined,
        getEntities,
        getCameraBounds,
        getPlayers,
      })
      : null;
    this.aiDiagnosticsPanel = shouldMountAiDiagnosticsPanel({ capabilities })
      ? new AiDiagnosticsPanel({
        root,
        preferences: aiDiagnosticsPanelPreferences || undefined,
        getPlayers,
      })
      : null;
  }

  applyObserverAnalysis(payload) {
    this.observerAnalysisOverlay?.applyObserverAnalysis(payload);
    this.aiDiagnosticsPanel?.applyObserverAnalysis(payload);
  }

  update(frameViews = null, { profiler = null } = {}) {
    this.observerAnalysisOverlay?.update(frameViews, { profiler });
  }

  destroy() {
    this.observerAnalysisOverlay?.destroy();
    this.aiDiagnosticsPanel?.destroy();
    this.observerAnalysisOverlay = null;
    this.aiDiagnosticsPanel = null;
  }
}
