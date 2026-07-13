const AUTO_SPECTATOR_ENABLED_STORAGE_KEY = "rts.autoSpectator.enabled";

export function readAutoSpectatorEnabled(storage = globalThis.localStorage) {
  try {
    return storage?.getItem(AUTO_SPECTATOR_ENABLED_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function writeAutoSpectatorEnabled(enabled, storage = globalThis.localStorage) {
  try {
    if (enabled) storage?.setItem(AUTO_SPECTATOR_ENABLED_STORAGE_KEY, "1");
    else storage?.removeItem(AUTO_SPECTATOR_ENABLED_STORAGE_KEY);
  } catch {
    // Storage failures only make this preference session-local.
  }
}
