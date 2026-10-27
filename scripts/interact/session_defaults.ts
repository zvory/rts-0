export const DEFAULT_GAME_AI_PROFILE_ID = "ai_2_3";
export const GAME_AI_PROFILE_IDS = Object.freeze(["ai_2_3", "ai_2_1", "ai_turtle", "jeffs_ai", "rush_ai"]);

export const DEFAULT_LAB_MAP = "Classic";
export const DEFAULT_GAME_MAP = "Chokes";
export const DEFAULT_MAP_EDITOR_MAP = "1v1.json";

export function defaultMapForMode(mode: "lab" | "game" | "scenario" | "map-editor") {
  return mode === "lab" ? DEFAULT_LAB_MAP : mode === "map-editor" ? DEFAULT_MAP_EDITOR_MAP : DEFAULT_GAME_MAP;
}
