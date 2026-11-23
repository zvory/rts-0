// Client-visible rules mirror data. Gameplay is authoritative on the server;
// these values drive UI labels, command cards, fog sight radii, and rendering
// hints that are validated against Rust-owned dumps.

import { ABILITY, KIND, UPGRADE } from "../protocol.js";
import { TICK_HZ } from "./timing.js";

export const DAO_LIGHTNESS_DASH_RANGE_TILES = 10;
export function entityAbilityRangeTiles(definition, entity) {
  return definition?.ability === ABILITY.EKAT_TELEPORT && entity?.kind === KIND.LIGHTNESS_ADEPT
    ? DAO_LIGHTNESS_DASH_RANGE_TILES : definition?.rangeTiles;
}
export const DAO_WAR_RANGE_TILES = 1.75;
export const DAO_WAR_BODY_SCALE = 0.7;

export function entityBodyRadius(entity) {
  return STATS[entity?.kind]?.size || 0;
}
export const DAO_RANGED_RANGE_TILES = 5;
export const DISCIPLE_INSIGHT_REQUIRED = 60;
export const DISCIPLE_EVOLVED_INSIGHT_CAP = DISCIPLE_INSIGHT_REQUIRED * 2;

export function entityAttackRangeTiles(entity) {
  return STATS[entity?.kind]?.rangeTiles || 0;
}

// Carrier identity includes permanent Dao specialization, independently of cooldown/readiness.
export function abilityCarriedByEntity(definition, entity) {
  return !!definition?.carriers?.includes(entity?.kind);
}

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
export const MORTAR_BODY = Object.freeze({
  length: 50.4,
  width: 28.8,
  clearance: 1.2,
});
export const ARTILLERY_BODY = Object.freeze({
  length: 37.800003,
  width: 21.599998,
  clearance: 1.125,
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
export const ROCKET_LAUNCHER_BODY = Object.freeze({
  length: 48.0,
  width: 26.4,
  clearance: 1.2,
});

// Gatherers can mine a resource only when a completed home-base mining anchor is within range.
export const MINING_ANCHOR_RANGE_TILES = 11.0;
export const OIL_LOAD = 2;
export const HARVEST_TICKS = 40;
export const ANTI_TANK_GUN_DEPLOYED_RANGE_TILES = 20;
export const ANTI_TANK_GUN_FIELD_OF_FIRE_RAD = 40 * Math.PI / 180;
export const ARTILLERY_MIN_RANGE_TILES = 10;
export const ARTILLERY_MAX_RANGE_TILES = 35;
export const ARTILLERY_FIELD_OF_FIRE_RAD = 30 * Math.PI / 180;
export const ANTI_TANK_GUN_SETUP_TICKS = 120;
export const ANTI_TANK_GUN_TEARDOWN_TICKS = 30;
export const ARTILLERY_SETUP_TICKS = TICK_HZ * 6;
export const ARTILLERY_SHELL_DELAY_TICKS = TICK_HZ * 5;
export const ARTILLERY_OUTER_RADIUS_TILES = 2;
export const ARTILLERY_BLANKET_RADIUS_TILES = 15;
export const TANK_TRAP_CLUSTER_ATTACK_RADIUS_TILES = 4;
export const ARTILLERY_MIN_FIRE_RADIUS_TILES = 4;
export const ARTILLERY_FIRE_CONTROL_MIN_FIRE_RADIUS_TILES = 1;
export const ARTILLERY_AMMO_COST = Object.freeze({ steel: 0, oil: 0 });
export const SMOKE_ABILITY_RANGE_TILES = 14;
export const SMOKE_LAUNCH_MAX_DELAY_MS = 100;
export const SMOKE_CLOUD_RADIUS_TILES = 2.25;
export const SMOKE_CLOUD_DURATION_TICKS = TICK_HZ * 5;
export const SMOKE_PLUS_CLOUD_RADIUS_TILES = 4;
export const SMOKE_PLUS_CLOUD_DURATION_TICKS = SMOKE_CLOUD_DURATION_TICKS * 2;
export const SMOKE_ABILITY_COOLDOWN_TICKS = 0;
export const SCOUT_CAR_SMOKE_CHARGES = 2;
export const SCOUT_CAR_SMOKE_CHARGE_RECHARGE_TICKS = TICK_HZ * 15;
export const SMOKE_ABILITY_COST = Object.freeze({ steel: 0, oil: 0 });
export const MORTAR_RANGE_TILES = 17;
export const MORTAR_MIN_RANGE_TILES = 5;
export const MORTAR_FIELD_OF_FIRE_RAD = Math.PI * 2;
export const MORTAR_OUTER_RADIUS_TILES = 2;
// Cosmetic blast-core size; server damage now falls off continuously.
export const MORTAR_INNER_RADIUS_TILES = 0.5;
export const MORTAR_FIRE_COOLDOWN_TICKS = TICK_HZ * 6;
export const PANZERFAUST_RANGE_TILES = 5;
export const PANZERFAUST_DAMAGE = 100;
export const PANZERFAUST_ARMOR_PENETRATION = 0.5;
export const PANZERFAUST_WINDUP_TICKS = TICK_HZ / 2;
export const PANZERFAUST_TRAVEL_TICKS = TICK_HZ / 2;
export const METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS = 12;
export const METHAMPHETAMINES_RESEARCH_TICKS = TICK_HZ * 20;
export const DAO_RESEARCH_TICKS = METHAMPHETAMINES_RESEARCH_TICKS;
export const PANZERFAUSTS_RESEARCH_TICKS = TICK_HZ * 20;
export const ENTRENCHMENT_RESEARCH_TICKS = TICK_HZ * 20;
export const ENTRENCHMENT_DIG_IN_TICKS = TICK_HZ * 3;
export const ENTRENCHMENT_RANGE_BONUS_TILES = 1;
export const ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION = 0.50;
export const ENTRENCHMENT_AREA_DAMAGE_REDUCTION = 0.25;
export const ENTRENCHMENT_TRENCH_RADIUS_TILES = 0.375;
export const ARTILLERY_UNLOCK_RESEARCH_TICKS = TICK_HZ * 25;
export const BALLISTIC_TABLES_RESEARCH_TICKS = TICK_HZ * 15;
export const TANK_UNLOCK_RESEARCH_TICKS = TICK_HZ * 20;
export const SCOUT_PLANE_UNLOCK_RESEARCH_TICKS = TICK_HZ * 20;
export const ROCKETS_RESEARCH_TICKS = TICK_HZ * 25;
export const ROCKET_BARRAGE_MIN_RANGE_TILES = 10;
export const ROCKET_BARRAGE_MAX_RANGE_TILES = 44;
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
export const EKAT_LINE_SHOT_COOLDOWN_TICKS = TICK_HZ * 20;
export const EKAT_MAGIC_ANCHOR_RANGE_TILES = 5;
export const EKAT_MAGIC_ANCHOR_DURATION_TICKS = TICK_HZ * 10;
export const EKAT_MAGIC_ANCHOR_RADIUS_TILES = 3.0;
export const EKAT_MAGIC_ANCHOR_PULL_AWAY_MULTIPLIER = 0.45;
export const EKAT_MAGIC_ANCHOR_PULL_TOWARD_MULTIPLIER = 1.70;
export const BASE_COMMAND_SUPPLY_CAP = 25;
export const COMMAND_CAR_SUPPLY_CAP_BONUS = 0;
export const SCOUT_PLANE_ORBIT_RADIUS_TILES = 2;
export const SCOUT_PLANE_SPEED_PX_PER_TICK = 2.6;
export const SCOUT_PLANE_LIFETIME_TICKS = TICK_HZ * 30;
export const SCOUT_PLANE_ABILITY_COOLDOWN_TICKS = TICK_HZ * 30;

// Per-kind UI / render info. `size` is the render radius (units) or half-extent hint.
// `sight` (tiles) drives the local fog overlay. `rangeTiles` mirrors weapon range for visuals.
// `cost`/`supply` drive the command card.
export const DAO_TRAINING_COST_OIL = 100;
export const DAO_TRAINING_TICKS = TICK_HZ * 10;
export const MAX_DAO_TRAINING_QUEUE = 64;

export const STATS = Object.freeze({
  [KIND.WORKER]: { armored: false, label: "Engineer", icon: "EN", size: 9, sight: 10,
    rangeTiles: 1, cost: { steel: 50, oil: 0 }, supply: 1, buildTicks: 150 },
  [KIND.GOLEM]: { armored: false, label: "Golem", icon: "GLM", size: 9, sight: 10,
    rangeTiles: 1, cost: { steel: 0, oil: 0 }, supply: 4, buildTicks: 396 },
  [KIND.RIFLEMAN]: { armored: false, label: "Rifleman", icon: "RF", size: 9, sight: 11,
    rangeTiles: 5, cost: { steel: 35, oil: 0 }, supply: 1, buildTicks: 300 },
  [KIND.PANZERFAUST]: { armored: false, label: "Panzerfaust", icon: "PF", size: 9, sight: 11,
    rangeTiles: 5, cost: { steel: 55, oil: 10 }, supply: 1, buildTicks: 300,
    upgradeRequires: UPGRADE.PANZERFAUSTS,
    description: `Rifle infantry carrying one disposable ${PANZERFAUST_RANGE_TILES}-tile anti-vehicle shot. After launching it, the unit keeps fighting as ordinary rifle infantry.` },
  [KIND.WARRIOR]: { armored: false, label: "Novice", icon: "NV", size: 26, sight: 11,
    rangeTiles: 0.75, cost: { steel: 100, oil: 0 }, supply: 2, buildTicks: 300,
    description: "Durable melee infantry with a short sword reach and 50% armor penetration. Slow attacks deal heavy damage. Charge (automatic): 1.5x movement speed for 1 second, with a 15-second cooldown, when attacking an enemy unit at least 2 tiles away or moving away." },
  [KIND.LIGHTNESS_ADEPT]: { armored: false, label: "Lightness Adept", icon: "LA", size: 26, sight: 11,
    rangeTiles: 0.75, cost: { steel: 100, oil: 0 }, supply: 2, buildTicks: 300,
    description: "Fast sword infantry with Dash and 50% armor penetration." },
  [KIND.WAR_ADEPT]: { armored: false, label: "War Adept", icon: "WA", size: Math.fround(26 * Math.fround(DAO_WAR_BODY_SCALE)), sight: 11,
    rangeTiles: DAO_WAR_RANGE_TILES, cost: { steel: 100, oil: 0 }, supply: 2, buildTicks: 300,
    description: "Armored spear infantry with Throw Sword and 50% armor penetration." },
  [KIND.MAGIC_ADEPT]: { armored: false, label: "Magic Adept", icon: "MA", size: 26, sight: 11,
    rangeTiles: DAO_RANGED_RANGE_TILES, cost: { steel: 100, oil: 0 }, supply: 2, buildTicks: 300,
    description: "Ranged orb infantry with Anchor and Shield." },
  [KIND.MACHINE_GUNNER]: { armored: false, label: "Machine Gunner", icon: "MG", size: 10, sight: 11,
    rangeTiles: 6.1, cost: { steel: 75, oil: 10 }, supply: 2, buildTicks: 400, requires: KIND.TRAINING_CENTRE },
  [KIND.ANTI_TANK_GUN]: { armored: false, label: "Anti-Tank Gun", icon: "ATG", size: 20, sight: 9, body: ANTI_TANK_GUN_BODY,
    rangeTiles: ANTI_TANK_GUN_DEPLOYED_RANGE_TILES, cost: { steel: 150, oil: 40 }, supply: 3, buildTicks: 440,
    requires: [KIND.STEELWORKS, KIND.ENGINEERING_COMPLEX] },
  [KIND.MORTAR_TEAM]: { armored: false, label: "Mortar Team", icon: "MT", size: 21.6, sight: 10,
    rangeTiles: MORTAR_RANGE_TILES, minRangeTiles: MORTAR_MIN_RANGE_TILES,
    cost: { steel: 150, oil: 40 }, supply: 3, buildTicks: 460,
    requires: KIND.STEELWORKS,
    description: "Manual indirect fire, extremely inaccurate without vision. Every shell requires a Mortar Fire command." },
  [KIND.ARTILLERY]: { armored: false, label: "Artillery", icon: "AR", size: 13.5, sight: 7, body: ARTILLERY_BODY,
    rangeTiles: ARTILLERY_MAX_RANGE_TILES, minRangeTiles: ARTILLERY_MIN_RANGE_TILES,
    cost: { steel: 150, oil: 50 }, supply: 4, buildTicks: TICK_HZ * 20,
    requires: KIND.STEELWORKS, upgradeRequires: UPGRADE.ARTILLERY_UNLOCK,
    upgradeRequiresText: "Requires research in Engineering Complex" },
  [KIND.ROCKET_LAUNCHER]: { armored: false, label: "Rocket Truck", icon: "RT", size: 21.6, sight: 8, body: ROCKET_LAUNCHER_BODY,
    rangeTiles: 0, minRangeTiles: 10,
    cost: { steel: 225, oil: 100 }, supply: 3, buildTicks: TICK_HZ * 20,
    requires: [KIND.STEELWORKS], upgradeRequires: UPGRADE.ROCKETS,
    upgradeRequiresText: "Requires Rockets research in Engineering Complex",
    description: "Mobile rocket artillery. Stop and manually fire a 16-rocket Barrage over a wide area." },
  [KIND.SCOUT_CAR]: { armored: false, label: "Scout Car", icon: "SC", size: 14.4, sight: 15, body: SCOUT_CAR_BODY,
    rangeTiles: 7, cost: { steel: 125, oil: 60 }, supply: 3, buildTicks: 480 },
  [KIND.SCOUT_PLANE]: { armored: false, label: "Scout Plane", icon: "SP", size: 17, sight: 19, body: SCOUT_PLANE_BODY,
    blocksGroundPlacement: false,
    rangeTiles: 0, cost: { steel: 0, oil: 0 }, supply: 0, buildTicks: 0 },
  [KIND.TANK]: { armored: true, label: "Tank", icon: "TK", size: 18, sight: 9, body: TANK_BODY,
    rangeTiles: 5, cost: { steel: 350, oil: 145 }, supply: 5, buildTicks: 750,
    requires: KIND.FACTORY, upgradeRequires: UPGRADE.TANK_UNLOCK,
    upgradeRequiresText: "Requires research in Engineering Complex" },
  [KIND.COMMAND_CAR]: { armored: false, label: "Command Car", icon: "CAR", size: 12.6, sight: 8, body: COMMAND_CAR_BODY,
    rangeTiles: 0, cost: { steel: 150, oil: 85 }, supply: 4, buildTicks: TICK_HZ * 15,
    requires: KIND.ENGINEERING_COMPLEX },
  [KIND.EKAT]: { armored: false, label: "Ekat", icon: "EK", size: 10, sight: 12,
    rangeTiles: 0, cost: { steel: 0, oil: 0 }, supply: 0, buildTicks: 0 },

  [KIND.RESOURCE_DEPOT]: { label: "Resource Depot", icon: "RD", footW: 4, footH: 4, sight: 1,
    cost: { steel: 600, oil: 135 }, buildTicks: TICK_HZ * 5,
    trains: [KIND.WORKER, KIND.STEEL_MINE, KIND.PUMP_JACK] },
  [KIND.PORTAL]: { label: "Portal", icon: "PRT", footW: 3, footH: 3, sight: 1,
    cost: { steel: 150, oil: 0 }, buildTicks: TICK_HZ * 5, trains: [KIND.WARRIOR],
    description: "A supernatural gateway that trains Cultivator Novices." },
  [KIND.SHOP]: { label: "Shop", icon: "SH", footW: 3, footH: 4, sight: 1,
    cost: { steel: 200, oil: 200 }, buildTicks: TICK_HZ * 40, trains: [],
    description: "A sturdy gourd-shaped shop. No goods or services are available yet." },
  [KIND.PROVEN_GROUNDS]: { label: "Proving Grounds", icon: "PG", footW: 3, footH: 3, sight: 1,
    cost: { steel: 100, oil: 0 }, buildTicks: 200, trains: [], requires: [KIND.PORTAL],
    description: "Send base Novices inside for researched Dao training. Queue any mix of Lightness, War and Magic; each costs 100 oil and takes 10 seconds. Supports a rally point." },
  [KIND.TOURNAMENT_GROUNDS]: { label: "Tournament Grounds", icon: "TG", footW: 4, footH: 4, sight: 1,
    cost: { steel: 100, oil: 0 }, buildTicks: 200, trains: [], requires: [KIND.PORTAL],
    description: "Right-click four base Novices inside, or rally a Portal here. A free five-second tournament produces one winner with a random Dao specialization. Supports a rally point." },
  [KIND.THREE_DAO_PAVILION]: { label: "Three Dao Pavilion", icon: "DAO", footW: 3, footH: 3, sight: 1,
    cost: { steel: 100, oil: 25 }, buildTicks: 560, trains: [],
    researches: [UPGRADE.DAO_OF_LIGHTNESS, UPGRADE.DAO_OF_MAGIC, UPGRADE.DAO_OF_HEAVINESS],
    requires: [KIND.RESOURCE_DEPOT, KIND.PORTAL],
    description: "Research Lightness, War, and Magic to unlock paid training at the Proving Grounds." },
  [KIND.ZAMOK]: { label: "Zamok", icon: "ZK", footW: 3, footH: 3, sight: 1,
    cost: { steel: 0, oil: 0 }, buildTicks: 0, trains: [KIND.GOLEM] },
  [KIND.DEPOT]: { label: "Supply Depot", icon: "SD", footW: 2, footH: 2, sight: 1,
    cost: { steel: 100, oil: 0 }, buildTicks: 300, trains: [] },
  [KIND.BARRACKS]: { label: "Barracks", icon: "BK", footW: 3, footH: 2, sight: 1,
    cost: { steel: 150, oil: 0 }, buildTicks: 200, trains: [KIND.RIFLEMAN, KIND.MACHINE_GUNNER, KIND.PANZERFAUST], requires: KIND.RESOURCE_DEPOT },
  [KIND.TRAINING_CENTRE]: { label: "Training Centre", icon: "TC", footW: 3, footH: 2, sight: 1,
    cost: { steel: 100, oil: 25 }, buildTicks: 560, trains: [],
    researches: [UPGRADE.METHAMPHETAMINES, UPGRADE.PANZERFAUSTS, UPGRADE.ENTRENCHMENT],
    requires: [KIND.RESOURCE_DEPOT, KIND.BARRACKS] },
  [KIND.ENGINEERING_COMPLEX]: { label: "Engineering Complex", icon: "EC", footW: 3, footH: 3, sight: 1,
    cost: { steel: 100, oil: 100 }, buildTicks: TICK_HZ * 15, trains: [],
    researches: [
      null,
      UPGRADE.ARTILLERY_UNLOCK,
      UPGRADE.BALLISTIC_TABLES,
      UPGRADE.TANK_UNLOCK,
      UPGRADE.SMOKE_PLUS,
      UPGRADE.SCOUT_PLANE_UNLOCK,
      UPGRADE.ROCKETS,
      UPGRADE.TANK_PLUS,
    ],
    requires: [KIND.RESOURCE_DEPOT, KIND.TRAINING_CENTRE] },
  [KIND.FACTORY]: { label: "Vehicle Works", icon: "VW", footW: 3, footH: 3, sight: 1,
    cost: { steel: 125, oil: 125 }, buildTicks: 749,
    trains: [KIND.SCOUT_CAR, KIND.TANK, KIND.COMMAND_CAR],
    requires: [KIND.RESOURCE_DEPOT, KIND.TRAINING_CENTRE] },
  [KIND.STEELWORKS]: { label: "Gun Works", icon: "GW", footW: 3, footH: 3, sight: 1,
    cost: { steel: 150, oil: 100 }, buildTicks: 599,
    trains: [KIND.MORTAR_TEAM, KIND.ANTI_TANK_GUN, KIND.ARTILLERY, KIND.ROCKET_LAUNCHER],
    requires: [KIND.RESOURCE_DEPOT, KIND.TRAINING_CENTRE] },
  [KIND.BARBED_WIRE]: { label: "Barbed Wire", icon: "BW", footW: 1, footH: 1, sight: 0,
    cost: { steel: 5, oil: 0 }, buildTicks: TICK_HZ * 4, trains: [],
    requires: KIND.TRAINING_CENTRE, requiresText: "Requires Training Centre",
    description: "Slows infantry and mortar teams by 30% and deals 10 damage on entry, including allies. Disappears after two ground-unit crossings. Immune to weapons." },
  [KIND.TANK_TRAP]: { label: "Tank Trap", icon: "TT", footW: 1, footH: 1, sight: 0,
    cost: { steel: 20, oil: 0 }, buildTicks: TICK_HZ * 5, trains: [],
    requires: KIND.TRAINING_CENTRE,
    requiresText: "Requires Training Centre" },
  [KIND.STEEL_MINE]: { label: "Steel Mine", icon: "SM", footW: 1, footH: 1, sight: 1,
    cost: { steel: 0, oil: 0 }, buildTicks: TICK_HZ * 24, trains: [],
    description:
      "Resource Depots automatically build this for free on nearby Steel patches. " +
      `Extracts 2 Steel every ${(HARVEST_TICKS / TICK_HZ).toFixed(1)}s.` },
  [KIND.PUMP_JACK]: { label: "Pump Jack", icon: "PJ", footW: 1, footH: 1, sight: 1,
    cost: { steel: 150, oil: 0 }, buildTicks: TICK_HZ * 10, trains: [],
    description:
      "Engineers can build this on Oil patches for 150 Steel in 10s. " +
      "Resource Depots automatically build this for free in 36s on nearby Oil patches. " +
      `Extracts ${OIL_LOAD} Oil every ${(HARVEST_TICKS / TICK_HZ).toFixed(1)}s while within ` +
      `${MINING_ANCHOR_RANGE_TILES} tiles of a completed friendly Resource Depot or Zamok.` },

  [KIND.STEEL]: { label: "Steel", size: 22 },
  [KIND.OIL]: { label: "Oil", size: 14 },
});

export const ABILITIES = Object.freeze({
  [ABILITY.DISCIPLE_SHIELD]: Object.freeze({
    ability: ABILITY.DISCIPLE_SHIELD, label: "Shield", icon: "SHD", hotkey: "X",
    title: "Place a shield that absorbs 300 enemy damage to allied units; lasts up to 15 seconds",
    carriers: Object.freeze([KIND.MAGIC_ADEPT]), targetMode: "worldPoint",
    rangeTiles: EKAT_MAGIC_ANCHOR_RANGE_TILES, radiusTiles: EKAT_MAGIC_ANCHOR_RADIUS_TILES,
    cooldownTicks: 300, cost: Object.freeze({ steel: 0, oil: 0 }),
    queued: true, queuePolicy: "skipIfNotReady",
  }),
  [ABILITY.EVOLVE_LIGHTNESS]: Object.freeze({
    ability: ABILITY.EVOLVE_LIGHTNESS, label: "Dao of Lightness", icon: "Z", hotkey: "Z",
    title: "Choose Dao of Lightness; evolves automatically at 60 insight; no research required", carriers: Object.freeze([KIND.WARRIOR]),
    targetMode: "self", cooldownTicks: 0, cost: Object.freeze({ steel: 0, oil: 0 }),
    upgradeRequirement: null, queued: false, queuePolicy: "notQueueable",
  }),
  [ABILITY.EVOLVE_WAR]: Object.freeze({
    ability: ABILITY.EVOLVE_WAR, label: "Dao of War", icon: "X", hotkey: "X",
    title: "Choose Dao of War; evolves automatically at 60 insight; no research required", carriers: Object.freeze([KIND.WARRIOR]),
    targetMode: "self", cooldownTicks: 0, cost: Object.freeze({ steel: 0, oil: 0 }),
    upgradeRequirement: null, queued: false, queuePolicy: "notQueueable",
  }),
  [ABILITY.EVOLVE_MAGIC]: Object.freeze({
    ability: ABILITY.EVOLVE_MAGIC, label: "Dao of Magic", icon: "C", hotkey: "C",
    title: "Choose Dao of Magic; evolves automatically at 60 insight; no research required", carriers: Object.freeze([KIND.WARRIOR]),
    targetMode: "self", cooldownTicks: 0, cost: Object.freeze({ steel: 0, oil: 0 }),
    upgradeRequirement: null, queued: false, queuePolicy: "notQueueable",
  }),

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
    charges: SCOUT_CAR_SMOKE_CHARGES,
    chargeRechargeTicks: SCOUT_CAR_SMOKE_CHARGE_RECHARGE_TICKS,
    cost: SMOKE_ABILITY_COST,
    techRequirement: KIND.ENGINEERING_COMPLEX,
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
  }),
  [ABILITY.BARRAGE]: Object.freeze({
    ability: ABILITY.BARRAGE,
    label: "Barrage",
    icon: "RKT",
    hotkey: "X",
    classicHotkey: "F",
    commandCardPriority: -1,
    title: "Target a 16-rocket saturation barrage",
    carriers: Object.freeze([KIND.ROCKET_LAUNCHER]),
    targetMode: "worldPoint",
    rangeTiles: ROCKET_BARRAGE_MAX_RANGE_TILES,
    minRangeTiles: ROCKET_BARRAGE_MIN_RANGE_TILES,
    cooldownTicks: TICK_HZ * 30,
    charges: 1,
    cost: Object.freeze({ steel: 0, oil: 150 }),
    radiusTiles: 6,
    upgradeRequirement: UPGRADE.ROCKETS,
    queued: true,
    queuePolicy: "waitUntilReady",
  }),
  [ABILITY.POINT_FIRE]: Object.freeze({
    ability: ABILITY.POINT_FIRE,
    label: "Fire",
    icon: "FIR",
    hotkey: "X",
    commandCardPriority: -1,
    title: "Click a target, then click again to set the fire radius",
    carriers: Object.freeze([KIND.ARTILLERY]),
    targetMode: "worldPoint",
    rangeTiles: ARTILLERY_MAX_RANGE_TILES,
    minRangeTiles: ARTILLERY_MIN_RANGE_TILES,
    cooldownTicks: TICK_HZ * 6,
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
    cooldownTicks: TICK_HZ * 6,
    cost: ARTILLERY_AMMO_COST,
    radiusTiles: ARTILLERY_BLANKET_RADIUS_TILES,
    queued: true,
    queuePolicy: "skipIfNotReady",
    commandCard: false,
  }),
  [ABILITY.BREAKTHROUGH]: Object.freeze({
    ability: ABILITY.BREAKTHROUGH,
    label: "Breakthrough!",
    icon: "BRK",
    hotkey: "E",
    title: "Activate full speed and moving fire with half attack cooldown (stronger in smoke)",
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
    title: "Launch a scout plane sortie",
    carriers: Object.freeze([KIND.COMMAND_CAR]),
    targetMode: "worldPoint",
    rangeTiles: null,
    cooldownTicks: SCOUT_PLANE_ABILITY_COOLDOWN_TICKS,
    cost: Object.freeze({ steel: 0, oil: 0 }),
    upgradeRequirement: UPGRADE.SCOUT_PLANE_UNLOCK,
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
    carriers: Object.freeze([KIND.EKAT, KIND.LIGHTNESS_ADEPT]),
    targetMode: "worldPoint",
    rangeTiles: EKAT_TELEPORT_RANGE_TILES,
    cooldownTicks: EKAT_TELEPORT_COOLDOWN_TICKS,
    cost: Object.freeze({ steel: 0, oil: 0 }),
    queued: true,
    queuePolicy: "skipIfNotReady",
  }),
  [ABILITY.THROW_SWORD]: Object.freeze({
    ability: ABILITY.THROW_SWORD,
    label: "Throw Sword",
    icon: "⚔",
    hotkey: "E",
    title: "Throw a spinning sword out and back; deals 40 damage outbound and 80 on return",
    carriers: Object.freeze([KIND.WAR_ADEPT]),
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
    label: "Anchor",
    icon: "ANC",
    hotkey: "C",
    title: "Place a 10-second pull field",
    carriers: Object.freeze([KIND.EKAT, KIND.MAGIC_ADEPT]),
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
  [UPGRADE.DAO_OF_HEAVINESS]: Object.freeze({
    upgrade: UPGRADE.DAO_OF_HEAVINESS, label: "Dao of War", icon: "HVY",
    cost: Object.freeze({ steel: 100, oil: 200 }), researchTicks: DAO_RESEARCH_TICKS,
    description: "Study the Dao of War. Unlocks the corresponding paid Adept training at the Proving Grounds.",
    researchedAt: KIND.THREE_DAO_PAVILION,
  }),
  [UPGRADE.DAO_OF_MAGIC]: Object.freeze({
    upgrade: UPGRADE.DAO_OF_MAGIC, label: "Dao of Magic", icon: "MAG",
    cost: Object.freeze({ steel: 100, oil: 200 }), researchTicks: DAO_RESEARCH_TICKS,
    description: "Study the Dao of Magic. Unlocks the corresponding paid Adept training at the Proving Grounds.",
    researchedAt: KIND.THREE_DAO_PAVILION,
  }),
  [UPGRADE.DAO_OF_LIGHTNESS]: Object.freeze({
    upgrade: UPGRADE.DAO_OF_LIGHTNESS, label: "Dao of Lightness", icon: "LGT",
    cost: Object.freeze({ steel: 100, oil: 200 }), researchTicks: DAO_RESEARCH_TICKS,
    description: "Study the Dao of Lightness. Unlocks the corresponding paid Adept training at the Proving Grounds.",
    researchedAt: KIND.THREE_DAO_PAVILION,
  }),
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
    description: `Unlock Panzerfaust rifle infantry at the Barracks`,
    researchedAt: KIND.TRAINING_CENTRE,
  }),
  [UPGRADE.ENTRENCHMENT]: Object.freeze({
    upgrade: UPGRADE.ENTRENCHMENT,
    label: "Entrenchment",
    icon: "ENT",
    cost: Object.freeze({ steel: 100, oil: 0 }),
    researchTicks: ENTRENCHMENT_RESEARCH_TICKS,
    description: "Let eligible infantry create and use persistent trenches",
    researchedAt: KIND.TRAINING_CENTRE,
  }),
  [UPGRADE.ARTILLERY_UNLOCK]: Object.freeze({
    upgrade: UPGRADE.ARTILLERY_UNLOCK,
    label: "Artillery",
    icon: "ART",
    cost: Object.freeze({ steel: 200, oil: 100 }),
    researchTicks: ARTILLERY_UNLOCK_RESEARCH_TICKS,
    description: "Unlock Artillery training",
    researchedAt: KIND.ENGINEERING_COMPLEX,
  }),
  [UPGRADE.BALLISTIC_TABLES]: Object.freeze({
    upgrade: UPGRADE.BALLISTIC_TABLES,
    label: "Artillery Fire Control",
    icon: "AFC",
    cost: Object.freeze({ steel: 50, oil: 100 }),
    researchTicks: BALLISTIC_TABLES_RESEARCH_TICKS,
    description: "Reduce Artillery Fire minimum radius from 4 to 1 tile",
    researchedAt: KIND.ENGINEERING_COMPLEX,
    requiresUpgrade: UPGRADE.ARTILLERY_UNLOCK,
    requiresText: "Requires Artillery",
  }),
  [UPGRADE.TANK_UNLOCK]: Object.freeze({
    upgrade: UPGRADE.TANK_UNLOCK,
    label: "Tank Production",
    icon: "TK+",
    cost: Object.freeze({ steel: 150, oil: 100 }),
    researchTicks: TANK_UNLOCK_RESEARCH_TICKS,
    description: "Unlock Tank training",
    researchedAt: KIND.ENGINEERING_COMPLEX,
  }),
  [UPGRADE.TANK_PLUS]: Object.freeze({
    upgrade: UPGRADE.TANK_PLUS,
    label: "Tank Plus",
    icon: "TK++",
    cost: Object.freeze({ steel: 150, oil: 150 }),
    researchTicks: TICK_HZ * 20,
    description: "Tanks turn 50% faster, gain stationary range twice as fast, +2 sight and +10% top speed",
    researchedAt: KIND.ENGINEERING_COMPLEX,
    requiresUpgrade: UPGRADE.TANK_UNLOCK,
    requiresText: "Requires Tank Production",
  }),
  [UPGRADE.SMOKE_PLUS]: Object.freeze({
    upgrade: UPGRADE.SMOKE_PLUS,
    label: "Smoke Plus",
    icon: "SMK+",
    cost: Object.freeze({ steel: 150, oil: 150 }),
    researchTicks: SMOKE_PLUS_RESEARCH_TICKS,
    description: "Increase Scout Car Smoke radius to 4 tiles and double duration",
    researchedAt: KIND.ENGINEERING_COMPLEX,
  }),
  [UPGRADE.SCOUT_PLANE_UNLOCK]: Object.freeze({
    upgrade: UPGRADE.SCOUT_PLANE_UNLOCK,
    label: "Scout Plane",
    icon: "SP+",
    cost: Object.freeze({ steel: 100, oil: 200 }),
    researchTicks: SCOUT_PLANE_UNLOCK_RESEARCH_TICKS,
    description: "Unlock the Command Car Scout Plane ability",
    researchedAt: KIND.ENGINEERING_COMPLEX,
  }),
  [UPGRADE.ROCKETS]: Object.freeze({
    upgrade: UPGRADE.ROCKETS,
    label: "Rockets",
    icon: "RKT+",
    cost: Object.freeze({ steel: 75, oil: 125 }),
    researchTicks: ROCKETS_RESEARCH_TICKS,
    description: "Unlock Rocket Truck training at the Gun Works",
    researchedAt: KIND.ENGINEERING_COMPLEX,
  }),
});

// A building that trains units — the only buildings that accept a rally point.
export const isProducerBuilding = (kind) =>
  kind === KIND.TOURNAMENT_GROUNDS || kind === KIND.PROVEN_GROUNDS || (Array.isArray(STATS[kind]?.trains) && STATS[kind].trains.length > 0);

export const RESOURCE_AMOUNTS = Object.freeze({
  [KIND.STEEL]: 625,
  [KIND.OIL]: 962,
});

// What a worker can build (command card when a worker is selected).
export const WORKER_BUILDABLE = Object.freeze([
  KIND.RESOURCE_DEPOT,
  KIND.PUMP_JACK,
  KIND.BARRACKS,
  KIND.TRAINING_CENTRE,
  KIND.ENGINEERING_COMPLEX,
  KIND.FACTORY,
  KIND.STEELWORKS,
  KIND.TANK_TRAP,
  KIND.BARBED_WIRE,
]);

export const FIXTURE_FACTION_ID = "phase2_empty_fixture";
export const EKAT_FACTION_ID = "ekat";
export const CULTIVATORS_FACTION_ID = "cultivators";

// Dao research tier is shared across a player's completed and paid active researches.
export function researchDefinition(upgrade, upgrades = [], entities = [], owner = null) {
  const def = UPGRADES[upgrade];
  if (!def || def.researchedAt !== KIND.THREE_DAO_PAVILION) return def;
  const isDao = (id) => UPGRADES[id]?.researchedAt === KIND.THREE_DAO_PAVILION;
  const active = entities.filter((entity) => entity.owner === owner &&
    entity.kind === KIND.THREE_DAO_PAVILION && entity.prodUpgrade && !entity.prodWaiting)
    .map((entity) => entity.prodUpgrade);
  const count = new Set([...upgrades, ...active].filter(isDao)).size;
  const multiplier = 2 ** Math.min(count, 2) / 2;
  return { ...def, cost: { steel: def.cost.steel * multiplier, oil: def.cost.oil * multiplier },
    researchTicks: def.researchTicks * multiplier };
}
