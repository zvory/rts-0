import { buildSettingsTabs } from "./settings_panels.js";
import { exclusiveFullscreenSupported } from "./exclusive_fullscreen_settings.js";

export function mountLobbySettings(app) {
app.settings?.setContext({
  kind: "lobby",
  spectator: false,
  replay: false,
  tabs: buildSettingsTabs({
    preferredColor: app.preferredColor,
    audio: app.audio,
    hotkeyProfiles: app.hotkeyProfiles,
    game: {
      prediction: {
        state: () => ({
          enabled: app.predictionEnabled,
          active: false,
          available: true,
        }),
        onToggle: () => app.setPredictionEnabled(!app.predictionEnabled),
      },
      unitRanges: {
        state: () => ({
          enabled: app.unitRangesEnabled,
          available: true,
        }),
        onToggle: () => app.setUnitRangesEnabled(!app.unitRangesEnabled),
      },
      projectedUnitShadows: {
        state: () => ({
          enabled: app.projectedUnitShadowsEnabled,
          available: true,
        }),
        onToggle: () =>
          app.setProjectedUnitShadowsEnabled(!app.projectedUnitShadowsEnabled),
      },
      healthBars: {
        state: () => ({
          enabled: app.healthBarsAlwaysEnabled,
          available: true,
        }),
        onToggle: () => app.setHealthBarsAlwaysEnabled(!app.healthBarsAlwaysEnabled),
      },
      exclusiveFullscreen: {
        state: () => ({
          hidden: !exclusiveFullscreenSupported(),
          supported: exclusiveFullscreenSupported(),
          enabled: app.exclusiveFullscreenEnabled,
        }),
        onToggle: () =>
          app.setExclusiveFullscreenEnabled(!app.exclusiveFullscreenEnabled),
      },
    },
  }),
});
}
