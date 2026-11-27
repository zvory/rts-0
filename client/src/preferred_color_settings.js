import { PLAYER_PALETTE } from "./config.js";

const STORAGE_KEY = "rts.player.preferredColor";

export function createPreferredColorSettings(onChange) {
  let color = null;
  try {
    const stored = globalThis.localStorage?.getItem(STORAGE_KEY);
    if (PLAYER_PALETTE.includes(stored)) color = stored;
  } catch { /* Keep the default when storage is unavailable. */ }
  return {
    read: () => color,
    write(value) {
      if (value !== null && !PLAYER_PALETTE.includes(value)) return;
      color = value;
      try {
        if (color === null) globalThis.localStorage?.removeItem(STORAGE_KEY);
        else globalThis.localStorage?.setItem(STORAGE_KEY, color);
      } catch { /* The current session still keeps the preference. */ }
      onChange?.(color);
    },
  };
}
