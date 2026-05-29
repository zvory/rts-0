// Render / UI constants — mirror of the subset of `server/src/config.rs` the client needs.
// Gameplay is authoritative on the server; these values drive UI labels, the command
// card, fog sight radii, and rendering. Keep costs/supply/sight in sync with the server.

import { KIND } from "./protocol.js";

// Timing (for snapshot interpolation). Must match server TICK_HZ / SNAPSHOT_EVERY_N_TICKS.
export const TICK_HZ = 30;
export const SNAPSHOT_MS = 1000 / TICK_HZ; // expected ms between snapshots; used to compute interp alpha
export const INTERP_DELAY_MS = SNAPSHOT_MS; // render this far in the past for smooth interpolation

// Palette ------------------------------------------------------------------
export const COLORS = Object.freeze({
  bgVoid: 0x05070d, // outside the map
  grass: 0x2e6b3a, // base terrain
  grassAlt: 0x3b7d42, // checker alternate
  rock: 0x7a6250,
  water: 0x115783,
  grid: 0x000000,
  minerals: 0x46e3ff,
  gas: 0x53e08a,
  shadow: 0x000000,
  hpBack: 0x101010,
  hpGood: 0x4fd24f,
  hpMid: 0xe0c23a,
  hpLow: 0xe04646,
  selectOwn: 0x6cff6c,
  selectEnemy: 0xff6c6c,
  selectNeutral: 0xf0d24a,
  dragBox: 0x6cff6c,
  placeOk: 0x6cff6c,
  placeBad: 0xff5555,
  fogUnexplored: 0x05070d,
  fogExplored: 0x000000, // drawn at fogExploredAlpha
});
export const FOG_EXPLORED_ALPHA = 0.55;
export const FOG_UNEXPLORED_ALPHA = 0.82;

// Player colors (server assigns from a matching palette; used as a fallback for blips).
export const PLAYER_PALETTE = Object.freeze([
  "#3aa0ff", "#ff5a4d", "#46d36b", "#f0c64a", "#b96cff", "#ff9a3c",
]);

// Per-kind UI / render info. `size` is the render radius (units) or half-extent hint.
// `sight` (tiles) drives the local fog overlay. `cost`/`supply` drive the command card.
export const STATS = Object.freeze({
  [KIND.WORKER]: { label: "Worker", hotkey: "W", icon: "⛏", size: 9, sight: 7,
    cost: { min: 50, gas: 0 }, supply: 1, buildTicks: 120 },
  [KIND.SOLDIER]: { label: "Soldier", hotkey: "A", icon: "⚔", size: 9, sight: 8,
    cost: { min: 50, gas: 0 }, supply: 1, buildTicks: 150 },
  [KIND.HEAVY]: { label: "Heavy", hotkey: "D", icon: "✦", size: 13, sight: 7,
    cost: { min: 100, gas: 50 }, supply: 2, buildTicks: 250 },

  [KIND.HQ]: { label: "HQ", hotkey: "H", icon: "🏛", footW: 3, footH: 3, sight: 9,
    cost: { min: 400, gas: 0 }, buildTicks: 400, trains: [KIND.WORKER] },
  [KIND.DEPOT]: { label: "Supply Depot", hotkey: "S", icon: "▣", footW: 2, footH: 2, sight: 4,
    cost: { min: 50, gas: 0 }, buildTicks: 120, trains: [] },
  [KIND.BARRACKS]: { label: "Barracks", hotkey: "B", icon: "⚒", footW: 3, footH: 2, sight: 6,
    cost: { min: 100, gas: 0 }, buildTicks: 200, trains: [KIND.SOLDIER, KIND.HEAVY], requires: KIND.HQ },
  [KIND.TURRET]: { label: "Turret", hotkey: "T", icon: "◎", footW: 1, footH: 1, sight: 6,
    cost: { min: 75, gas: 0 }, buildTicks: 120, trains: [] },

  [KIND.MINERALS]: { label: "Minerals", size: 11 },
  [KIND.GAS]: { label: "Gas", size: 14 },
});

// What a worker can build (command card when a worker is selected).
export const WORKER_BUILDABLE = Object.freeze([KIND.HQ, KIND.DEPOT, KIND.BARRACKS, KIND.TURRET]);

// Camera defaults.
export const CAMERA = Object.freeze({
  minZoom: 0.4,
  maxZoom: 2.0,
  panSpeed: 900, // world px / sec at zoom 1
  edgeScrollPx: 14, // screen-edge band that triggers panning
});
