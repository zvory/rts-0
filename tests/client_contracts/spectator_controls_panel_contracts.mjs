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
} finally {
  if (priorWindow === undefined) delete globalThis.window;
  else globalThis.window = priorWindow;
}
