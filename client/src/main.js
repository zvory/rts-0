import { App } from "./app.js";
import {
  diagnostics,
  snapshotStreamLaunchConfig,
  stressTestLaunchConfig,
} from "./bootstrap.js";
import { MapEditorApp } from "./map_editor_app.js";
import { mapEditorLaunchConfig } from "./map_editor_launch.js";
import { SnapshotStreamNet } from "./snapshot_stream_net.js";
import {
  createSelectedBackendBundle,
  showRendererBootstrapError,
} from "./renderer/backend_selection.js";

async function start() {
  let app;
  try {
    const stressTestLaunch = stressTestLaunchConfig();
    const snapshotStreamLaunch = stressTestLaunch || snapshotStreamLaunchConfig();
    app = mapEditorLaunchConfig()
      ? new MapEditorApp()
      : new App({
        rendererBackendBundle: await createSelectedBackendBundle(),
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

void start();
