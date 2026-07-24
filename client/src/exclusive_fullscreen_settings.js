const EXCLUSIVE_FULLSCREEN_STORAGE_KEY = "rts.windowsExclusiveFullscreen.enabled";
const EXCLUSIVE_FULLSCREEN_MARKER = "__RTS_EXCLUSIVE_FULLSCREEN_ENABLED";

export function readExclusiveFullscreenEnabled(storage = globalThis.localStorage) {
  try {
    return storage?.getItem(EXCLUSIVE_FULLSCREEN_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function writeExclusiveFullscreenEnabled(enabled, storage = globalThis.localStorage) {
  try {
    if (enabled) storage?.setItem(EXCLUSIVE_FULLSCREEN_STORAGE_KEY, "1");
    else storage?.removeItem(EXCLUSIVE_FULLSCREEN_STORAGE_KEY);
  } catch {
    // Storage failures only make this preference session-local.
  }
}

export function exclusiveFullscreenSupported(root = globalThis) {
  const runtime = root?.__RTS_DESKTOP_RUNTIME;
  return runtime?.shell === "tauri" &&
    runtime?.platform === "windows" &&
    runtime?.exclusiveFullscreenSupported === true &&
    typeof tauriInvokeFn(root) === "function";
}

export async function applyExclusiveFullscreen(enabled, root = globalThis) {
  if (!exclusiveFullscreenSupported(root)) {
    setExclusiveFullscreenMarker(false, root);
    return {
      supported: false,
      requested: false,
      active: false,
      mode: null,
    };
  }

  setExclusiveFullscreenMarker(enabled, root);
  try {
    return await tauriInvokeFn(root)("desktop_set_exclusive_fullscreen", {
      enabled: !!enabled,
    });
  } catch (err) {
    if (enabled) setExclusiveFullscreenMarker(false, root);
    throw err;
  }
}

export function exclusiveFullscreenPreferenceActive(root = globalThis) {
  return root?.[EXCLUSIVE_FULLSCREEN_MARKER] === true;
}

function setExclusiveFullscreenMarker(enabled, root) {
  if (!root) return;
  try {
    Object.defineProperty(root, EXCLUSIVE_FULLSCREEN_MARKER, {
      value: !!enabled,
      configurable: true,
      writable: true,
    });
  } catch {
    try {
      root[EXCLUSIVE_FULLSCREEN_MARKER] = !!enabled;
    } catch {}
  }
}

function tauriInvokeFn(root) {
  const candidates = [
    root?.__TAURI_INTERNALS__?.invoke,
    root?.__TAURI__?.core?.invoke,
    root?.__TAURI__?.tauri?.invoke,
    root?.__TAURI__?.invoke,
  ];
  return candidates.find((candidate) => typeof candidate === "function") || null;
}
