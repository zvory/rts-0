import { dom } from "./bootstrap.js";
import { createMapHandoff, consumeMapHandoff } from "./map_editor_handoff.js";
import { mapEditorLaunchConfig } from "./map_editor_launch.js";
import { MapEditorInteractBridge } from "./map_editor_interact_bridge.js";
import { MapEditorPanel } from "./map_editor_panel.js";
import { MapEditorSession } from "./map_editor_session.js";
import { MapEditorViewport } from "./map_editor_viewport.js";

export class MapEditorApp {
  constructor({ locationObj = window.location } = {}) {
    this.locationObj = locationObj;
    this.launch = mapEditorLaunchConfig(locationObj);
    this.capabilities = Object.freeze({
      mapEditing: true,
      simulation: false,
      gameplayCommands: false,
      roomTime: false,
      fog: false,
      replay: false,
      ai: false,
    });
    this.session = new MapEditorSession();
    this.viewport = null;
    this.panel = null;
    this.interactBridge = null;
    this.allowUnload = false;
    this.onBeforeUnload = (event) => {
      if (this.allowUnload || !this.session.hasUnsavedChanges) return;
      event.preventDefault();
      event.returnValue = true;
    };
  }

  async start() {
    document.body.classList.add("map-editor-mode");
    document.title = "Map Editor · Bewegungskrieg";
    dom.lobbyScreen.hidden = true;
    if (dom.labEntryScreen) dom.labEntryScreen.hidden = true;
    if (dom.branchScreen) dom.branchScreen.hidden = true;
    dom.gameScreen.hidden = false;
    if (dom.devLinks) dom.devLinks.hidden = true;
    if (dom.devBanner) dom.devBanner.hidden = true;
    window.addEventListener("beforeunload", this.onBeforeUnload);

    if (this.launch.error) {
      this.session.initializeBlank();
    } else if (this.launch.handoffId) {
      try {
        const handoff = await consumeMapHandoff(this.launch.handoffId);
        if (handoff?.destination !== "editor" || !handoff?.authoredMap) {
          throw new Error("Map handoff was not addressed to the editor.");
        }
        this.session.loadAuthoredMap(handoff.authoredMap);
      } catch (error) {
        this.session.initializeBlank();
        this.launch.error = error.message || String(error);
      }
    } else {
      this.session.initializeBlank();
    }

    let panel = null;
    this.viewport = await MapEditorViewport.create({
      root: dom.viewport,
      session: this.session,
      onStatus: (message, error) => panel?.setStatus(message, error),
    });
    panel = new MapEditorPanel({
      root: dom.gameScreen,
      session: this.session,
      viewport: this.viewport,
      onOpenLab: (map) => this.openInLab(map),
      onOpenPreview: (map) => this.openPreview(map),
    });
    this.panel = panel;
    if (this.launch.interact && !this.launch.error) {
      this.interactBridge = new MapEditorInteractBridge({ app: this });
      await this.interactBridge.initialize(this.launch.mapFile);
    }
    if (this.launch.error) this.panel.setStatus(this.launch.error, true);
    globalThis.__mapEditor = this;
  }

  async openInLab({ authoredMap, materializedMap }) {
    const handoff = await createMapHandoff({
      destination: "lab",
      authoredMap,
      materializedMap,
    });
    const url = new URL("/lab", this.locationObj.href);
    url.searchParams.set("handoff", handoff.handoffId);
    this.allowUnload = true;
    window.location.assign(url.toString());
  }

  async openPreview({ authoredMap, materializedMap }) {
    const opened = window.open("about:blank", "_blank");
    if (!opened) throw new Error("The browser blocked the map preview window. Allow pop-ups and try again.");
    opened.opener = null;
    try {
      const handoff = await createMapHandoff({
        destination: "lab",
        authoredMap,
        materializedMap,
      });
      const url = new URL("/map-preview", this.locationObj.href);
      url.searchParams.set("handoff", handoff.handoffId);
      opened.location.replace(url.toString());
    } catch (error) {
      opened.close();
      throw error;
    }
  }

  destroy() {
    window.removeEventListener("beforeunload", this.onBeforeUnload);
    this.interactBridge?.destroy();
    this.interactBridge = null;
    this.panel?.destroy();
    this.viewport?.destroy();
    document.body.classList.remove("map-editor-mode");
  }
}
