import { App } from "./app.js";
import { MapEditorApp } from "./map_editor_app.js";
import { mapEditorLaunchConfig } from "./map_editor_launch.js";

const app = mapEditorLaunchConfig() ? new MapEditorApp() : new App();
app.start();

// Debug/introspection handle. Harmless in production; lets dev tooling and the
// integration tests inspect live match state (e.g. `__rts.match.state.selection`).
if (typeof window !== "undefined") window.__rts = app;
