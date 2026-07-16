export const DEFAULT_LAB_MAP = "1v1";
export const DEFAULT_GAME_MAP = "Chokes";

export function defaultMapForMode(mode: "lab" | "game" | "scenario") {
  return mode === "lab" ? DEFAULT_LAB_MAP : DEFAULT_GAME_MAP;
}
