const PROJECTED_UNIT_SHADOWS_STORAGE_KEY = "rts.unitShadows.projected";

export function readProjectedUnitShadowsEnabled(storage = undefined) {
  try {
    const target = storage === undefined ? globalThis.localStorage : storage;
    return target?.getItem(PROJECTED_UNIT_SHADOWS_STORAGE_KEY) !== "0";
  } catch {
    return true;
  }
}

export function writeProjectedUnitShadowsEnabled(enabled, storage = undefined) {
  try {
    const target = storage === undefined ? globalThis.localStorage : storage;
    if (enabled) target?.removeItem(PROJECTED_UNIT_SHADOWS_STORAGE_KEY);
    else target?.setItem(PROJECTED_UNIT_SHADOWS_STORAGE_KEY, "0");
  } catch {
    // Storage failures only make this preference session-local.
  }
}
