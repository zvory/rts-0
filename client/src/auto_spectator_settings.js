const AUTO_SPECTATOR_ENABLED_STORAGE_KEY = "rts.autoSpectator.enabled";

export function initialAutoSpectatorEnabled({
  interactLaunch = false,
  storage = globalThis.localStorage,
} = {}) {
  if (!interactLaunch) return false;
  try {
    return storage?.getItem(AUTO_SPECTATOR_ENABLED_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}
