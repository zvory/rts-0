// Render / UI constants — mirror of the subset of `server/src/config.rs` the client needs.
// Gameplay is authoritative on the server; these values drive UI labels, the command
// card, fog sight radii, and rendering. Keep costs/supply/sight in sync with the server.

import { KIND } from "./protocol.js";

// Timing (for snapshot interpolation). Must match server TICK_HZ / SNAPSHOT_EVERY_N_TICKS.
export const TICK_HZ = 30;
export const SNAPSHOT_MS = 1000 / TICK_HZ; // expected ms between snapshots; used to compute interp alpha
export const SNAPSHOT_INTERP_DELAY_TICKS = 2; // render two snapshots behind to absorb receive jitter
export const INTERP_DELAY_MS = SNAPSHOT_MS * SNAPSHOT_INTERP_DELAY_TICKS;

// Palette ------------------------------------------------------------------
export const COLORS = Object.freeze({
  bgVoid: 0x11110f, // outside the map
  grass: 0x59633f, // base terrain
  grassAlt: 0x66704a, // dither alternate
  field: 0x746947,
  mud: 0x4b3e32,
  rock: 0x6a6659,
  water: 0x2f5560,
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

// Mirrors server/src/config.rs TANK_BODY_* values. Server collision is authoritative;
// the client uses these only for tank art, selection, and advisory placement previews.
export const TANK_BODY = Object.freeze({
  length: 50.4,
  width: 28.8,
  clearance: 1.5,
});
export const SCOUT_CAR_BODY = Object.freeze({
  length: 40.8,
  width: 21.6,
  clearance: 1.0,
});

// Workers can mine a resource only when a completed City Centre is within this many tiles.
export const MINING_CC_RANGE_TILES = 7.0;
export const AT_GUN_DEPLOYED_RANGE_TILES = 7;
export const AT_GUN_FIELD_OF_FIRE_RAD = Math.PI / 6;

// Player colors (server assigns from a matching palette; used as a fallback for blips).
export const PLAYER_PALETTE = Object.freeze([
  "#4878c8", "#c84848", "#30a090", "#8040c8",
  "#c83880", "#c87830", "#409840", "#c8b030",
]);

// Per-kind UI / render info. `size` is the render radius (units) or half-extent hint.
// `sight` (tiles) drives the local fog overlay. `rangeTiles` mirrors weapon range for visuals.
// `cost`/`supply` drive the command card.
export const STATS = Object.freeze({
  [KIND.WORKER]: { label: "Engineer", icon: "EN", size: 9, sight: 7,
    rangeTiles: 1, cost: { steel: 50, oil: 0 }, supply: 1, buildTicks: 360 },
  [KIND.RIFLEMAN]: { label: "Rifleman", icon: "RF", size: 9, sight: 8,
    rangeTiles: 4, cost: { steel: 50, oil: 0 }, supply: 1, buildTicks: 300 },
  [KIND.MACHINE_GUNNER]: { label: "Machine Gunner", icon: "MG", size: 10, sight: 8,
    rangeTiles: 5, cost: { steel: 75, oil: 25 }, supply: 2, buildTicks: 400, requires: KIND.TRAINING_CENTRE },
  [KIND.AT_TEAM]: { label: "AT Gun", icon: "AT", size: 20, sight: 8,
    rangeTiles: AT_GUN_DEPLOYED_RANGE_TILES, cost: { steel: 75, oil: 25 }, supply: 3, buildTicks: 440, requires: KIND.STEELWORKS },
  [KIND.SCOUT_CAR]: { label: "Scout Car", icon: "SC", size: 14.4, sight: 10, body: SCOUT_CAR_BODY,
    rangeTiles: 5, cost: { steel: 125, oil: 75 }, supply: 3, buildTicks: 480 },
  [KIND.TANK]: { label: "Tank", icon: "TK", size: 18, sight: 7, body: TANK_BODY,
    rangeTiles: 3, cost: { steel: 200, oil: 150 }, supply: 6, buildTicks: 750, requires: KIND.STEELWORKS },

  [KIND.CITY_CENTRE]: { label: "City Centre", icon: "CC", footW: 3, footH: 3, sight: 9,
    cost: { steel: 200, oil: 0 }, buildTicks: 400, trains: [KIND.WORKER] },
  [KIND.DEPOT]: { label: "Supply Depot", icon: "SD", footW: 2, footH: 2, sight: 4,
    cost: { steel: 100, oil: 0 }, buildTicks: 180, trains: [] },
  [KIND.BARRACKS]: { label: "Barracks", icon: "BK", footW: 3, footH: 2, sight: 6,
    cost: { steel: 150, oil: 0 }, buildTicks: 200, trains: [KIND.RIFLEMAN, KIND.MACHINE_GUNNER, KIND.AT_TEAM], requires: KIND.CITY_CENTRE },
  [KIND.TRAINING_CENTRE]: { label: "Training Centre", icon: "TC", footW: 3, footH: 2, sight: 6,
    cost: { steel: 100, oil: 50 }, buildTicks: 220, trains: [],
    requires: [KIND.CITY_CENTRE, KIND.BARRACKS] },
  [KIND.FACTORY]: { label: "Factory", icon: "FCT", footW: 3, footH: 3, sight: 6,
    cost: { steel: 200, oil: 100 }, buildTicks: 240, trains: [KIND.SCOUT_CAR, KIND.TANK],
    requires: [KIND.CITY_CENTRE, KIND.TRAINING_CENTRE] },
  [KIND.STEELWORKS]: { label: "Steelworks", icon: "SW", footW: 2, footH: 2, sight: 6,
    cost: { steel: 125, oil: 125 }, buildTicks: 220, trains: [],
    requires: [KIND.CITY_CENTRE, KIND.TRAINING_CENTRE] },

  [KIND.STEEL]: { label: "Steel", size: 22 },
  [KIND.OIL]: { label: "Oil", size: 14 },
});

// A building that trains units — the only buildings that accept a rally point.
export const isProducerBuilding = (kind) =>
  Array.isArray(STATS[kind]?.trains) && STATS[kind].trains.length > 0;

export const RESOURCE_AMOUNTS = Object.freeze({
  [KIND.STEEL]: 1500,
  [KIND.OIL]: 5000,
});

// What a worker can build (command card when a worker is selected).
export const WORKER_BUILDABLE = Object.freeze([
  KIND.CITY_CENTRE,
  KIND.DEPOT,
  KIND.BARRACKS,
  KIND.TRAINING_CENTRE,
  KIND.FACTORY,
  KIND.STEELWORKS,
]);

// Camera defaults.
export const CAMERA = Object.freeze({
  minZoom: 0.4,
  maxZoom: 2.0,
  panSpeed: 900, // world px / sec at zoom 1
  edgeScrollPx: 14, // screen-edge band that triggers panning
});
