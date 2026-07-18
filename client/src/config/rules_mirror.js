// Client-visible rules mirror data. Gameplay is authoritative on the server;
// these values drive UI labels, command cards, fog sight radii, and rendering
// hints that are validated against Rust-owned dumps.

import { ABILITY, KIND, UPGRADE } from "../protocol.js";
import { TICK_HZ } from "./timing.js";

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
export const SCOUT_PLANE_BODY = Object.freeze({
  length: 48.0,
  width: 34.0,
  clearance: 0.0,
});
export const COMMAND_CAR_BODY = Object.freeze({
  length: 34.8,
  width: 18.4,
  clearance: 1.0,
});

// Gatherers can mine a resource only when a completed home-base mining anchor is within range.
export const MINING_CC_RANGE_TILES = 11.0;
export const ANTI_TANK_GUN_DEPLOYED_RANGE_TILES = 20;
export const ANTI_TANK_GUN_FIELD_OF_FIRE_RAD = 30 * Math.PI / 180;
export const ARTILLERY_MIN_RANGE_TILES = 25;
export const ARTILLERY_MAX_RANGE_TILES = 55;
export const ARTILLERY_FIELD_OF_FIRE_RAD = 20 * Math.PI / 180;
export const ARTILLERY_SETUP_TICKS = TICK_HZ * 6;
export const ARTILLERY_SHELL_DELAY_TICKS = TICK_HZ * 5;
export const ARTILLERY_OUTER_RADIUS_TILES = 3;
export const ARTILLERY_BLANKET_RADIUS_TILES = 15;
export const ARTILLERY_AMMO_COST = Object.freeze({ steel: 10, oil: 0 });
export const SMOKE_ABILITY_RANGE_TILES = 14;
export const SMOKE_LAUNCH_MAX_DELAY_MS = 100;
export const SMOKE_CLOUD_RADIUS_TILES = 2;
export const SMOKE_CLOUD_DURATION_TICKS = TICK_HZ * 5;
export const SMOKE_PLUS_CLOUD_RADIUS_TILES = SMOKE_CLOUD_RADIUS_TILES * 1.5;
export const SMOKE_PLUS_CLOUD_DURATION_TICKS = SMOKE_CLOUD_DURATION_TICKS * 2;
export const SMOKE_ABILITY_COOLDOWN_TICKS = 0;
export const SCOUT_CAR_SMOKE_USES = 2;
export const SMOKE_ABILITY_COST = Object.freeze({ steel: 0, oil: 0 });
export const MORTAR_SHELL_DELAY_TICKS = Math.round(TICK_HZ * 2.25);
export const MORTAR_RANGE_TILES = 15;
export const MORTAR_MIN_RANGE_TILES = 5;
export const MORTAR_FIELD_OF_FIRE_RAD = Math.PI * 2;
export const MORTAR_SETUP_TICKS = TICK_HZ * 1.5;
export const MORTAR_TEARDOWN_TICKS = TICK_HZ * 0.5;
export const MORTAR_OUTER_RADIUS_TILES = 1.5;
export const MORTAR_INNER_RADIUS_TILES = 0.5;
export const MORTAR_FIRE_COOLDOWN_TICKS = TICK_HZ * 2;
export const PANZERFAUST_RANGE_TILES = 5;
export const PANZERFAUST_DAMAGE = 100;
export const PANZERFAUST_ARMOR_PENETRATION = 0.5;
export const PANZERFAUST_WINDUP_TICKS = TICK_HZ / 2;
export const PANZERFAUST_TRAVEL_TICKS = TICK_HZ / 2;
export const METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS = 12;
export const METHAMPHETAMINES_RESEARCH_TICKS = TICK_HZ * 20;
export const PANZERFAUSTS_RESEARCH_TICKS = TICK_HZ * 20;
export const ENTRENCHMENT_RESEARCH_TICKS = TICK_HZ * 30;
export const ENTRENCHMENT_DIG_IN_TICKS = TICK_HZ * 3;
export const ENTRENCHMENT_RANGE_BONUS_TILES = 1;
export const ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION = 0.50;
export const ENTRENCHMENT_AREA_DAMAGE_REDUCTION = 0.25;
export const ENTRENCHMENT_TRENCH_RADIUS_TILES = 0.375;
export const ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS = TICK_HZ * 10;
export const ARTILLERY_UNLOCK_RESEARCH_TICKS = TICK_HZ * 25;
export const BALLISTIC_TABLES_RESEARCH_TICKS = TICK_HZ * 40;
export const TANK_UNLOCK_RESEARCH_TICKS = TICK_HZ * 20;
export const MORTAR_AUTOCAST_RESEARCH_TICKS = TICK_HZ * 20;
export const SMOKE_PLUS_RESEARCH_TICKS = TICK_HZ * 20;
export const BREAKTHROUGH_RADIUS_TILES = 9;
export const BREAKTHROUGH_DURATION_TICKS = TICK_HZ * 6;
export const BREAKTHROUGH_COOLDOWN_TICKS = TICK_HZ * 25;
export const EKAT_CONSUME_GOLEM_RANGE_TILES = 2;
export const EKAT_TELEPORT_RANGE_TILES = 5;
export const EKAT_TELEPORT_COOLDOWN_TICKS = TICK_HZ * 8;
export const EKAT_LINE_SHOT_RANGE_TILES = 6;
export const EKAT_LINE_SHOT_WIDTH_TILES = 0.6;
export const EKAT_LINE_SHOT_SPEED_PX_PER_TICK = 8;
export const EKAT_LINE_SHOT_DAMAGE = 40;
export const EKAT_LINE_SHOT_COOLDOWN_TICKS = TICK_HZ * 10;
export const EKAT_MAGIC_ANCHOR_RANGE_TILES = 5;
export const EKAT_MAGIC_ANCHOR_DURATION_TICKS = TICK_HZ * 10;
export const EKAT_MAGIC_ANCHOR_RADIUS_TILES = 3.0;
export const EKAT_MAGIC_ANCHOR_PULL_AWAY_MULTIPLIER = 0.45;
export const EKAT_MAGIC_ANCHOR_PULL_TOWARD_MULTIPLIER = 1.35;
export const BASE_COMMAND_SUPPLY_CAP = 24;
export const COMMAND_CAR_SUPPLY_CAP_BONUS = 20;
export const SCOUT_PLANE_ORBIT_RADIUS_TILES = 4;
export const SCOUT_PLANE_SPEED_PX_PER_TICK = 2;
export const SCOUT_PLANE_LIFETIME_TICKS = TICK_HZ * 20;
export const SCOUT_PLANE_ABILITY_COOLDOWN_TICKS = TICK_HZ * 30;

// Per-kind UI / render info. `size` is the render radius (units) or half-extent hint.
// `sight` (tiles) drives the local fog overlay. `rangeTiles` mirrors weapon range for visuals.
// `cost`/`supply` drive the command card.
export const STATS = Object.freeze({
  [KIND.WORKER]: { label: "Engineer", icon: "EN", size: 9, sight: 10,
    rangeTiles: 1, cost: { steel: 50, oil: 0 }, supply: 1, buildTicks: 396 },
  [KIND.GOLEM]: { label: "Golem", icon: "GLM", size: 9, sight: 10,
    rangeTiles: 1, cost: { steel: 0, oil: 0 }, supply: 4, buildTicks: 396 },
  [KIND.RIFLEMAN]: { label: "Rifleman", icon: "RF", size: 9, sight: 11,
    rangeTiles: 4, cost: { steel: 60, oil: 10 }, supply: 1, buildTicks: 300 },
  [KIND.MACHINE_GUNNER]: { label: "Machine Gunner", icon: "MG", size: 10, sight: 11,
    rangeTiles: 6, cost: { steel: 75, oil: 10 }, supply: 2, buildTicks: 400, requires: KIND.TRAINING_CENTRE },
  [KIND.ANTI_TANK_GUN]: { label: "Anti-Tank Gun", icon: "ATG", size: 20, sight: 9, body: ANTI_TANK_GUN_BODY,
    rangeTiles: ANTI_TANK_GUN_DEPLOYED_RANGE_TILES, cost: { steel: 75, oil: 25 }, supply: 3, buildTicks: 440,
    requires: KIND.STEELWORKS, upgradeRequires: UPGRADE.ANTI_TANK_GUN_UNLOCK,
    upgradeRequiresText: "Requires research in R&D Complex" },
  [KIND.MORTAR_TEAM]: { label: "Mortar Team", icon: "MT", size: 18, sight: 10,
    rangeTiles: MORTAR_RANGE_TILES, minRangeTiles: MORTAR_MIN_RANGE_TILES,
    cost: { steel: 100, oil: 50 }, supply: 3, buildTicks: 460,
    requires: KIND.STEELWORKS,
    description: "Indirect fire, extremely inaccurate without vision. Upgrade auto cast in R&D." },
  [KIND.ARTILLERY]: { label: "Artillery", icon: "AR", size: 18, sight: 7, body: ARTILLERY_BODY,
    rangeTiles: ARTILLERY_MAX_RANGE_TILES, minRangeTiles: ARTILLERY_MIN_RANGE_TILES,
    cost: { steel: 300, oil: 100 }, supply: 5, buildTicks: 750,
    requires: KIND.STEELWORKS, upgradeRequires: UPGRADE.ARTILLERY_UNLOCK,
    upgradeRequiresText: "Requires research in R&D Complex" },
  [KIND.SCOUT_CAR]: { label: "Scout Car", icon: "SC", size: 14.4, sight: 15, body: SCOUT_CAR_BODY,
    rangeTiles: 7, cost: { steel: 125, oil: 50 }, supply: 3, buildTicks: 480 },
  [KIND.SCOUT_PLANE]: { label: "Scout Plane", icon: "SP", size: 17, sight: 15, body: SCOUT_PLANE_BODY,
    blocksGroundPlacement: false,
    rangeTiles: 0, cost: { steel: 50, oil: 75 }, supply: 0, buildTicks: 0 },
  [KIND.TANK]: { label: "Tank", icon: "TK", size: 18, sight: 9, body: TANK_BODY,
    rangeTiles: 5, cost: { steel: 425, oil: 150 }, supply: 8, buildTicks: 750,
    requires: KIND.FACTORY, upgradeRequires: UPGRADE.TANK_UNLOCK,
    upgradeRequiresText: "Requires research in R&D Complex" },
  [KIND.COMMAND_CAR]: { label: "Command Car", icon: "CAR", size: 12.6, sight: 8, body: COMMAND_CAR_BODY,
    rangeTiles: 0, cost: { steel: 150, oil: 75 }, supply: 4, buildTicks: TICK_HZ * 15,
    requires: KIND.FACTORY, upgradeRequires: UPGRADE.TANK_UNLOCK,
    upgradeRequiresText: "Requires Tank Production" },
  [KIND.EKAT]: { label: "Ekat", icon: "EK", size: 10, sight: 12,
    rangeTiles: 0, cost: { steel: 0, oil: 0 }, supply: 0, buildTicks: 0 },

  [KIND.CITY_CENTRE]: { label: "City Centre", icon: "CC", footW: 3, footH: 3, sight: 1,
    cost: { steel: 450, oil: 150 }, buildTicks: 750, trains: [KIND.WORKER] },
  [KIND.ZAMOK]: { label: "Zamok", icon: "ZK", footW: 3, footH: 3, sight: 1,
    cost: { steel: 0, oil: 0 }, buildTicks: 0, trains: [KIND.GOLEM] },
  [KIND.DEPOT]: { label: "Supply Depot", icon: "SD", footW: 2, footH: 2, sight: 1,
    cost: { steel: 100, oil: 0 }, buildTicks: 300, trains: [] },
  [KIND.BARRACKS]: { label: "Barracks", icon: "BK", footW: 3, footH: 2, sight: 1,
    cost: { steel: 150, oil: 0 }, buildTicks: 200, trains: [KIND.RIFLEMAN, KIND.MACHINE_GUNNER], requires: KIND.CITY_CENTRE },
  [KIND.TRAINING_CENTRE]: { label: "Training Centre", icon: "TC", footW: 3, footH: 2, sight: 1,
    cost: { steel: 100, oil: 50 }, buildTicks: 560, trains: [],
    researches: [UPGRADE.METHAMPHETAMINES, UPGRADE.PANZERFAUSTS, UPGRADE.ENTRENCHMENT],
    requires: [KIND.CITY_CENTRE, KIND.BARRACKS] },
  [KIND.RESEARCH_COMPLEX]: { label: "R&D Complex", icon: "RD", footW: 3, footH: 3, sight: 1,
    cost: { steel: 100, oil: 100 }, buildTicks: TICK_HZ * 15, trains: [],
    researches: [
      UPGRADE.ANTI_TANK_GUN_UNLOCK,
      UPGRADE.BALLISTIC_TABLES,
      UPGRADE.TANK_UNLOCK,
      UPGRADE.MORTAR_AUTOCAST,
      UPGRADE.SMOKE_PLUS,
      UPGRADE.ARTILLERY_UNLOCK,
    ],
    requires: [KIND.CITY_CENTRE, KIND.TRAINING_CENTRE] },
  [KIND.FACTORY]: { label: "Vehicle Works", icon: "VW", footW: 3, footH: 3, sight: 1,
    cost: { steel: 125, oil: 125 }, buildTicks: 749,
    trains: [KIND.SCOUT_CAR, KIND.TANK, KIND.COMMAND_CAR],
    requires: [KIND.CITY_CENTRE, KIND.TRAINING_CENTRE] },
  [KIND.STEELWORKS]: { label: "Gun Works", icon: "GW", footW: 3, footH: 3, sight: 1,
    cost: { steel: 150, oil: 100 }, buildTicks: 599,
    trains: [KIND.MORTAR_TEAM, KIND.ANTI_TANK_GUN, KIND.ARTILLERY],
    requires: [KIND.CITY_CENTRE, KIND.TRAINING_CENTRE] },
  [KIND.TANK_TRAP]: { label: "Tank Trap", icon: "TT", footW: 1, footH: 1, sight: 0,
    cost: { steel: 30, oil: 0 }, buildTicks: TICK_HZ * 10, trains: [],
    requires: KIND.TRAINING_CENTRE,
    requiresText: "Requires Training Centre" },
  [KIND.PUMP_JACK]: { label: "Pump Jack", icon: "PJ", footW: 1, footH: 1, sight: 1,
    cost: { steel: 50, oil: 0 }, buildTicks: TICK_HZ * 20, trains: [],
    description: "Build on an oil patch. Extracts 2 Oil every 1.3s while within 11 tiles of an allied completed City Centre or Zamok." },

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
    charges: SCOUT_CAR_SMOKE_USES,
    cost: SMOKE_ABILITY_COST,
    techRequirement: KIND.RESEARCH_COMPLEX,
    radiusTiles: SMOKE_CLOUD_RADIUS_TILES,
    durationTicks: SMOKE_CLOUD_DURATION_TICKS,
    upgradedRadiusTiles: SMOKE_PLUS_CLOUD_RADIUS_TILES,
    upgradedDurationTicks: SMOKE_PLUS_CLOUD_DURATION_TICKS,
    queued: true,
    queuePolicy: "skipIfNotReady",
  }),
  [ABILITY.MORTAR_FIRE]: Object.freeze({
    ability: ABILITY.MORTAR_FIRE,
    label: "Fire",
    icon: "FIR",
    hotkey: "X",
    title: "Target mortar fire",
    carriers: Object.freeze([KIND.MORTAR_TEAM]),
    targetMode: "worldPoint",
    rangeTiles: MORTAR_RANGE_TILES,
    minRangeTiles: MORTAR_MIN_RANGE_TILES,
    cooldownTicks: MORTAR_FIRE_COOLDOWN_TICKS,
    cost: Object.freeze({ steel: 0, oil: 0 }),
    radiusTiles: MORTAR_OUTER_RADIUS_TILES,
    queued: true,
    queuePolicy: "waitUntilReady",
    autocast: true,
  }),
  [ABILITY.POINT_FIRE]: Object.freeze({
    ability: ABILITY.POINT_FIRE,
    label: "Point Fire",
    icon: "PF",
    hotkey: "X",
    commandCardPriority: -1,
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
    queuePolicy: "skipIfNotReady",
  }),
  [ABILITY.BLANKET_FIRE]: Object.freeze({
    ability: ABILITY.BLANKET_FIRE,
    label: "Blanket Fire",
    icon: "BF",
    hotkey: "C",
    commandCardPriority: -1,
    title: "Target blanket artillery fire",
    carriers: Object.freeze([KIND.ARTILLERY]),
    targetMode: "worldPoint",
    rangeTiles: ARTILLERY_MAX_RANGE_TILES,
    minRangeTiles: ARTILLERY_MIN_RANGE_TILES,
    cooldownTicks: TICK_HZ * 3,
    cost: ARTILLERY_AMMO_COST,
    radiusTiles: ARTILLERY_BLANKET_RADIUS_TILES,
    queued: true,
    queuePolicy: "skipIfNotReady",
  }),
  [ABILITY.BREAKTHROUGH]: Object.freeze({
    ability: ABILITY.BREAKTHROUGH,
    label: "Breakthrough!",
    icon: "BRK",
    hotkey: "E",
    title: "Nearby owned units are always faster; activate full speed (stronger in smoke)",
    carriers: Object.freeze([KIND.COMMAND_CAR]),
    targetMode: "self",
    rangeTiles: null,
    cooldownTicks: BREAKTHROUGH_COOLDOWN_TICKS,
    cost: Object.freeze({ steel: 0, oil: 0 }),
    radiusTiles: BREAKTHROUGH_RADIUS_TILES,
    durationTicks: BREAKTHROUGH_DURATION_TICKS,
    queued: true,
    queuePolicy: "skipIfNotReady",
  }),
  [ABILITY.SCOUT_PLANE]: Object.freeze({
    ability: ABILITY.SCOUT_PLANE,
    label: "Scout Plane",
    icon: "SP",
    hotkey: "C",
    title: "Launch this Command Car's scout plane",
    carriers: Object.freeze([KIND.COMMAND_CAR]),
    targetMode: "worldPoint",
    rangeTiles: null,
    cooldownTicks: SCOUT_PLANE_ABILITY_COOLDOWN_TICKS,
    cost: Object.freeze({ steel: 50, oil: 75 }),
    radiusTiles: SCOUT_PLANE_ORBIT_RADIUS_TILES,
    durationTicks: SCOUT_PLANE_LIFETIME_TICKS,
    queued: true,
    queuePolicy: "skipIfNotReady",
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
    queued: true,
    queuePolicy: "skipIfNotReady",
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
    queued: true,
    queuePolicy: "skipIfNotReady",
  }),
  [ABILITY.EKAT_MAGIC_ANCHOR]: Object.freeze({
    ability: ABILITY.EKAT_MAGIC_ANCHOR,
    label: "Magic Anchor",
    icon: "ANC",
    hotkey: "C",
    title: "Place a 10-second pull field",
    carriers: Object.freeze([KIND.EKAT]),
    targetMode: "worldPoint",
    rangeTiles: EKAT_MAGIC_ANCHOR_RANGE_TILES,
    cooldownTicks: 0,
    cost: Object.freeze({ steel: 0, oil: 0 }),
    radiusTiles: EKAT_MAGIC_ANCHOR_RADIUS_TILES,
    durationTicks: EKAT_MAGIC_ANCHOR_DURATION_TICKS,
    pullAwayMultiplier: EKAT_MAGIC_ANCHOR_PULL_AWAY_MULTIPLIER,
    pullTowardMultiplier: EKAT_MAGIC_ANCHOR_PULL_TOWARD_MULTIPLIER,
    queued: true,
    queuePolicy: "skipIfNotReady",
  }),
  [ABILITY.EKAT_CONSUME_GOLEM]: Object.freeze({
    ability: ABILITY.EKAT_CONSUME_GOLEM,
    label: "Consume",
    icon: "CON",
    hotkey: "Z",
    title: "Consume a nearby Golem to heal Ekat to full HP",
    carriers: Object.freeze([KIND.EKAT]),
    targetMode: "self",
    rangeTiles: EKAT_CONSUME_GOLEM_RANGE_TILES,
    cooldownTicks: 0,
    cost: Object.freeze({ steel: 0, oil: 0 }),
    radiusTiles: EKAT_CONSUME_GOLEM_RANGE_TILES,
    queued: false,
    queuePolicy: "notQueueable",
  }),
});

export const UPGRADES = Object.freeze({
  [UPGRADE.METHAMPHETAMINES]: Object.freeze({
    upgrade: UPGRADE.METHAMPHETAMINES,
    label: "Methamphetamines",
    icon: "METH",
    cost: Object.freeze({ steel: 100, oil: 100 }),
    researchTicks: METHAMPHETAMINES_RESEARCH_TICKS,
    description: "Boost Riflemen; speed up Machine Gunner movement and setup",
    researchedAt: KIND.TRAINING_CENTRE,
  }),
  [UPGRADE.PANZERFAUSTS]: Object.freeze({
    upgrade: UPGRADE.PANZERFAUSTS,
    label: "Panzerfausts",
    icon: "PF+",
    cost: Object.freeze({ steel: 100, oil: 100 }),
    researchTicks: PANZERFAUSTS_RESEARCH_TICKS,
    description: `Give newly produced Riflemen one disposable ${PANZERFAUST_RANGE_TILES}-tile anti-vehicle shot`,
    researchedAt: KIND.TRAINING_CENTRE,
  }),
  [UPGRADE.ENTRENCHMENT]: Object.freeze({
    upgrade: UPGRADE.ENTRENCHMENT,
    label: "Entrenchment",
    icon: "ENT",
    cost: Object.freeze({ steel: 200, oil: 0 }),
    researchTicks: ENTRENCHMENT_RESEARCH_TICKS,
    description: "Let eligible infantry create and use persistent trenches",
    researchedAt: KIND.TRAINING_CENTRE,
  }),
  [UPGRADE.ANTI_TANK_GUN_UNLOCK]: Object.freeze({
    upgrade: UPGRADE.ANTI_TANK_GUN_UNLOCK,
    label: "Medium Guns",
    icon: "MD+",
    cost: Object.freeze({ steel: 100, oil: 50 }),
    researchTicks: ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS,
    description: "Unlock Anti-Tank Gun training",
    researchedAt: KIND.RESEARCH_COMPLEX,
  }),
  [UPGRADE.ARTILLERY_UNLOCK]: Object.freeze({
    upgrade: UPGRADE.ARTILLERY_UNLOCK,
    label: "Heavy Guns",
    icon: "HG+",
    cost: Object.freeze({ steel: 200, oil: 100 }),
    researchTicks: ARTILLERY_UNLOCK_RESEARCH_TICKS,
    description: "Unlock Artillery training",
    researchedAt: KIND.RESEARCH_COMPLEX,
    requiresUpgrade: UPGRADE.ANTI_TANK_GUN_UNLOCK,
    requiresText: "Requires Medium Guns",
    replacesUpgrade: UPGRADE.ANTI_TANK_GUN_UNLOCK,
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
  [UPGRADE.BALLISTIC_TABLES]: Object.freeze({
    upgrade: UPGRADE.BALLISTIC_TABLES,
    label: "Artillery Fire Control",
    icon: "AFC",
    cost: Object.freeze({ steel: 300, oil: 200 }),
    researchTicks: BALLISTIC_TABLES_RESEARCH_TICKS,
    description: "Artillery fire tightens over repeated shots",
    researchedAt: KIND.RESEARCH_COMPLEX,
    requiresUpgrade: UPGRADE.ARTILLERY_UNLOCK,
    requiresText: "Requires Heavy Guns",
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
  [UPGRADE.SMOKE_PLUS]: Object.freeze({
    upgrade: UPGRADE.SMOKE_PLUS,
    label: "Smoke Plus",
    icon: "SMK+",
    cost: Object.freeze({ steel: 150, oil: 150 }),
    researchTicks: SMOKE_PLUS_RESEARCH_TICKS,
    description: "Double Scout Car Smoke radius and duration",
    researchedAt: KIND.RESEARCH_COMPLEX,
  }),
});

// A building that trains units — the only buildings that accept a rally point.
export const isProducerBuilding = (kind) =>
  Array.isArray(STATS[kind]?.trains) && STATS[kind].trains.length > 0;

export const RESOURCE_AMOUNTS = Object.freeze({
  [KIND.STEEL]: 625,
  [KIND.OIL]: 962,
});

// What a worker can build (command card when a worker is selected).
export const WORKER_BUILDABLE = Object.freeze([
  KIND.CITY_CENTRE,
  KIND.PUMP_JACK,
  KIND.BARRACKS,
  KIND.TRAINING_CENTRE,
  KIND.RESEARCH_COMPLEX,
  KIND.FACTORY,
  KIND.STEELWORKS,
  KIND.TANK_TRAP,
]);

export const FIXTURE_FACTION_ID = "phase2_empty_fixture";
export const EKAT_FACTION_ID = "ekat";
