import { dom } from "./bootstrap.js";
import { consumeMapHandoff } from "./map_editor_handoff.js";
import { MapEditorSession } from "./map_editor_session.js";
import { MapPreviewBridge } from "./map_preview_bridge.js";
import { mapPreviewLaunchConfig } from "./map_preview_launch.js";
import { MapEditorPixiPresentationAdapter } from "./renderer/map_editor_presentation_adapter.js";

export class MapPreviewApp {
  constructor({ locationObj = window.location } = {}) {
    this.locationObj = locationObj;
    this.launch = mapPreviewLaunchConfig(locationObj);
    this.session = new MapEditorSession();
    this.bridge = null;
    this.controls = null;
    this.destroyed = false;
  }

  async start() {
    document.body.classList.add("map-preview-mode");
    document.title = "Map Preview · Bewegungskrieg";
    dom.lobbyScreen.hidden = true;
    if (dom.labEntryScreen) dom.labEntryScreen.hidden = true;
    if (dom.branchScreen) dom.branchScreen.hidden = true;
    dom.gameScreen.hidden = false;
    if (dom.devLinks) dom.devLinks.hidden = true;
    if (dom.devBanner) dom.devBanner.hidden = true;
    globalThis.__rtsMapPreview = loadingStatus();
    try {
      if (!this.launch?.handoffId || this.launch.error) throw new Error(this.launch?.error || "Map preview launch is invalid.");
      const handoff = await consumeMapHandoff(this.launch.handoffId);
      if (handoff?.destination !== "editor" || !handoff?.authoredMap) {
        throw new Error("Map handoff was not addressed to the preview renderer.");
      }
      this.session.loadAuthoredMap(handoff.authoredMap);
      const presentation = await MapEditorPixiPresentationAdapter.create(dom.viewport);
      if (this.destroyed) {
        presentation.destroy();
        return;
      }
      this.bridge = new MapPreviewBridge({ session: this.session, presentation });
      globalThis.__rtsMapPreview = this.bridge;
      this.controls = createPreviewControls({
        root: dom.gameScreen,
        mapName: this.session.draft.name,
        capture: (kind) => this.download(kind),
      });
    } catch (error) {
      globalThis.__rtsMapPreview = failedStatus(error);
      throw error;
    }
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    this.bridge?.destroy();
    this.controls?.remove();
    if (globalThis.__rtsMapPreview === this.bridge) delete globalThis.__rtsMapPreview;
    document.body.classList.remove("map-preview-mode");
  }

  async download(kind) {
    if (!this.bridge || !this.controls) return;
    const status = this.controls.querySelector("[data-map-preview-status]");
    const buttons = [...this.controls.querySelectorAll("button")];
    for (const button of buttons) button.disabled = true;
    if (status) status.textContent = `Rendering ${kind} PNG…`;
    try {
      const result = await this.bridge.call("capture", {
        kind,
        width: 2048,
        height: 2048,
        padding: 32,
      });
      const anchor = document.createElement("a");
      anchor.href = result.pngDataUrl;
      anchor.download = `${slug(this.session.draft.name)}-${kind}-2048.png`;
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      if (status) status.textContent = `Downloaded ${anchor.download}.`;
    } catch (error) {
      if (status) status.textContent = `Error: ${error?.message || String(error)}`;
    } finally {
      for (const button of buttons) button.disabled = false;
    }
  }
}

function createPreviewControls({ root, mapName, capture }) {
  const controls = document.createElement("aside");
  controls.className = "map-preview-controls";
  controls.setAttribute("aria-label", "Map PNG preview controls");
  const title = document.createElement("strong");
  title.textContent = String(mapName || "Map");
  const summary = document.createElement("p");
  summary.textContent = "Clean full-map captures from the game renderer.";
  const world = document.createElement("button");
  world.type = "button";
  world.textContent = "Download world PNG (2048 px)";
  world.addEventListener("click", () => void capture("world"));
  const minimap = document.createElement("button");
  minimap.type = "button";
  minimap.textContent = "Download minimap PNG (2048 px)";
  minimap.addEventListener("click", () => void capture("minimap"));
  const status = document.createElement("p");
  status.dataset.mapPreviewStatus = "";
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  status.textContent = "Ready.";
  controls.append(title, summary, world, minimap, status);
  root.appendChild(controls);
  return controls;
}

function slug(value) {
  return String(value || "map")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "") || "map";
}

function loadingStatus() {
  return Object.freeze({
    status: () => Object.freeze({ version: 1, state: "loading" }),
    call: async () => { throw new Error("Map preview is still loading."); },
  });
}

function failedStatus(error) {
  const message = String(error?.message || error || "Map preview failed.").slice(0, 500);
  return Object.freeze({
    status: () => Object.freeze({ version: 1, state: "failed", error: message }),
    call: async () => { throw new Error(message); },
  });
}
