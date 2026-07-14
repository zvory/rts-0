import { readFileSync } from "node:fs";
import { createMatchAutoSpectator } from "../../client/src/match_auto_spectator.js";
import { SpectatorControlsPanel } from "../../client/src/spectator_controls_panel.js";
import { assert } from "./assertions.mjs";
import { fakeStorage, withFakeOverlayDocument } from "./fakes.mjs";

const priorWindow = globalThis.window;
const listeners = new Map();
const storage = fakeStorage();

globalThis.window = {
  innerWidth: 1200,
  innerHeight: 800,
  localStorage: storage,
  matchMedia: () => ({ matches: false }),
  addEventListener(type, handler) {
    listeners.set(type, handler);
  },
  removeEventListener(type, handler) {
    if (listeners.get(type) === handler) listeners.delete(type);
  },
};

try {
  withFakeOverlayDocument(() => {
    const host = document.createElement("div");
    let enabled = false;
    const panel = new SpectatorControlsPanel({
      root: host,
      state: () => ({ available: true, enabled }),
      onToggle: (next) => { enabled = next; },
    });

    const root = host.querySelector(".spectator-controls-panel");
    const title = host.querySelector(".spectator-controls-title");
    const toggle = host.querySelector("#auto-spectator-toggle");
    assert(root && title?.textContent === "Spectator Controls", "spectator controls mount as a dedicated floating panel");
    assert(toggle?.textContent === "Off", "spectator fight-following defaults off");
    assert(toggle?.getAttribute("aria-checked") === "false", "spectator fight-following exposes switch state");

    toggle.listeners.click({ pointerType: "mouse", detail: 1 });
    assert(enabled && toggle.textContent === "On", "spectator panel toggles automatic fight-following on");
    assert(toggle.getAttribute("aria-checked") === "true", "spectator panel synchronizes the enabled switch state");
    assert(listeners.has("resize"), "spectator panel constrains its floating position on resize");

    panel.destroy();
    assert(host.children.length === 0, "spectator panel removes its DOM on teardown");
    assert(!listeners.has("resize"), "spectator panel removes its resize listener on teardown");
  });

  withFakeOverlayDocument(() => {
    const host = document.createElement("div");
    let persistedEnabled = false;
    const match = {
      labMetadata: null,
      replayViewer: false,
      camera: { snapshot: () => null },
      state: {
        map: { width: 64, height: 64, tileSize: 32 },
        players: [],
        entitiesInterpolated: () => [],
      },
    };
    const autoSpectator = createMatchAutoSpectator(match, { spectator: true }, {
      onAutoSpectatorEnabledChange: (enabled) => { persistedEnabled = enabled; },
    }, host);

    assert(autoSpectator && host.children.length === 1,
      "match auto spectator mounts its controls into the injected host");
    host.querySelector("#auto-spectator-toggle").listeners.click({ pointerType: "mouse", detail: 1 });
    assert(autoSpectator.enabled && persistedEnabled,
      "match auto spectator keeps its director, panel, and persisted preference synchronized");
    autoSpectator.destroy();
    assert(host.children.length === 0, "match auto spectator tears down its injected controls");

    match.labMetadata = {};
    assert(createMatchAutoSpectator(match, { spectator: true }, {}, host) === null,
      "lab matches do not mount spectator controls");
  });

  const styles = readFileSync(new URL("../../client/styles.css", import.meta.url), "utf8");
  assert(
    /@media \(pointer: coarse\) and \(max-width: 1024px\) and \(max-height: 599px\)\s*\{[\s\S]*?\.spectator-controls-panel\s*\{[\s\S]*?left:\s*var\(--mobile-debug-left\)[\s\S]*?bottom:\s*var\(--mobile-debug-bottom\)/s.test(styles),
    "mobile landscape keeps fixed spectator controls below the left-side room controls",
  );
} finally {
  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
}
