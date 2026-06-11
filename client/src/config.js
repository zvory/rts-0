// Render / UI constants — mirror of the subset of `server/src/config.rs` the client needs.
// Gameplay is authoritative on the server; these values drive UI labels, the command
// card, fog sight radii, and rendering. Keep costs/supply/sight in sync with the server.

import { ABILITY, KIND, UPGRADE } from "./protocol.js";

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
export const FOG_EXPLORED_ALPHA = 0.48;
export const FOG_UNEXPLORED_ALPHA = 0.8;

// Mirrors server/src/config.rs *_BODY_* values. Server collision is authoritative;
// the client uses these only for art, selection, and advisory placement previews.
export const TANK_BODY = Object.freeze({
  length: 50.4,
  width: 28.8,
  clearance: 1.5,
});
export const AT_GUN_BODY = Object.freeze({
  length: 42.0,
  width: 24.0,
  clearance: 1.0,
});
export const ARTILLERY_BODY = Object.freeze({
  length: TANK_BODY.length,
  width: TANK_BODY.width,
  clearance: TANK_BODY.clearance,
});
export const SCOUT_CAR_BODY = Object.freeze({
  length: 40.8,
  width: 21.6,
  clearance: 1.0,
});

// Workers can mine a resource only when a completed City Centre is within this many tiles.
export const MINING_CC_RANGE_TILES = 7.0;
export const AT_GUN_DEPLOYED_RANGE_TILES = 12;
export const AT_GUN_FIELD_OF_FIRE_RAD = Math.PI / 4;
export const ARTILLERY_MIN_RANGE_TILES = 10;
export const ARTILLERY_MAX_RANGE_TILES = 50;
export const ARTILLERY_FIELD_OF_FIRE_RAD = 20 * Math.PI / 180;
export const ARTILLERY_SETUP_TICKS = TICK_HZ * 3;
export const ARTILLERY_SHELL_DELAY_TICKS = TICK_HZ * 5;
export const ARTILLERY_OUTER_RADIUS_TILES = 3;
export const ARTILLERY_AMMO_COST = Object.freeze({ steel: 10, oil: 0 });
export const RIFLEMAN_CHARGE_COOLDOWN_TICKS = 150;
export const SMOKE_ABILITY_RANGE_TILES = 9;
export const SMOKE_LAUNCH_MAX_DELAY_MS = 100;
export const SMOKE_CLOUD_RADIUS_TILES = 2;
export const SMOKE_CLOUD_DURATION_TICKS = TICK_HZ * 5;
export const SMOKE_ABILITY_COOLDOWN_TICKS = 600;
export const SCOUT_CAR_SMOKE_USES = 2;
export const SMOKE_ABILITY_COST = Object.freeze({ steel: 0, oil: 0 });
export const MORTAR_SHELL_DELAY_TICKS = Math.round(TICK_HZ * 2.25);
export const MORTAR_OUTER_RADIUS_TILES = 1.5;
export const MORTAR_INNER_RADIUS_TILES = 0.5;
export const MORTAR_FIRE_COOLDOWN_TICKS = TICK_HZ * 2;
export const METHAMPHETAMINES_RESEARCH_TICKS = TICK_HZ * 20;
export const AT_GUN_UNLOCK_RESEARCH_TICKS = TICK_HZ * 20;
export const ARTILLERY_UNLOCK_RESEARCH_TICKS = TICK_HZ * 30;
export const TANK_UNLOCK_RESEARCH_TICKS = TICK_HZ * 20;

// Player colors (server assigns from a matching palette; used as a fallback for blips).
export const PLAYER_PALETTE = Object.freeze([
  "#cc1111", "#1133bb", "#33aaee", "#dd1188",
  "#117733", "#eeeeee", "#222222", "#8822cc",
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
    rangeTiles: 6, cost: { steel: 75, oil: 10 }, supply: 2, buildTicks: 400, requires: KIND.TRAINING_CENTRE },
  [KIND.AT_TEAM]: { label: "AT Gun", icon: "AT", size: 20, sight: 6, body: AT_GUN_BODY,
    rangeTiles: AT_GUN_DEPLOYED_RANGE_TILES, cost: { steel: 75, oil: 25 }, supply: 3, buildTicks: 440,
    requires: KIND.STEELWORKS, upgradeRequires: UPGRADE.AT_GUN_UNLOCK,
    upgradeRequiresText: "Requires research in R&D Complex" },
  [KIND.MORTAR_TEAM]: { label: "Mortar Team", icon: "MT", size: 18, sight: 7,
    rangeTiles: 9, cost: { steel: 100, oil: 50 }, supply: 3, buildTicks: 460,
    requires: KIND.STEELWORKS },
  [KIND.ARTILLERY]: { label: "Artillery", icon: "AR", size: 18, sight: 4, body: ARTILLERY_BODY,
    rangeTiles: ARTILLERY_MAX_RANGE_TILES, minRangeTiles: ARTILLERY_MIN_RANGE_TILES,
    cost: { steel: 300, oil: 100 }, supply: 5, buildTicks: 750,
    requires: KIND.STEELWORKS, upgradeRequires: UPGRADE.ARTILLERY_UNLOCK,
    upgradeRequiresText: "Requires research in R&D Complex" },
  [KIND.SCOUT_CAR]: { label: "Scout Car", icon: "SC", size: 14.4, sight: 10, body: SCOUT_CAR_BODY,
    rangeTiles: 5, cost: { steel: 125, oil: 50 }, supply: 3, buildTicks: 480 },
  [KIND.TANK]: { label: "Tank", icon: "TK", size: 18, sight: 6, body: TANK_BODY,
    rangeTiles: 5, cost: { steel: 300, oil: 150 }, supply: 6, buildTicks: 750,
    requires: KIND.FACTORY, upgradeRequires: UPGRADE.TANK_UNLOCK,
    upgradeRequiresText: "Requires research in R&D Complex" },

  [KIND.CITY_CENTRE]: { label: "City Centre", icon: "CC", footW: 3, footH: 3, sight: 9,
    cost: { steel: 200, oil: 0 }, buildTicks: 400, trains: [KIND.WORKER] },
  [KIND.DEPOT]: { label: "Supply Depot", icon: "SD", footW: 2, footH: 2, sight: 4,
    cost: { steel: 100, oil: 0 }, buildTicks: 300, trains: [] },
  [KIND.BARRACKS]: { label: "Barracks", icon: "BK", footW: 3, footH: 2, sight: 6,
    cost: { steel: 150, oil: 0 }, buildTicks: 200, trains: [KIND.RIFLEMAN, KIND.MACHINE_GUNNER], requires: KIND.CITY_CENTRE },
  [KIND.TRAINING_CENTRE]: { label: "Training Centre", icon: "TC", footW: 3, footH: 2, sight: 6,
    cost: { steel: 100, oil: 50 }, buildTicks: 560, trains: [],
    researches: [UPGRADE.METHAMPHETAMINES],
    requires: [KIND.CITY_CENTRE, KIND.BARRACKS] },
  [KIND.RESEARCH_COMPLEX]: { label: "R&D Complex", icon: "RD", footW: 3, footH: 3, sight: 6,
    cost: { steel: 100, oil: 100 }, buildTicks: TICK_HZ * 15, trains: [],
    researches: [UPGRADE.AT_GUN_UNLOCK, UPGRADE.ARTILLERY_UNLOCK, UPGRADE.TANK_UNLOCK],
    requires: [KIND.CITY_CENTRE, KIND.TRAINING_CENTRE] },
  [KIND.FACTORY]: { label: "Vehicle Works", icon: "VW", footW: 3, footH: 3, sight: 6,
    cost: { steel: 125, oil: 125 }, buildTicks: 620, trains: [KIND.SCOUT_CAR, KIND.TANK],
    requires: [KIND.CITY_CENTRE, KIND.TRAINING_CENTRE] },
  [KIND.STEELWORKS]: { label: "Gun Works", icon: "GW", footW: 3, footH: 3, sight: 6,
    cost: { steel: 125, oil: 125 }, buildTicks: 620,
    trains: [KIND.MORTAR_TEAM, KIND.AT_TEAM, KIND.ARTILLERY],
    requires: [KIND.CITY_CENTRE, KIND.TRAINING_CENTRE] },

  [KIND.STEEL]: { label: "Steel", size: 22 },
  [KIND.OIL]: { label: "Oil", size: 14 },
});

export const ABILITIES = Object.freeze({
  [ABILITY.SMOKE]: Object.freeze({
    ability: ABILITY.SMOKE,
    label: "Smoke",
    icon: "SMK",
    hotkey: "D",
    title: "Target a smoke grenade location",
    carriers: Object.freeze([KIND.SCOUT_CAR]),
    targetMode: "worldPoint",
    rangeTiles: SMOKE_ABILITY_RANGE_TILES,
    cooldownTicks: SMOKE_ABILITY_COOLDOWN_TICKS,
    cost: SMOKE_ABILITY_COST,
    radiusTiles: SMOKE_CLOUD_RADIUS_TILES,
    durationTicks: SMOKE_CLOUD_DURATION_TICKS,
    queued: false,
  }),
  [ABILITY.MORTAR_FIRE]: Object.freeze({
    ability: ABILITY.MORTAR_FIRE,
    label: "Fire",
    icon: "FIR",
    hotkey: "X",
    title: "Target mortar fire",
    carriers: Object.freeze([KIND.MORTAR_TEAM]),
    targetMode: "worldPoint",
    rangeTiles: 9,
    cooldownTicks: MORTAR_FIRE_COOLDOWN_TICKS,
    cost: Object.freeze({ steel: 0, oil: 0 }),
    radiusTiles: MORTAR_OUTER_RADIUS_TILES,
    queued: false,
  }),
  [ABILITY.POINT_FIRE]: Object.freeze({
    ability: ABILITY.POINT_FIRE,
    label: "Point Fire",
    icon: "PF",
    hotkey: "X",
    title: "Target artillery fire",
    carriers: Object.freeze([KIND.ARTILLERY]),
    targetMode: "worldPoint",
    rangeTiles: ARTILLERY_MAX_RANGE_TILES,
    minRangeTiles: ARTILLERY_MIN_RANGE_TILES,
    cooldownTicks: TICK_HZ * 3,
    cost: ARTILLERY_AMMO_COST,
    radiusTiles: ARTILLERY_OUTER_RADIUS_TILES,
    delayTicks: ARTILLERY_SHELL_DELAY_TICKS,
    queued: true,
  }),
});

export const UPGRADES = Object.freeze({
  [UPGRADE.METHAMPHETAMINES]: Object.freeze({
    upgrade: UPGRADE.METHAMPHETAMINES,
    label: "Methamphetamines",
    icon: "METH",
    cost: Object.freeze({ steel: 100, oil: 100 }),
    researchTicks: METHAMPHETAMINES_RESEARCH_TICKS,
    description: "Increase Rifleman Attack and Speed, Shoot While Moving",
    researchedAt: KIND.TRAINING_CENTRE,
  }),
  [UPGRADE.AT_GUN_UNLOCK]: Object.freeze({
    upgrade: UPGRADE.AT_GUN_UNLOCK,
    label: "AT Gun Crews",
    icon: "AT+",
    cost: Object.freeze({ steel: 200, oil: 75 }),
    researchTicks: AT_GUN_UNLOCK_RESEARCH_TICKS,
    description: "Unlock AT Gun training",
    researchedAt: KIND.RESEARCH_COMPLEX,
  }),
  [UPGRADE.ARTILLERY_UNLOCK]: Object.freeze({
    upgrade: UPGRADE.ARTILLERY_UNLOCK,
    label: "Unlock Artillery",
    icon: "AR+",
    cost: Object.freeze({ steel: 300, oil: 200 }),
    researchTicks: ARTILLERY_UNLOCK_RESEARCH_TICKS,
    description: "Unlocks production of Artillery",
    researchedAt: KIND.RESEARCH_COMPLEX,
    requiresUpgrade: UPGRADE.AT_GUN_UNLOCK,
    requiresText: "Requires AT Gun Research",
  }),
  [UPGRADE.TANK_UNLOCK]: Object.freeze({
    upgrade: UPGRADE.TANK_UNLOCK,
    label: "Tank Production",
    icon: "TK+",
    cost: Object.freeze({ steel: 150, oil: 100 }),
    researchTicks: TANK_UNLOCK_RESEARCH_TICKS,
    description: "Unlock Tank training",
    researchedAt: KIND.RESEARCH_COMPLEX,
  }),
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
  KIND.RESEARCH_COMPLEX,
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
