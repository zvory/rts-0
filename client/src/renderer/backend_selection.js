export const BABYLON_VERSION = "7.54.3";
export const BABYLON_SCRIPT_URL = `https://cdn.jsdelivr.net/npm/babylonjs@${BABYLON_VERSION}/babylon.js`;

export class RendererSelectionError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "RendererSelectionError";
    this.code = code;
  }
}
export function parseRendererSelection(locationLike = globalThis.location) {
  const params = new URLSearchParams(locationLike?.search || "");
  const value = params.get("rtsRenderer");
  if (value == null || value === "" || value === "pixi") return Object.freeze({ id: "pixi" });
  if (value !== "babylon") {
    throw new RendererSelectionError("invalidRenderer", "rtsRenderer must be either pixi or babylon.");
  }
  const pathname = locationLike?.pathname || "/";
  if (pathname !== "/lab" && pathname !== "/lab/") {
    throw new RendererSelectionError("unsupportedRendererRoute", "The Babylon renderer is currently available only in Lab.");
  }
  return Object.freeze({ id: "babylon" });
}

export async function loadBabylonDependency({
  documentLike = globalThis.document,
  globalLike = globalThis,
} = {}) {
  if (globalLike?.BABYLON) return validateBabylon(globalLike.BABYLON);
  if (!documentLike?.createElement || !documentLike?.head?.appendChild) {
    throw new RendererSelectionError("babylonUnavailable", "Babylon cannot load in this environment.");
  }
  const existing = documentLike.querySelector?.("script[data-rts-babylon]");
  if (existing) {
    await new Promise((resolve, reject) => {
      existing.addEventListener("load", resolve, { once: true });
      existing.addEventListener("error", () => reject(new RendererSelectionError(
        "babylonLoadFailed", "The pinned Babylon dependency failed to load.",
      )), { once: true });
    });
    return validateBabylon(globalLike.BABYLON);
  }
  await new Promise((resolve, reject) => {
    const script = documentLike.createElement("script");
    script.src = BABYLON_SCRIPT_URL;
    script.async = true;
    script.dataset.rtsBabylon = BABYLON_VERSION;
    script.addEventListener("load", resolve, { once: true });
    script.addEventListener("error", () => reject(new RendererSelectionError(
      "babylonLoadFailed", "The pinned Babylon dependency failed to load.",
    )), { once: true });
    documentLike.head.appendChild(script);
  });
  return validateBabylon(globalLike.BABYLON);
}

export async function createSelectedBackendBundle(options = {}) {
  const selection = parseRendererSelection(options.locationLike);
  if (selection.id === "pixi") {
    const { createPixiBackendBundle } = await import("./backend_bundle.js");
    return createPixiBackendBundle();
  }
  const Babylon = await loadBabylonDependency(options);
  const { createBabylonBackendBundle } = await import("./babylon/backend_bundle.js");
  return createBabylonBackendBundle({ Babylon });
}

export function showRendererBootstrapError(error, documentLike = globalThis.document) {
  const message = error instanceof RendererSelectionError
    ? error.message
    : "The selected renderer could not start.";
  const target = documentLike?.getElementById?.("toast") || documentLike?.getElementById?.("app");
  if (target) {
    target.textContent = message;
    target.hidden = false;
    target.setAttribute?.("role", "alert");
  }
  return message;
}

function validateBabylon(Babylon) {
  if (!Babylon?.Engine || !Babylon?.Scene || !Babylon?.FreeCamera || !Babylon?.Vector3) {
    throw new RendererSelectionError("babylonCapabilityMissing", "The pinned Babylon dependency is missing required capabilities.");
  }
  const actual = String(Babylon.Engine.Version || Babylon.Engine.version || "");
  if (actual && actual !== BABYLON_VERSION) {
    throw new RendererSelectionError("babylonVersionMismatch", `Expected Babylon ${BABYLON_VERSION}, received ${actual}.`);
  }
  return Babylon;
}
