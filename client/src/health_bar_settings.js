const ALWAYS_SHOW_HEALTH_BARS_STORAGE_KEY = "rts.healthBars.alwaysShow";

export function readAlwaysShowHealthBarsEnabled(storage = globalThis.localStorage) {
  try {
    return storage?.getItem(ALWAYS_SHOW_HEALTH_BARS_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function writeAlwaysShowHealthBarsEnabled(enabled, storage = globalThis.localStorage) {
  try {
    if (enabled) storage?.setItem(ALWAYS_SHOW_HEALTH_BARS_STORAGE_KEY, "1");
    else storage?.removeItem(ALWAYS_SHOW_HEALTH_BARS_STORAGE_KEY);
  } catch {
    // Storage failures only make this preference session-local.
  }
}
