import { createMapHandoff } from "./map_editor_handoff.js";

const PREVIEW_SIZE = 512;
const COPY_SIZE = 2048;
const READY_TIMEOUT_MS = 45_000;
const READY_POLL_MS = 50;

/** Own the Map Editor's authoritative minimap hover preview and clipboard copy flow. */
export class MapEditorMinimapPreview {
  constructor({
    root,
    locationObj = window.location,
    documentObj = document,
    createHandoff = createMapHandoff,
    clipboard = globalThis.navigator?.clipboard,
    ClipboardItemCtor = globalThis.ClipboardItem,
  }) {
    if (!root) throw new TypeError("Map Editor minimap preview requires a root element.");
    this.root = root;
    this.locationObj = locationObj;
    this.documentObj = documentObj;
    this.createHandoff = createHandoff;
    this.clipboard = clipboard;
    this.ClipboardItemCtor = ClipboardItemCtor;
    this.destroyed = false;
    this.mapKey = "";
    this.frame = null;
    this.bridgePromise = null;
    this.captureQueue = Promise.resolve();
    this.generation = 0;
    this.visibleRequest = 0;

    this.popover = documentObj.createElement("aside");
    this.popover.className = "map-editor-minimap-preview";
    this.popover.hidden = true;
    this.popover.setAttribute("role", "tooltip");
    this.popover.setAttribute("aria-label", "Minimap preview");
    this.image = documentObj.createElement("img");
    this.image.alt = "Current authored map minimap preview";
    this.status = documentObj.createElement("span");
    this.status.textContent = "Preparing minimap preview…";
    this.popover.append(this.image, this.status);
    root.appendChild(this.popover);
  }

  show(anchor, payload) {
    if (this.destroyed || !anchor) return;
    const request = ++this.visibleRequest;
    this._position(anchor);
    this.popover.hidden = false;
    this.image.hidden = true;
    this.status.hidden = false;
    this.status.textContent = "Preparing minimap preview…";
    void this._capture(payload, PREVIEW_SIZE).then((pngDataUrl) => {
      if (this.destroyed || this.popover.hidden || request !== this.visibleRequest) return;
      this.image.src = pngDataUrl;
      this.image.hidden = false;
      this.status.hidden = true;
    }).catch((error) => {
      if (this.destroyed || this.popover.hidden || request !== this.visibleRequest) return;
      this.status.textContent = `Preview unavailable: ${error?.message || String(error)}`;
    });
  }

  hide() {
    this.visibleRequest += 1;
    this.popover.hidden = true;
  }

  async copy(payload) {
    if (this.destroyed) throw new Error("Map Editor minimap preview is closed.");
    if (typeof this.clipboard?.write !== "function" || typeof this.ClipboardItemCtor !== "function") {
      throw new Error("This browser does not support copying PNG images to the clipboard.");
    }
    // Start the clipboard write in the click's user-activation turn. ClipboardItem accepts a
    // promised Blob, so the authoritative capture may finish without losing that activation.
    const pngBlob = this._capture(payload, COPY_SIZE).then(pngDataUrlToBlob);
    await this.clipboard.write([
      new this.ClipboardItemCtor({ "image/png": pngBlob }),
    ]);
  }

  invalidate() {
    this._discardFrame();
    this.hide();
  }

  _discardFrame() {
    this.generation += 1;
    this.mapKey = "";
    this.bridgePromise = null;
    this.frame?.remove();
    this.frame = null;
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    this.invalidate();
    this.popover.remove();
  }

  _position(anchor) {
    const rect = anchor.getBoundingClientRect?.();
    if (!rect) return;
    const viewportWidth = Number(globalThis.innerWidth) || this.documentObj.documentElement?.clientWidth || 0;
    this.popover.style.top = `${Math.round(rect.bottom + 8)}px`;
    this.popover.style.right = `${Math.max(8, Math.round(viewportWidth - rect.right))}px`;
  }

  _capture(payload, size) {
    const task = this.captureQueue.then(async () => {
      const bridge = await this._bridge(payload);
      const result = await bridge.call("capture", { width: size, height: size });
      return result.pngDataUrl;
    });
    this.captureQueue = task.catch(() => {});
    return task;
  }

  _bridge(payload) {
    const key = JSON.stringify(payload?.authoredMap || null);
    if (!key || key === "null") return Promise.reject(new Error("The map is not ready to preview."));
    if (key === this.mapKey && this.bridgePromise) return this.bridgePromise;
    this._discardFrame();
    const generation = this.generation;
    this.mapKey = key;
    this.bridgePromise = this._createBridge(payload, key, generation).catch((error) => {
      if (this.mapKey === key) {
        this.mapKey = "";
        this.bridgePromise = null;
      }
      throw error;
    });
    return this.bridgePromise;
  }

  async _createBridge({ authoredMap, materializedMap }, key, generation) {
    const handoff = await this.createHandoff({
      destination: "lab",
      authoredMap,
      materializedMap,
    });
    if (this.destroyed) throw new Error("Map Editor minimap preview is closed.");
    if (this.mapKey !== key || this.generation !== generation) {
      throw new Error("The map changed while its minimap preview was preparing.");
    }
    const frame = this.documentObj.createElement("iframe");
    frame.className = "map-editor-minimap-preview-frame";
    frame.tabIndex = -1;
    frame.setAttribute("aria-hidden", "true");
    frame.title = "Minimap preview renderer";
    const url = new URL("/map-preview", this.locationObj.href);
    url.searchParams.set("handoff", handoff.handoffId);
    frame.src = url.toString();
    this.frame = frame;
    this.root.appendChild(frame);
    return waitForPreviewBridge(frame, READY_TIMEOUT_MS, () => this.destroyed || this.generation !== generation);
  }
}

export function pngDataUrlToBlob(dataUrl) {
  const match = /^data:image\/png;base64,([A-Za-z0-9+/=]+)$/.exec(String(dataUrl || ""));
  if (!match) throw new Error("The minimap renderer returned an invalid PNG.");
  const binary = globalThis.atob(match[1]);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index++) bytes[index] = binary.charCodeAt(index);
  return new Blob([bytes], { type: "image/png" });
}

async function waitForPreviewBridge(frame, timeoutMs, cancelled) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (cancelled()) throw new Error("The map changed while its minimap preview was preparing.");
    const bridge = frame.contentWindow?.__rtsMapPreview;
    const status = bridge?.status?.();
    if (status?.state === "ready") return bridge;
    if (status?.state === "failed") throw new Error(status.error || "Minimap preview failed to start.");
    await new Promise((resolve) => setTimeout(resolve, READY_POLL_MS));
  }
  throw new Error("Minimap preview timed out while starting.");
}
