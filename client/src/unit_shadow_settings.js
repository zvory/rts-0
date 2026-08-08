const PROJECTED_UNIT_SHADOWS_STORAGE_KEY = "rts.unitShadows.projected";

export function readProjectedUnitShadowsEnabled(storage = undefined) {
  try {
    const target = storage === undefined ? globalThis.localStorage : storage;
    return target?.getItem(PROJECTED_UNIT_SHADOWS_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function writeProjectedUnitShadowsEnabled(enabled, storage = undefined) {
  try {
    const target = storage === undefined ? globalThis.localStorage : storage;
    if (enabled) target?.setItem(PROJECTED_UNIT_SHADOWS_STORAGE_KEY, "1");
    else target?.removeItem(PROJECTED_UNIT_SHADOWS_STORAGE_KEY);
  } catch {
    // Storage failures only make this preference session-local.
  }
}
