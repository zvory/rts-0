const UNIT_RANGES_ENABLED_STORAGE_KEY = "rts.unitRanges.enabled";

export function readUnitRangesEnabled(storage = globalThis.localStorage) {
  try {
    return storage?.getItem(UNIT_RANGES_ENABLED_STORAGE_KEY) !== "0";
  } catch {
    return true;
  }
}

export function writeUnitRangesEnabled(enabled, storage = globalThis.localStorage) {
  try {
    if (enabled) storage?.removeItem(UNIT_RANGES_ENABLED_STORAGE_KEY);
    else storage?.setItem(UNIT_RANGES_ENABLED_STORAGE_KEY, "0");
  } catch {
    // Storage failures only make this preference session-local.
  }
}
