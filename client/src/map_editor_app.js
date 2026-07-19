import { dom } from "./bootstrap.js";
import { createMapHandoff, consumeMapHandoff } from "./map_editor_handoff.js";
import { mapEditorLaunchConfig } from "./map_editor_launch.js";
import { MapEditorPanel } from "./map_editor_panel.js";
import { MapEditorSession, materializedMapsEqual } from "./map_editor_session.js";
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
        const returned = new MapEditorSession({ storage: null });
        returned.loadAuthoredMap(handoff.authoredMap);
        const restoredWorkspace = this.session.loadLocal(this.launch.workspaceId);
        if (!restoredWorkspace || !sessionsHaveSameMap(this.session, returned)) {
          this.session.loadAuthoredMap(handoff.authoredMap);
        }
      } catch (error) {
        this.session.initializeBlank();
        this.launch.error = error.message || String(error);
      }
    } else if (!this.session.loadLocal(this.launch.workspaceId)) {
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
      workspaceId: this.launch.workspaceId,
      onOpenLab: (map) => this.openInLab(map),
    });
    this.panel = panel;
    if (this.launch.error) this.panel.setStatus(this.launch.error, true);
    globalThis.__mapEditor = this;
  }

  async openInLab({ authoredMap, materializedMap, workspaceId }) {
    this.session.saveLocal(workspaceId);
    const handoff = await createMapHandoff({
      destination: "lab",
      authoredMap,
      materializedMap,
    });
    const url = new URL("/lab", this.locationObj.href);
    url.searchParams.set("handoff", handoff.handoffId);
    url.searchParams.set("workspace", workspaceId);
    this.allowUnload = true;
    window.location.assign(url.toString());
  }

  destroy() {
    window.removeEventListener("beforeunload", this.onBeforeUnload);
    this.panel?.destroy();
    this.viewport?.destroy();
    document.body.classList.remove("map-editor-mode");
  }
}

function sessionsHaveSameMap(left, right) {
  try {
    return materializedMapsEqual(left.materialized(), right.materialized());
  } catch {
    return false;
  }
}
