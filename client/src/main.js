import { App } from "./app.js";
import { MapEditorApp } from "./map_editor_app.js";
import { mapEditorLaunchConfig } from "./map_editor_launch.js";
import {
  createSelectedBackendBundle,
  showRendererBootstrapError,
} from "./renderer/backend_selection.js";

async function start() {
  let app;
  try {
    app = mapEditorLaunchConfig()
      ? new MapEditorApp()
      : new App({ rendererBackendBundle: await createSelectedBackendBundle() });
  } catch (error) {
    showRendererBootstrapError(error);
    return;
  }
  // Debug/introspection handle. Harmless in production; lets dev tooling and the
  // integration tests inspect live match state (e.g. `__rts.match.state.selection`).
  if (typeof window !== "undefined") window.__rts = app;
  await app.start();
}

void start();
