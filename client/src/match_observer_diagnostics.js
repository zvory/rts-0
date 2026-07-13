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
    this.latestMapAnalysis = null;
    this.mapLayerVisibility = {};
    this.observerAnalysisOverlay = shouldMountObserverAnalysisOverlay({ capabilities })
      ? new ObserverAnalysisOverlay({
        root,
        preferences: observerAnalysisOverlayPreferences || undefined,
        getEntities,
        getCameraBounds,
        getPlayers,
      })
      : null;
    this.aiDiagnosticsPanel = shouldMountAiDiagnosticsPanel({
      capabilities,
      players: getPlayers(),
    })
      ? new AiDiagnosticsPanel({
        root,
        preferences: aiDiagnosticsPanelPreferences || undefined,
        getPlayers,
        onMapLayerVisibilityChange: (visibility) => {
          this.mapLayerVisibility = { ...(visibility || {}) };
        },
      })
      : null;
  }

  applyObserverAnalysis(payload) {
    this.latestMapAnalysis = payload?.mapAnalysis || null;
    this.observerAnalysisOverlay?.applyObserverAnalysis(payload);
    this.aiDiagnosticsPanel?.applyObserverAnalysis(payload);
  }

  mapOverlayModel() {
    return this.latestMapAnalysis
      ? {
        analysis: this.latestMapAnalysis,
        visibleLayers: this.aiDiagnosticsPanel?.mapLayerVisibility?.() || this.mapLayerVisibility,
      }
      : null;
  }

  update(frameViews = null, { profiler = null } = {}) {
    this.observerAnalysisOverlay?.update(frameViews, { profiler });
  }

  destroy() {
    this.observerAnalysisOverlay?.destroy();
    this.aiDiagnosticsPanel?.destroy();
    this.observerAnalysisOverlay = null;
    this.aiDiagnosticsPanel = null;
    this.latestMapAnalysis = null;
    this.mapLayerVisibility = {};
  }
}
