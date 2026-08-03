const ALWAYS_SHOW_HEALTH_BARS_STORAGE_KEY = "rts.healthBars.alwaysShow";

export function readAlwaysShowHealthBarsEnabled(storage = undefined) {
  try {
    const target = storage === undefined ? globalThis.localStorage : storage;
    return target?.getItem(ALWAYS_SHOW_HEALTH_BARS_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function writeAlwaysShowHealthBarsEnabled(enabled, storage = undefined) {
  try {
    const target = storage === undefined ? globalThis.localStorage : storage;
    if (enabled) target?.setItem(ALWAYS_SHOW_HEALTH_BARS_STORAGE_KEY, "1");
    else target?.removeItem(ALWAYS_SHOW_HEALTH_BARS_STORAGE_KEY);
  } catch {
    // Storage failures only make this preference session-local.
  }
}
