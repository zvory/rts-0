import { App } from "./app.js";
import { AnalyticsConsent } from "./analytics_consent.js";
import {
  diagnostics,
  snapshotStreamLaunchConfig,
  stressTestLaunchConfig,
} from "./bootstrap.js";
import { MapEditorApp } from "./map_editor_app.js";
import { mapEditorLaunchConfig } from "./map_editor_launch.js";
import { MapPreviewApp } from "./map_preview_app.js";
import { mapPreviewLaunchConfig } from "./map_preview_launch.js";
import { SnapshotStreamNet } from "./snapshot_stream_net.js";

async function start() {
  const analyticsConsent = new AnalyticsConsent();
  analyticsConsent.start();

  let app;
  try {
    const stressTestLaunch = stressTestLaunchConfig();
    const snapshotStreamLaunch = stressTestLaunch || snapshotStreamLaunchConfig();
    app = mapPreviewLaunchConfig()
      ? new MapPreviewApp()
      : mapEditorLaunchConfig()
        ? new MapEditorApp()
        : new App({
          net: snapshotStreamLaunch
            ? new SnapshotStreamNet({
              id: snapshotStreamLaunch.id,
              diagnostics,
              autoStart: !stressTestLaunch,
            })
            : null,
          snapshotStreamLaunch,
          stressTestLaunch,
        });
  } catch (error) {
    showRendererBootstrapError(error);
    return;
  }
  // Debug/introspection handle. Harmless in production; lets dev tooling and the
  // integration tests inspect live match state (e.g. `__rts.match.state.selection`).
  if (typeof window !== "undefined") window.__rts = app;
  try {
    await app.start();
  } catch (error) {
    showRendererBootstrapError(error);
    app.destroy?.();
  }
}

function showRendererBootstrapError(_error) {
  const target = globalThis.document?.getElementById?.("toast")
    || globalThis.document?.getElementById?.("app");
  if (!target) return;
  target.textContent = "The game client could not start.";
  target.hidden = false;
  target.setAttribute?.("role", "alert");
}

void start();
