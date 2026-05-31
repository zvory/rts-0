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
  bgVoid: 0x11110f, // outside the map
  grass: 0x59633f, // base terrain
  grassAlt: 0x66704a, // dither alternate
  field: 0x746947,
  mud: 0x4b3e32,
  rock: 0x6a6659,
  water: 0x2f5560,
  forest: 0x3a4a2e,
  grid: 0x000000,
  steel: 0x9a9a9a,
  oil: 0x111111,
  shadow: 0x000000,
  hpBack: 0x101010,
  hpGood: 0x7ca45a,
  hpMid: 0xc7a24a,
  hpLow: 0xb64a3f,
  selectOwn: 0xc7d07a,
  selectEnemy: 0xd47a5f,
  selectNeutral: 0xc9b56a,
  dragBox: 0xc7d07a,
  placeOk: 0xc7d07a,
  placeBad: 0xd47a5f,
  fogUnexplored: 0x11110f,
  fogExplored: 0x000000, // drawn at fogExploredAlpha
});
export const FOG_EXPLORED_ALPHA = 0.55;
export const FOG_UNEXPLORED_ALPHA = 0.86;

// Player colors (server assigns from a matching palette; used as a fallback for blips).
export const PLAYER_PALETTE = Object.freeze([
  "#6f8fa8", "#b2775f", "#7f9462", "#c0a463", "#8d7aa8", "#b68a58",
]);

// Per-kind UI / render info. `size` is the render radius (units) or half-extent hint.
// `sight` (tiles) drives the local fog overlay. `rangeTiles` mirrors weapon range for visuals.
// `cost`/`supply` drive the command card.
export const STATS = Object.freeze({
  [KIND.WORKER]: { label: "Engineer", icon: "EN", size: 9, sight: 7,
    rangeTiles: 1, cost: { steel: 50, oil: 0 }, supply: 1, buildTicks: 120 },
  [KIND.RIFLEMAN]: { label: "Rifleman", icon: "RF", size: 9, sight: 8,
    rangeTiles: 4, cost: { steel: 50, oil: 0 }, supply: 1, buildTicks: 150 },
  [KIND.MACHINE_GUNNER]: { label: "Machine Gunner", icon: "MG", size: 10, sight: 8,
    rangeTiles: 5, cost: { steel: 75, oil: 25 }, supply: 2, buildTicks: 200, requires: KIND.TRAINING_CENTRE },
  [KIND.AT_TEAM]: { label: "AT Team", icon: "AT", size: 10, sight: 8,
    rangeTiles: 4, cost: { steel: 75, oil: 25 }, supply: 2, buildTicks: 220, requires: KIND.TRAINING_CENTRE },
  [KIND.TANK]: { label: "Tank", icon: "TK", size: 26, sight: 7,
    rangeTiles: 3, cost: { steel: 200, oil: 100 }, supply: 2, buildTicks: 250 },

  [KIND.INDUSTRIAL_CENTER]: { label: "Industrial Center", icon: "IC", footW: 3, footH: 3, sight: 9,
    cost: { steel: 400, oil: 0 }, buildTicks: 400, trains: [KIND.WORKER] },
  [KIND.DEPOT]: { label: "Supply Depot", icon: "SD", footW: 2, footH: 2, sight: 4,
    cost: { steel: 50, oil: 0 }, buildTicks: 120, trains: [] },
  [KIND.BARRACKS]: { label: "Barracks", icon: "BK", footW: 3, footH: 2, sight: 6,
    cost: { steel: 100, oil: 0 }, buildTicks: 200, trains: [KIND.RIFLEMAN, KIND.MACHINE_GUNNER, KIND.AT_TEAM], requires: KIND.INDUSTRIAL_CENTER },
  [KIND.TRAINING_CENTRE]: { label: "Training Centre", icon: "TC", footW: 3, footH: 2, sight: 6,
    cost: { steel: 100, oil: 50 }, buildTicks: 220, trains: [], requires: KIND.INDUSTRIAL_CENTER },
  [KIND.TANK_FACTORY]: { label: "Tank Factory", icon: "TF", footW: 3, footH: 3, sight: 6,
    cost: { steel: 200, oil: 100 }, buildTicks: 240, trains: [KIND.TANK], requires: KIND.INDUSTRIAL_CENTER },

  [KIND.STEEL]: { label: "Steel", size: 22 },
  [KIND.OIL]: { label: "Oil", size: 14 },
});

// What a worker can build (command card when a worker is selected).
export const WORKER_BUILDABLE = Object.freeze([
  KIND.INDUSTRIAL_CENTER,
  KIND.DEPOT,
  KIND.BARRACKS,
  KIND.TRAINING_CENTRE,
  KIND.TANK_FACTORY,
]);

// Camera defaults.
export const CAMERA = Object.freeze({
  minZoom: 0.4,
  maxZoom: 2.0,
  panSpeed: 900, // world px / sec at zoom 1
  edgeScrollPx: 14, // screen-edge band that triggers panning
});
