const STORAGE_KEY = "rts.experimentalGraphics.v1";

export function readExperimentalGraphicsEnabled(storage = undefined) {
  try {
    return (storage === undefined ? globalThis.localStorage : storage)?.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function writeExperimentalGraphicsEnabled(enabled, storage = undefined) {
  try {
    const target = storage === undefined ? globalThis.localStorage : storage;
    if (enabled) target?.setItem(STORAGE_KEY, "1");
    else target?.removeItem(STORAGE_KEY);
  } catch {
    // A storage failure leaves the current session's choice usable.
  }
}
