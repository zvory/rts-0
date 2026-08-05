// Narrow, launch-gated bridge for Interact Map Editor inspection and capture.
// It exposes map facts and camera controls, never the editor/session/renderer objects.

const TILE_SIZE = 32;
const BRIDGE_KEY = "__rtsInteract";
const BRIDGE_VERSION = 1;

export class MapEditorInteractBridge {
  constructor({ app, windowLike = globalThis.window, fetchImpl = (...args) => globalThis.fetch(...args) } = {}) {
    this.app = app;
    this.windowLike = windowLike;
    this.fetchImpl = fetchImpl;
    this.destroyed = false;
    this.state = "starting";
    this.error = "";
    this.surface = Object.freeze({
      version: BRIDGE_VERSION,
      status: () => this.status(),
      call: (method, input) => this.call(method, input),
    });
    if (this.windowLike) this.windowLike[BRIDGE_KEY] = this.surface;
  }

  async initialize(mapFile) {
    try {
      const response = await this.fetchImpl(`/maps/${encodeURIComponent(mapFile)}`, { cache: "no-store" });
      if (!response.ok) throw new Error(`Bundled map ${mapFile} returned HTTP ${response.status}.`);
      this.app.panel.loadMapData(await response.json());
      this.app.panel.selectedMapFile = mapFile;
      this.app.panel.render();
      await animationFrames(3);
      this.state = "ready";
      return this.status();
    } catch (error) {
      this.state = "failed";
      this.error = boundedMessage(error);
      throw error;
    }
  }

  status() {
    const draft = this.app?.session?.draft;
    const viewport = this.app?.viewport;
    const reason = this.destroyed ? "bridgeClosed"
      : this.state === "failed" ? "launchError"
        : this.state !== "ready" ? "waitingForMap" : "ready";
    return {
      version: BRIDGE_VERSION,
      enabled: !this.destroyed,
      ready: reason === "ready",
      reason,
      launchError: this.error,
      mode: "map-editor",
      map: draft ? projectMap(draft) : null,
      frame: viewport?.presentationFrameId || 0,
      camera: viewport?.cameraSnapshot?.() || null,
      cameraViewport: viewport?.cameraViewportSnapshot?.() || null,
      cameraWorldBounds: viewport?.cameraWorldBoundsSnapshot?.() || null,
    };
  }

  async call(method, input = {}) {
    try {
      let value;
      if (method === "status") value = this.status();
      else if (method === "inspect") value = this.inspect();
      else if (method === "camera") value = await this.camera(input);
      else if (method === "presentation") value = this.presentation(input);
      else if (method === "captureReadiness") value = this.captureReadiness();
      else throw bridgeError("unknownMethod", `Unknown Interact Map Editor bridge method ${JSON.stringify(method)}.`);
      return { ok: true, value };
    } catch (error) {
      return { ok: false, error: { code: error?.code || "bridgeError", message: boundedMessage(error) } };
    }
  }

  session() {
    const status = this.status();
    if (!status.ready) throw bridgeError(status.reason, `Interact Map Editor is not ready: ${status.reason}.`);
    return { draft: this.app.session.draft, viewport: this.app.viewport };
  }

  inspect() {
    const { draft } = this.session();
    return { ...this.status(), map: projectMap(draft) };
  }

  async camera(input) {
    const { draft, viewport } = this.session();
    const result = await viewport.controlInteractCamera(input);
    return {
      ...result,
      map: projectMap(draft),
    };
  }

  presentation(input) {
    const clean = input?.mode === "clean";
    document.body.classList.toggle("map-editor-interact-clean", clean);
    return { mode: clean ? "clean" : "default" };
  }

  captureReadiness() {
    const { viewport } = this.session();
    const assets = viewport.presentation?.host?.captureReadiness?.() || {};
    const pending = !!viewport.presentationInFlight || !!viewport.pendingTerrainUpdate
      || !!viewport.pendingOverlay || !!viewport.pendingDoodadUpdate;
    return {
      ...this.status(),
      ready: !pending && assets.ready !== false,
      phase: "editing",
      assets,
      failedAssets: assets.failedAssets || [],
      pendingAssets: assets.pendingAssets || [],
      frameErrors: [],
      renderErrors: viewport.presentationStopped ? [{ message: "Map Editor presentation stopped." }] : [],
      missingTextureSubjectIds: [],
    };
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    document.body.classList.remove("map-editor-interact-clean");
    if (this.windowLike?.[BRIDGE_KEY] === this.surface) delete this.windowLike[BRIDGE_KEY];
  }
}

function projectMap(draft) {
  return {
    name: String(draft.name || "Map"),
    width: draft.width,
    height: draft.height,
    tileSize: TILE_SIZE,
    starts: Array.isArray(draft.startLocations) ? draft.startLocations.length : 0,
    baseSites: Array.isArray(draft.baseSites) ? draft.baseSites.length : 0,
  };
}

function animationFrames(count) {
  return new Promise((resolve) => {
    const next = () => count-- <= 0 ? resolve() : requestAnimationFrame(next);
    next();
  });
}

function bridgeError(code, message) { return Object.assign(new Error(message), { code }); }
function boundedMessage(error) { return String(error?.message || error || "Map Editor bridge failed.").slice(0, 1000); }
