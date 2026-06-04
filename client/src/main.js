import { App } from "./app.js";

const app = new App();
app.start();

// Debug/introspection handle. Harmless in production; lets dev tooling and the
// integration tests inspect live match state (e.g. `__rts.match.state.selection`).
if (typeof window !== "undefined") window.__rts = app;
