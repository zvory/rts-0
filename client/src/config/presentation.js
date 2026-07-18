// Client-owned presentation constants: rendering palette, local fog opacity,
// camera defaults, and command-card layout.

import { KIND } from "../protocol.js";

export const COLORS = Object.freeze({
  bgVoid: 0x11110f, // outside the map
  grass: 0x59633f, // base terrain
  grassAlt: 0x66704a, // dither alternate
  field: 0x746947,
  mud: 0x4b3e32,
  rock: 0x6a6659,
  water: 0x2f5560,
  road: 0x30312f,
  roadAlt: 0x393a36,
  roadShoulder: 0x5a4934,
  roadShoulderDark: 0x40352a,
  roadLine: 0xd0aa32,
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

export const CAMERA = Object.freeze({
  minZoom: 0.4,
  maxZoom: 2.0,
  labMaxZoom: 8.0,
  maxVisibleTilesPerAxis: 100,
  panSpeed: 900, // world px / sec at zoom 1
  edgeScrollPx: 14, // screen-edge band that triggers panning
});

// Pump Jack occupies the former Supply Depot W slot so its economy role stays
// legible even though workers can also build one by right-clicking an oil patch.
export const WORKER_BUILD_CARD_SLOTS = Object.freeze([
  KIND.CITY_CENTRE,
  KIND.PUMP_JACK,
  KIND.BARRACKS,
  KIND.TRAINING_CENTRE,
  KIND.RESEARCH_COMPLEX,
  KIND.FACTORY,
  KIND.STEELWORKS,
  KIND.TANK_TRAP,
]);
