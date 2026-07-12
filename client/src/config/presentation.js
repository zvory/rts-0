// Client-owned presentation constants: rendering palette, local fog opacity,
// fallback player colors, camera defaults, and command-card layout.

import { KIND } from "../protocol.js";

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
  selectAlly: 0x7ab8d0,
  selectEnemy: 0xd47a5f,
  selectNeutral: 0xc9b56a,
  dragBox: 0xc7d07a,
  placeOk: 0xc7d07a,
  placeBad: 0xd47a5f,
  trenchShadow: 0x20140d,
  trenchDirt: 0x5a3822,
  trenchDirtLight: 0x6f5136,
  trenchRim: 0x3f2919,
  fogUnexplored: 0x11110f,
  fogExplored: 0x000000, // drawn at fogExploredAlpha
});

export const FOG_EXPLORED_ALPHA = 0.48;
export const FOG_UNEXPLORED_ALPHA = 0.8;

// Player colors (server assigns from a matching palette; used as a fallback for blips).
export const PLAYER_PALETTE = Object.freeze([
  "#0072b2", "#d55e00", "#009e73", "#cc79a7",
  "#56b4e9", "#e69f00", "#f0e442", "#7e57c2",
]);

export const CAMERA = Object.freeze({
  minZoom: 0.4,
  maxZoom: 2.0,
  labMaxZoom: 8.0,
  panSpeed: 900, // world px / sec at zoom 1
  edgeScrollPx: 14, // screen-edge band that triggers panning
});

// Keep the former Supply Depot W slot visibly empty during the depot-free supply experiment.
export const WORKER_BUILD_CARD_SLOTS = Object.freeze([
  KIND.CITY_CENTRE,
  null,
  KIND.BARRACKS,
  KIND.TRAINING_CENTRE,
  KIND.RESEARCH_COMPLEX,
  KIND.FACTORY,
  KIND.STEELWORKS,
  KIND.TANK_TRAP,
]);
