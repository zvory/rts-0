// Client-owned presentation constants: rendering palette, local fog opacity,
// camera defaults, and command-card layout.

import { KIND, TERRAIN } from "../protocol.js";

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
  minimapRoadLine: 0xf1cb43,
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

export const TERRAIN_VARIANT_PALETTES = Object.freeze({
  [TERRAIN.GRAVEL_A]: Object.freeze({ base: 0x66685f, alt: 0x74766c, details: [0x939185, 0x494c48], pattern: "gravel" }),
  [TERRAIN.GRAVEL_B]: Object.freeze({ base: 0x7b705d, alt: 0x8c806a, details: [0xaaa087, 0x5a5246], pattern: "gravel" }),
  [TERRAIN.GRAVEL_C]: Object.freeze({ base: 0x8a8777, alt: 0x9b9787, details: [0xb9b4a0, 0x666357], pattern: "gravel" }),
  [TERRAIN.DIRT_A]: Object.freeze({ base: 0x70543b, alt: 0x806148, details: [0x9a7755, 0x4f3c2c], pattern: "dirt" }),
  [TERRAIN.DIRT_B]: Object.freeze({ base: 0x78503d, alt: 0x8b5e47, details: [0xa97958, 0x55372d], pattern: "dirt" }),
  [TERRAIN.DIRT_C]: Object.freeze({ base: 0x806d4a, alt: 0x927c55, details: [0xad9465, 0x5e5038], pattern: "dirt" }),
  [TERRAIN.MUD_A]: Object.freeze({ base: 0x49382c, alt: 0x594536, details: [0x2d241e, 0x6b5542], pattern: "mud", activity: 0.26 }),
  [TERRAIN.MUD_B]: Object.freeze({ base: 0x454235, alt: 0x534f3e, details: [0x292922, 0x77725b], pattern: "mud", activity: 0.33 }),
  [TERRAIN.MUD_C]: Object.freeze({ base: 0x503a32, alt: 0x5f463d, details: [0x302621, 0x73574a], pattern: "mud", activity: 0.4 }),
  [TERRAIN.FROSTED_GROUND]: Object.freeze({ base: 0x646b5e, alt: 0x70766a, details: [0x92978d, 0x4f5749], pattern: "frost" }),
});

// Main-map fog stays lighter than the compact minimap wash so roads, base clearings,
// and terrain remain legible while the viewport still distinguishes explored and unseen ground.
export const MAIN_MAP_FOG_EXPLORED_ALPHA = 0.30;
export const MAIN_MAP_FOG_UNEXPLORED_ALPHA = 0.60;

// Minimap tuning remains darker so the compact map preserves its at-a-glance fog boundary.
export const MINIMAP_FOG_EXPLORED_ALPHA = 0.48;
export const MINIMAP_FOG_UNEXPLORED_ALPHA = 0.8;

export const CAMERA = Object.freeze({
  minZoom: 0.4,
  maxZoom: 2.0,
  labMaxZoom: 8.0,
  maxVisibleTilesPerAxis: 100,
  panSpeed: 900, // world px / sec at zoom 1
  edgeScrollPx: 14, // screen-edge band that triggers panning
});

export const WORKER_BUILD_CARD_SLOTS = Object.freeze([
  KIND.RESOURCE_DEPOT,
  KIND.BARRACKS,
  KIND.TRAINING_CENTRE,
  KIND.ENGINEERING_COMPLEX,
  KIND.FACTORY,
  KIND.STEELWORKS,
  KIND.TANK_TRAP,
]);
