// Render / UI constants — mirror of the subset of `server/src/config.rs` the client needs.
// Gameplay is authoritative on the server; these values drive UI labels, the command
// card, fog sight radii, and rendering. Keep costs/supply/sight in sync with the server.

import { ABILITY, DEFAULT_FACTION_ID, KIND, UPGRADE } from "./protocol.js";

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
  selectAlly: 0x7ab8d0,
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
export const ANTI_TANK_GUN_BODY = Object.freeze({
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
export const COMMAND_CAR_BODY = Object.freeze({
  length: 34.8,
  width: 18.4,
  clearance: 1.0,
});

// Workers can mine a resource only when a completed City Centre is within this many tiles.
export const MINING_CC_RANGE_TILES = 9.0;
export const ANTI_TANK_GUN_DEPLOYED_RANGE_TILES = 12;
export const ANTI_TANK_GUN_FIELD_OF_FIRE_RAD = Math.PI / 4;
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
export const ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS = TICK_HZ * 20;
export const ARTILLERY_UNLOCK_RESEARCH_TICKS = TICK_HZ * 30;
export const TANK_UNLOCK_RESEARCH_TICKS = TICK_HZ * 20;
export const COMMAND_CAR_UNLOCK_RESEARCH_TICKS = TICK_HZ * 30;
export const MORTAR_AUTOCAST_RESEARCH_TICKS = TICK_HZ * 20;
export const BREAKTHROUGH_RADIUS_TILES = 7;
export const BREAKTHROUGH_DURATION_TICKS = TICK_HZ * 6;
export const BREAKTHROUGH_COOLDOWN_TICKS = TICK_HZ * 25;
export const EKAT_REGEN_TICKS = TICK_HZ;
export const EKAT_TELEPORT_RANGE_TILES = 5;
export const EKAT_TELEPORT_COOLDOWN_TICKS = TICK_HZ * 8;
export const EKAT_LINE_SHOT_RANGE_TILES = 6;
export const EKAT_LINE_SHOT_WIDTH_TILES = 0.6;
export const EKAT_LINE_SHOT_SPEED_PX_PER_TICK = 8;
export const EKAT_LINE_SHOT_DAMAGE = 40;
export const EKAT_LINE_SHOT_COOLDOWN_TICKS = TICK_HZ * 10;
export const EKAT_MAGIC_ANCHOR_RANGE_TILES = 5;
export const EKAT_MAGIC_ANCHOR_DURATION_TICKS = TICK_HZ * 10;
export const EKAT_MAGIC_ANCHOR_LOCKOUT_TICKS = TICK_HZ * 60;
export const EKAT_MAGIC_ANCHOR_HP = 100;
export const EKAT_MAGIC_ANCHOR_RADIUS_TILES = 0.4;
export const BASE_COMMAND_SUPPLY_CAP = 24;
export const COMMAND_CAR_SUPPLY_CAP_BONUS = 12;

// Player colors (server assigns from a matching palette; used as a fallback for blips).
export const PLAYER_PALETTE = Object.freeze([
  "#0072b2", "#d55e00", "#009e73", "#cc79a7",
  "#56b4e9", "#e69f00", "#f0e442", "#7e57c2",
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
  [KIND.ANTI_TANK_GUN]: { label: "Anti-Tank Gun", icon: "ATG", size: 20, sight: 6, body: ANTI_TANK_GUN_BODY,
    rangeTiles: ANTI_TANK_GUN_DEPLOYED_RANGE_TILES, cost: { steel: 75, oil: 25 }, supply: 3, buildTicks: 440,
    requires: KIND.STEELWORKS, upgradeRequires: UPGRADE.ANTI_TANK_GUN_UNLOCK,
    upgradeRequiresText: "Requires research in R&D Complex" },
  [KIND.MORTAR_TEAM]: { label: "Mortar Team", icon: "MT", size: 18, sight: 7,
    rangeTiles: 9, cost: { steel: 100, oil: 50 }, supply: 2, buildTicks: 460,
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
  [KIND.COMMAND_CAR]: { label: "Command Car", icon: "CAR", size: 12.6, sight: 10, body: COMMAND_CAR_BODY,
    rangeTiles: 0, cost: { steel: 150, oil: 75 }, supply: 4, buildTicks: TICK_HZ * 15,
    requires: KIND.FACTORY, upgradeRequires: UPGRADE.COMMAND_CAR_UNLOCK,
    upgradeRequiresText: "Requires research in R&D Complex" },
  [KIND.EKAT]: { label: "Ekat", icon: "EK", size: 10, sight: 9,
    rangeTiles: 4, cost: { steel: 0, oil: 0 }, supply: 0, buildTicks: 0 },

  [KIND.CITY_CENTRE]: { label: "City Centre", icon: "CC", footW: 3, footH: 3, sight: 9,
    cost: { steel: 200, oil: 0 }, buildTicks: 400, trains: [KIND.WORKER] },
  [KIND.ZAMOK]: { label: "Zamok", icon: "ZK", footW: 3, footH: 3, sight: 9,
    cost: { steel: 0, oil: 0 }, buildTicks: 0, trains: [] },
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
    researches: [
      UPGRADE.ANTI_TANK_GUN_UNLOCK,
      UPGRADE.ARTILLERY_UNLOCK,
      UPGRADE.TANK_UNLOCK,
      UPGRADE.MORTAR_AUTOCAST,
      UPGRADE.COMMAND_CAR_UNLOCK,
    ],
    requires: [KIND.CITY_CENTRE, KIND.TRAINING_CENTRE] },
  [KIND.FACTORY]: { label: "Vehicle Works", icon: "VW", footW: 3, footH: 3, sight: 6,
    cost: { steel: 125, oil: 125 }, buildTicks: 620,
    trains: [KIND.SCOUT_CAR, KIND.TANK, KIND.COMMAND_CAR],
    requires: [KIND.CITY_CENTRE, KIND.TRAINING_CENTRE] },
  [KIND.STEELWORKS]: { label: "Gun Works", icon: "GW", footW: 3, footH: 3, sight: 6,
    cost: { steel: 125, oil: 125 }, buildTicks: 620,
    trains: [KIND.MORTAR_TEAM, KIND.ANTI_TANK_GUN, KIND.ARTILLERY],
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
    queued: true,
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
    autocast: true,
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
  [ABILITY.BREAKTHROUGH]: Object.freeze({
    ability: ABILITY.BREAKTHROUGH,
    label: "Breakthrough!",
    icon: "BRK",
    hotkey: "E",
    title: "Speed up nearby owned units; stronger in smoke",
    carriers: Object.freeze([KIND.COMMAND_CAR]),
    targetMode: "self",
    rangeTiles: null,
    cooldownTicks: BREAKTHROUGH_COOLDOWN_TICKS,
    cost: Object.freeze({ steel: 0, oil: 0 }),
    radiusTiles: BREAKTHROUGH_RADIUS_TILES,
    durationTicks: BREAKTHROUGH_DURATION_TICKS,
    queued: true,
  }),
  [ABILITY.EKAT_TELEPORT]: Object.freeze({
    ability: ABILITY.EKAT_TELEPORT,
    label: "Dash",
    icon: "DSH",
    hotkey: "D",
    title: "Dash up to 5 tiles, then recast to return",
    carriers: Object.freeze([KIND.EKAT]),
    targetMode: "worldPoint",
    rangeTiles: EKAT_TELEPORT_RANGE_TILES,
    cooldownTicks: EKAT_TELEPORT_COOLDOWN_TICKS,
    cost: Object.freeze({ steel: 0, oil: 0 }),
    queued: false,
  }),
  [ABILITY.EKAT_LINE_SHOT]: Object.freeze({
    ability: ABILITY.EKAT_LINE_SHOT,
    label: "Line Shot",
    icon: "LS",
    hotkey: "X",
    title: "Send a line projectile out and back",
    carriers: Object.freeze([KIND.EKAT]),
    targetMode: "worldPoint",
    rangeTiles: EKAT_LINE_SHOT_RANGE_TILES,
    cooldownTicks: EKAT_LINE_SHOT_COOLDOWN_TICKS,
    cost: Object.freeze({ steel: 0, oil: 0 }),
    radiusTiles: EKAT_LINE_SHOT_WIDTH_TILES * 0.5,
    speedPxPerTick: EKAT_LINE_SHOT_SPEED_PX_PER_TICK,
    damage: EKAT_LINE_SHOT_DAMAGE,
    queued: false,
  }),
  [ABILITY.EKAT_MAGIC_ANCHOR]: Object.freeze({
    ability: ABILITY.EKAT_MAGIC_ANCHOR,
    label: "Magic Anchor",
    icon: "ANC",
    hotkey: "C",
    title: "Place a destructible 10-second anchor",
    carriers: Object.freeze([KIND.EKAT]),
    targetMode: "worldPoint",
    rangeTiles: EKAT_MAGIC_ANCHOR_RANGE_TILES,
    cooldownTicks: 0,
    cost: Object.freeze({ steel: 0, oil: 0 }),
    radiusTiles: EKAT_MAGIC_ANCHOR_RADIUS_TILES,
    durationTicks: EKAT_MAGIC_ANCHOR_DURATION_TICKS,
    lockoutTicks: EKAT_MAGIC_ANCHOR_LOCKOUT_TICKS,
    hp: EKAT_MAGIC_ANCHOR_HP,
    queued: false,
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
  [UPGRADE.ANTI_TANK_GUN_UNLOCK]: Object.freeze({
    upgrade: UPGRADE.ANTI_TANK_GUN_UNLOCK,
    label: "Anti-Tank Gun Crews",
    icon: "ATG+",
    cost: Object.freeze({ steel: 200, oil: 75 }),
    researchTicks: ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS,
    description: "Unlock Anti-Tank Gun training",
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
    requiresUpgrade: UPGRADE.ANTI_TANK_GUN_UNLOCK,
    requiresText: "Requires Anti-Tank Gun Research",
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
  [UPGRADE.COMMAND_CAR_UNLOCK]: Object.freeze({
    upgrade: UPGRADE.COMMAND_CAR_UNLOCK,
    label: "Command Car",
    icon: "CC+",
    cost: Object.freeze({ steel: 150, oil: 150 }),
    researchTicks: COMMAND_CAR_UNLOCK_RESEARCH_TICKS,
    description: "Unlocks production of Command Cars",
    researchedAt: KIND.RESEARCH_COMPLEX,
    requiresUpgrade: UPGRADE.TANK_UNLOCK,
    requiresText: "Requires Tank Production",
  }),
  [UPGRADE.MORTAR_AUTOCAST]: Object.freeze({
    upgrade: UPGRADE.MORTAR_AUTOCAST,
    label: "Mortar Autocast",
    icon: "MT+",
    cost: Object.freeze({ steel: 150, oil: 150 }),
    researchTicks: MORTAR_AUTOCAST_RESEARCH_TICKS,
    description: "Enable Mortar Team autocast by default",
    researchedAt: KIND.RESEARCH_COMPLEX,
  }),
});

// A building that trains units — the only buildings that accept a rally point.
export const isProducerBuilding = (kind) =>
  Array.isArray(STATS[kind]?.trains) && STATS[kind].trains.length > 0;

export const RESOURCE_AMOUNTS = Object.freeze({
  [KIND.STEEL]: 1000,
  [KIND.OIL]: 3333,
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

export const FIXTURE_FACTION_ID = "phase2_empty_fixture";
export const EKAT_FACTION_ID = "ekat";

function freezeCatalog(catalog) {
  const trainables = {};
  for (const [building, units] of Object.entries(catalog.trainables || {})) {
    trainables[building] = Object.freeze([...units]);
  }
  const research = {};
  for (const [building, upgrades] of Object.entries(catalog.research || {})) {
    research[building] = Object.freeze([...upgrades]);
  }
  return Object.freeze({
    ...catalog,
    units: Object.freeze([...(catalog.units || [])]),
    buildings: Object.freeze([...(catalog.buildings || [])]),
    buildables: Object.freeze([...(catalog.buildables || [])]),
    trainables: Object.freeze(trainables),
    research: Object.freeze(research),
    abilities: Object.freeze([...(catalog.abilities || [])]),
  });
}

export const FACTION_CATALOGS = Object.freeze({
  [DEFAULT_FACTION_ID]: freezeCatalog({
    id: DEFAULT_FACTION_ID,
    loadoutId: "kriegsia.standard",
    units: [
      KIND.WORKER,
      KIND.RIFLEMAN,
      KIND.MACHINE_GUNNER,
      KIND.ANTI_TANK_GUN,
      KIND.MORTAR_TEAM,
      KIND.ARTILLERY,
      KIND.TANK,
      KIND.SCOUT_CAR,
      KIND.COMMAND_CAR,
    ],
    buildings: [
      KIND.CITY_CENTRE,
      KIND.DEPOT,
      KIND.BARRACKS,
      KIND.TRAINING_CENTRE,
      KIND.FACTORY,
      KIND.RESEARCH_COMPLEX,
      KIND.STEELWORKS,
    ],
    buildables: WORKER_BUILDABLE,
    trainables: {
      [KIND.CITY_CENTRE]: [KIND.WORKER],
      [KIND.BARRACKS]: [KIND.RIFLEMAN, KIND.MACHINE_GUNNER],
      [KIND.FACTORY]: [KIND.SCOUT_CAR, KIND.TANK, KIND.COMMAND_CAR],
      [KIND.STEELWORKS]: [KIND.MORTAR_TEAM, KIND.ANTI_TANK_GUN, KIND.ARTILLERY],
    },
    research: {
      [KIND.TRAINING_CENTRE]: [UPGRADE.METHAMPHETAMINES],
      [KIND.RESEARCH_COMPLEX]: [
        UPGRADE.ANTI_TANK_GUN_UNLOCK,
        UPGRADE.ARTILLERY_UNLOCK,
        UPGRADE.TANK_UNLOCK,
        UPGRADE.COMMAND_CAR_UNLOCK,
        UPGRADE.MORTAR_AUTOCAST,
      ],
    },
    abilities: [ABILITY.SMOKE, ABILITY.MORTAR_FIRE, ABILITY.POINT_FIRE, ABILITY.BREAKTHROUGH],
  }),
  [FIXTURE_FACTION_ID]: freezeCatalog({
    id: FIXTURE_FACTION_ID,
    loadoutId: "phase2_empty_fixture.scout_depot",
    units: [KIND.SCOUT_CAR],
    buildings: [KIND.DEPOT],
    buildables: [],
    trainables: {},
    research: {},
    abilities: [],
  }),
  [EKAT_FACTION_ID]: freezeCatalog({
    id: EKAT_FACTION_ID,
    loadoutId: "ekat.standard",
    units: [KIND.EKAT],
    buildings: [KIND.ZAMOK],
    buildables: [],
    trainables: {},
    research: {},
    abilities: [ABILITY.EKAT_TELEPORT, ABILITY.EKAT_LINE_SHOT, ABILITY.EKAT_MAGIC_ANCHOR],
  }),
});

const EMPTY_CLIENT_CATALOG = freezeCatalog({
  id: "unknown",
  loadoutId: "",
  units: [],
  buildings: [],
  buildables: [],
  trainables: {},
  research: {},
  abilities: [],
});

export function factionCatalog(factionId = DEFAULT_FACTION_ID) {
  return FACTION_CATALOGS[factionId] || EMPTY_CLIENT_CATALOG;
}

export function workerBuildablesForFaction(factionId) {
  return factionCatalog(factionId).buildables;
}

export function trainableUnitsForFaction(factionId, buildingKind) {
  return factionCatalog(factionId).trainables[buildingKind] || [];
}

export function researchableUpgradesForFaction(factionId, buildingKind) {
  return factionCatalog(factionId).research[buildingKind] || [];
}

export function commandCardAbilitiesForFaction(factionId) {
  return factionCatalog(factionId)
    .abilities
    .map((ability) => ABILITIES[ability])
    .filter(Boolean);
}

// Camera defaults.
export const CAMERA = Object.freeze({
  minZoom: 0.4,
  maxZoom: 2.0,
  panSpeed: 900, // world px / sec at zoom 1
  edgeScrollPx: 14, // screen-edge band that triggers panning
});
