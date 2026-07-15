// tests/client_contracts/config_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert, assertDeepEqual } from "./assertions.mjs";
import * as configExports from "../../client/src/config.js";
import {
  ARTILLERY_MAX_RANGE_TILES,
  ARTILLERY_MIN_RANGE_TILES,
  ARTILLERY_BLANKET_RADIUS_TILES,
  ARTILLERY_SHELL_DELAY_TICKS,
  MINING_CC_RANGE_TILES,
  SMOKE_ABILITY_COST,
  SMOKE_CLOUD_DURATION_TICKS,
  SMOKE_CLOUD_RADIUS_TILES,
  SMOKE_PLUS_CLOUD_DURATION_TICKS,
  SMOKE_PLUS_CLOUD_RADIUS_TILES,
  METHAMPHETAMINES_PANZERFAUST_RECOVERY_TICKS,
  METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS,
  PANZERFAUST_ARMOR_PENETRATION,
  PANZERFAUST_DAMAGE,
  PANZERFAUST_RANGE_TILES,
  PANZERFAUST_RECOVERY_TICKS,
  PANZERFAUST_TRAVEL_TICKS,
  PANZERFAUST_WINDUP_TICKS,
  ENTRENCHMENT_AREA_DAMAGE_REDUCTION,
  ENTRENCHMENT_DIG_IN_TICKS,
  ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION,
  ENTRENCHMENT_RANGE_BONUS_TILES,
  ENTRENCHMENT_RESEARCH_TICKS,
  ENTRENCHMENT_TRENCH_RADIUS_TILES,
  ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS,
  ARTILLERY_UNLOCK_RESEARCH_TICKS,
  ABILITIES,
  STATS,
  TICK_HZ,
  UPGRADES,
  WORKER_BUILDABLE,
  WORKER_BUILD_CARD_SLOTS,
  SMOKE_PLUS_RESEARCH_TICKS,
} from "../../client/src/config.js";
import {
  HUD,
  formatTankOilUsed,
  groupCooldownClocks,
  playerHasCompletedKind,
} from "../../client/src/hud.js";
import { buildCommandCardDescriptors } from "../../client/src/hud_command_card.js";
import {
  ABILITY,
  ABILITY_CODE,
  EVENT,
  EVENT_CODE,
  KIND,
  KIND_CODE,
  ORDER_STAGE,
  ORDER_STAGE_CODE,
  SETUP,
  UPGRADE,
  UPGRADE_CODE,
} from "../../client/src/protocol.js";
import { Input } from "../../client/src/input/index.js";
import { ClientIntent } from "../../client/src/client_intent.js";

const EXPECTED_CONFIG_EXPORT_NAMES = Object.freeze([
  "ABILITIES",
  "ANTI_TANK_GUN_BODY",
  "ANTI_TANK_GUN_DEPLOYED_RANGE_TILES",
  "ANTI_TANK_GUN_FIELD_OF_FIRE_RAD",
  "ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS",
  "ARTILLERY_AMMO_COST",
  "ARTILLERY_BLANKET_RADIUS_TILES",
  "ARTILLERY_BODY",
  "ARTILLERY_FIELD_OF_FIRE_RAD",
  "ARTILLERY_MAX_RANGE_TILES",
  "ARTILLERY_MIN_RANGE_TILES",
  "ARTILLERY_OUTER_RADIUS_TILES",
  "ARTILLERY_SETUP_TICKS",
  "ARTILLERY_SHELL_DELAY_TICKS",
  "ARTILLERY_UNLOCK_RESEARCH_TICKS",
  "BALLISTIC_TABLES_RESEARCH_TICKS",
  "BASE_COMMAND_SUPPLY_CAP",
  "BREAKTHROUGH_COOLDOWN_TICKS",
  "BREAKTHROUGH_DURATION_TICKS",
  "BREAKTHROUGH_RADIUS_TILES",
  "CAMERA",
  "COLORS",
  "COMMAND_CAR_BODY",
  "COMMAND_CAR_SUPPLY_CAP_BONUS",
  "EKAT_CONSUME_GOLEM_RANGE_TILES",
  "EKAT_FACTION_ID",
  "EKAT_LINE_SHOT_COOLDOWN_TICKS",
  "EKAT_LINE_SHOT_DAMAGE",
  "EKAT_LINE_SHOT_RANGE_TILES",
  "EKAT_LINE_SHOT_SPEED_PX_PER_TICK",
  "EKAT_LINE_SHOT_WIDTH_TILES",
  "EKAT_MAGIC_ANCHOR_DURATION_TICKS",
  "EKAT_MAGIC_ANCHOR_PULL_AWAY_MULTIPLIER",
  "EKAT_MAGIC_ANCHOR_PULL_TOWARD_MULTIPLIER",
  "EKAT_MAGIC_ANCHOR_RADIUS_TILES",
  "EKAT_MAGIC_ANCHOR_RANGE_TILES",
  "EKAT_TELEPORT_COOLDOWN_TICKS",
  "EKAT_TELEPORT_RANGE_TILES",
  "ENTRENCHMENT_AREA_DAMAGE_REDUCTION",
  "ENTRENCHMENT_DIG_IN_TICKS",
  "ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION",
  "ENTRENCHMENT_RANGE_BONUS_TILES",
  "ENTRENCHMENT_RESEARCH_TICKS",
  "ENTRENCHMENT_TRENCH_RADIUS_TILES",
  "FACTION_CATALOGS",
  "FIXTURE_FACTION_ID",
  "FOG_EXPLORED_ALPHA",
  "FOG_UNEXPLORED_ALPHA",
  "INTERP_DELAY_MS",
  "METHAMPHETAMINES_PANZERFAUST_RECOVERY_TICKS",
  "METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS",
  "METHAMPHETAMINES_RESEARCH_TICKS",
  "MINING_CC_RANGE_TILES",
  "MORTAR_AUTOCAST_RESEARCH_TICKS",
  "MORTAR_FIRE_COOLDOWN_TICKS",
  "MORTAR_INNER_RADIUS_TILES",
  "MORTAR_OUTER_RADIUS_TILES",
  "MORTAR_RANGE_TILES",
  "MORTAR_SHELL_DELAY_TICKS",
  "PANZERFAUST_ARMOR_PENETRATION",
  "PANZERFAUST_DAMAGE",
  "PANZERFAUST_RANGE_TILES",
  "PANZERFAUST_RECOVERY_TICKS",
  "PANZERFAUST_TRAVEL_TICKS",
  "PANZERFAUST_WINDUP_TICKS",
  "PLAYER_PALETTE",
  "RESOURCE_AMOUNTS",
  "SCOUT_CAR_BODY",
  "SCOUT_CAR_SMOKE_USES",
  "SCOUT_PLANE_ABILITY_COOLDOWN_TICKS",
  "SCOUT_PLANE_BODY",
  "SCOUT_PLANE_LIFETIME_TICKS",
  "SCOUT_PLANE_ORBIT_RADIUS_TILES",
  "SCOUT_PLANE_SPEED_PX_PER_TICK",
  "SMOKE_ABILITY_COOLDOWN_TICKS",
  "SMOKE_ABILITY_COST",
  "SMOKE_ABILITY_RANGE_TILES",
  "SMOKE_CLOUD_DURATION_TICKS",
  "SMOKE_CLOUD_RADIUS_TILES",
  "SMOKE_LAUNCH_MAX_DELAY_MS",
  "SMOKE_PLUS_CLOUD_DURATION_TICKS",
  "SMOKE_PLUS_CLOUD_RADIUS_TILES",
  "SMOKE_PLUS_RESEARCH_TICKS",
  "SNAPSHOT_INTERP_DELAY_TICKS",
  "SNAPSHOT_MS",
  "STATS",
  "TANK_BODY",
  "TANK_UNLOCK_RESEARCH_TICKS",
  "TICK_HZ",
  "UPGRADES",
  "WORKER_BUILDABLE",
  "WORKER_BUILD_CARD_SLOTS",
  "commandCardAbilitiesForFaction",
  "factionCatalog",
  "isProducerBuilding",
  "researchableUpgradesForFaction",
  "trainableUnitsForFaction",
  "workerBuildablesForFaction",
]);

// Config
// ---------------------------------------------------------------------------
{
  assertDeepEqual(
    Object.keys(configExports).sort(),
    EXPECTED_CONFIG_EXPORT_NAMES,
    "client config public export names remain stable across internal splits",
  );
  assert(MINING_CC_RANGE_TILES === 9, "client mirrors the server mining City Centre range");
  assert(STATS[KIND.CITY_CENTRE].cost.steel === 350, "City Centre cost mirrors server");
  assert(
    Array.isArray(STATS[KIND.FACTORY].requires),
    "Vehicle Works should expose all server-side build prerequisites",
  );
  assert(
    STATS[KIND.FACTORY].label === "Vehicle Works",
    "factory protocol kind should present as Vehicle Works",
  );
  assert(
    STATS[KIND.STEELWORKS].label === "Gun Works",
    "steelworks protocol kind should present as Gun Works",
  );
  assert(
    STATS[KIND.SCOUT_PLANE].cost.steel === 50 &&
      STATS[KIND.SCOUT_PLANE].cost.oil === 75 &&
      STATS[KIND.SCOUT_PLANE].supply === 0 &&
      STATS[KIND.SCOUT_PLANE].buildTicks === 0 &&
      STATS[KIND.SCOUT_PLANE].body.length === 48 &&
      STATS[KIND.SCOUT_PLANE].blocksGroundPlacement === false,
    "Scout Plane stats mirror the approved ability-launched unit contract",
  );
  assert(
    !STATS[KIND.CITY_CENTRE].trains.includes(KIND.SCOUT_PLANE) &&
      !configExports.trainableUnitsForFaction("kriegsia", KIND.CITY_CENTRE).includes(KIND.SCOUT_PLANE) &&
      ABILITIES[ABILITY.SCOUT_PLANE].carriers.includes(KIND.COMMAND_CAR) &&
      ABILITIES[ABILITY.SCOUT_PLANE].hotkey === "C" &&
      ABILITIES[ABILITY.SCOUT_PLANE].requires == null &&
      ABILITIES[ABILITY.SCOUT_PLANE].cost.steel === 50 &&
      ABILITIES[ABILITY.SCOUT_PLANE].cost.oil === 75 &&
      ABILITIES[ABILITY.SCOUT_PLANE].durationTicks === 600,
    "Command Car command card exposes Scout Plane as the C-slot ability",
  );
  assert(
    Array.isArray(STATS[KIND.TRAINING_CENTRE].requires),
    "Training Centre should expose all server-side build prerequisites",
  );
  assert(
    STATS[KIND.TRAINING_CENTRE].requires.includes(KIND.CITY_CENTRE),
    "Training Centre should require a City Centre in the command card",
  );
  assert(
    STATS[KIND.TRAINING_CENTRE].requires.includes(KIND.BARRACKS),
    "Training Centre should require a Barracks in the command card",
  );
  assert(STATS[KIND.TRAINING_CENTRE].buildTicks === 560, "Training Centre build time mirrors server");
  assert(
    STATS[KIND.FACTORY].requires.includes(KIND.CITY_CENTRE),
    "Vehicle Works should require a City Centre in the command card",
  );
  assert(
    STATS[KIND.FACTORY].requires.includes(KIND.TRAINING_CENTRE),
    "Vehicle Works should require a Training Centre in the command card",
  );
  assert(STATS[KIND.FACTORY].buildTicks === 749, "Vehicle Works build time mirrors server");
  assert(
    STATS[KIND.FACTORY].trains[0] === KIND.SCOUT_CAR,
    "Vehicle Works should put Scout Car in the leftmost train slot",
  );
  assert(
    STATS[KIND.FACTORY].trains.includes(KIND.TANK),
    "Vehicle Works should train Tanks after the unlock",
  );
  assert(
    STATS[KIND.FACTORY].trains[2] === KIND.COMMAND_CAR,
    "Vehicle Works should put Command Car in the top-right train slot",
  );
  assert(STATS[KIND.SCOUT_CAR].cost.steel === 125, "Scout Car steel cost mirrors server");
  assert(STATS[KIND.SCOUT_CAR].cost.oil === 50, "Scout Car oil cost mirrors server");
  assert(STATS[KIND.SCOUT_CAR].sight === 15, "Scout Car sight radius mirrors server");
  assert(SMOKE_ABILITY_COST.steel === 0 && SMOKE_ABILITY_COST.oil === 0, "Scout Car smoke has no resource cost");
  assert(!("requires" in ABILITIES[ABILITY.SMOKE]), "Scout Car smoke should be available without Gun Works");
  assert(STATS[KIND.SCOUT_CAR].body.length === 40.8, "Scout Car client body length mirrors server");
  assert(STATS[KIND.SCOUT_CAR].body.width === 21.6, "Scout Car client body width mirrors server");
  assert(KIND_CODE[KIND.SCOUT_CAR] === 14, "Scout Car compact kind code should follow steelworks protocol kind");
  assert(KIND_CODE[KIND.ARTILLERY] === 16, "Artillery compact kind code should be reserved");
  assert(KIND_CODE[KIND.RESEARCH_COMPLEX] === 17, "R&D Complex compact kind code should be reserved");
  assert(KIND_CODE[KIND.COMMAND_CAR] === 18, "Command Car compact kind code should be reserved");
  assert(KIND_CODE[KIND.EKAT] === 19, "Ekat compact kind code should be reserved");
  assert(KIND_CODE[KIND.ZAMOK] === 20, "Zamok compact kind code should be reserved");
  assert(KIND_CODE[KIND.TANK_TRAP] === 21, "Tank Trap compact kind code should be reserved");
  assert(KIND_CODE[KIND.PANZERFAUST] === 24, "Panzerfaust compact kind code should be reserved");
  assert(ABILITY_CODE[ABILITY.POINT_FIRE] === 4, "Point Fire compact ability code should be reserved");
  assert(ABILITY_CODE[ABILITY.BREAKTHROUGH] === 5, "Breakthrough compact ability code should be reserved");
  assert(ABILITY_CODE[ABILITY.EKAT_TELEPORT] === 6, "Ekat Teleport compact ability code should be reserved");
  assert(ABILITY_CODE[ABILITY.EKAT_LINE_SHOT] === 7, "Ekat Line Shot compact ability code should be reserved");
  assert(ABILITY_CODE[ABILITY.EKAT_MAGIC_ANCHOR] === 8, "Ekat Magic Anchor compact ability code should be reserved");
  assert(ABILITY_CODE[ABILITY.BLANKET_FIRE] === 10, "Blanket Fire compact ability code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.POINT_FIRE] === 10, "Point Fire compact order stage code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.BREAKTHROUGH] === 11, "Breakthrough compact order stage code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.EKAT_TELEPORT] === 12, "Ekat Teleport compact order stage code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.EKAT_LINE_SHOT] === 13, "Ekat Line Shot compact order stage code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.EKAT_MAGIC_ANCHOR] === 14, "Ekat Magic Anchor compact order stage code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.DECONSTRUCT] === 15, "Deconstruct compact order stage code should be reserved");
  assert(ORDER_STAGE_CODE[ORDER_STAGE.BLANKET_FIRE] === 17, "Blanket Fire compact order stage code should be reserved");
  assert(EVENT_CODE[EVENT.ARTILLERY_TARGET] === 7, "Artillery target compact event code should be reserved");
  assert(EVENT_CODE[EVENT.ARTILLERY_IMPACT] === 8, "Artillery impact compact event code should be reserved");
  assert(EVENT_CODE[EVENT.MORTAR_LAUNCH] === 9, "Mortar launch compact event code should be reserved");
  assert(EVENT_CODE[EVENT.OVERPENETRATION] === 10, "Overpenetration compact event code should be reserved");
  assert(EVENT_CODE[EVENT.ARTILLERY_FIRING] === 11, "Artillery firing compact event code should be reserved");
  assert(EVENT_CODE[EVENT.PANZERFAUST_LAUNCH] === 12, "Panzerfaust launch compact event code should be reserved");
  assert(EVENT_CODE[EVENT.PANZERFAUST_IMPACT] === 13, "Panzerfaust impact compact event code should be reserved");
  assert(EVENT_CODE[EVENT.PANZERFAUST_CONVERSION] === 14, "Panzerfaust conversion compact event code should be reserved");
  assert(EVENT_CODE[EVENT.MISS] === 15, "Miss compact event code should be reserved");
  assert(UPGRADE_CODE[UPGRADE.MORTAR_AUTOCAST] === 5, "Mortar Autocast compact upgrade code should be reserved");
  assert(UPGRADE_CODE[UPGRADE.BALLISTIC_TABLES] === 7, "Artillery Fire Control compact upgrade code should be reserved");
  assert(UPGRADE_CODE[UPGRADE.ENTRENCHMENT] === 8, "Entrenchment compact upgrade code should be reserved");
  assert(UPGRADE_CODE[UPGRADE.SMOKE_PLUS] === 9, "Smoke Plus compact upgrade code should be reserved");
  assert(
    STATS[KIND.COMMAND_CAR].cost.steel === 150 &&
      STATS[KIND.COMMAND_CAR].cost.oil === 75 &&
      STATS[KIND.COMMAND_CAR].supply === 4 &&
      STATS[KIND.COMMAND_CAR].sight === 13 &&
      STATS[KIND.COMMAND_CAR].size < STATS[KIND.SCOUT_CAR].size &&
      STATS[KIND.COMMAND_CAR].body.length < STATS[KIND.SCOUT_CAR].body.length &&
      STATS[KIND.COMMAND_CAR].body.width < STATS[KIND.SCOUT_CAR].body.width,
    "Command Car stats mirror the planned server values and use a smaller body than Scout Car",
  );
  assert(
    ABILITIES[ABILITY.BREAKTHROUGH].carriers.includes(KIND.COMMAND_CAR) &&
      ABILITIES[ABILITY.BREAKTHROUGH].targetMode === "self" &&
      ABILITIES[ABILITY.BREAKTHROUGH].radiusTiles === 9 &&
      ABILITIES[ABILITY.BREAKTHROUGH].durationTicks === 180 &&
      ABILITIES[ABILITY.BREAKTHROUGH].cooldownTicks === 750,
    "Breakthrough ability exposes Command Car carrier, self target, radius, duration, and cooldown",
  );
  assert(
    STATS[KIND.PANZERFAUST].cost.steel === 60 &&
      STATS[KIND.PANZERFAUST].cost.oil === 15 &&
      STATS[KIND.PANZERFAUST].supply === 1 &&
      STATS[KIND.PANZERFAUST].sight === 11 &&
      STATS[KIND.PANZERFAUST].size === 9 &&
      STATS[KIND.PANZERFAUST].rangeTiles === 4 &&
      STATS[KIND.PANZERFAUST].buildTicks === 400 &&
      PANZERFAUST_RANGE_TILES === 5 &&
      PANZERFAUST_DAMAGE === 100 &&
      PANZERFAUST_ARMOR_PENETRATION === 0.5 &&
      PANZERFAUST_WINDUP_TICKS === 15 &&
      PANZERFAUST_TRAVEL_TICKS === 15 &&
      PANZERFAUST_RECOVERY_TICKS === 60 &&
      METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS === 12 &&
      METHAMPHETAMINES_PANZERFAUST_RECOVERY_TICKS === 60,
    "Panzerfaust stats and reload timing mirror server",
  );
  assert(
    STATS[KIND.BARRACKS].trains[2] === KIND.PANZERFAUST &&
      configExports.trainableUnitsForFaction("kriegsia", KIND.BARRACKS)[2] === KIND.PANZERFAUST,
    "Barracks command card exposes Panzerfaust as the third Kriegsia train button",
  );
  assert(
    STATS[KIND.PANZERFAUST].requires === KIND.TRAINING_CENTRE &&
      STATS[KIND.PANZERFAUST].description.includes("prioritizes visible Tanks") &&
      STATS[KIND.PANZERFAUST].description.includes("vehicles and buildings") &&
      STATS[KIND.PANZERFAUST].description.includes("50% armor penetration") &&
      STATS[KIND.PANZERFAUST].description.includes("one disposable 5-tile anti-tank shot") &&
      STATS[KIND.PANZERFAUST].description.includes("Fights with normal rifle fire") &&
      STATS[KIND.PANZERFAUST].description.includes("becoming a Rifleman") &&
      STATS[KIND.PANZERFAUST].description.includes("Methamphetamines improves its rifle fire"),
    "Panzerfaust command-card metadata exposes its Training Centre requirement and approved tooltip copy",
  );
  assert(
    STATS[KIND.ARTILLERY].cost.steel === 300 &&
      STATS[KIND.ARTILLERY].cost.oil === 100 &&
      STATS[KIND.ARTILLERY].supply === 5,
    "Artillery cost and supply mirror server",
  );
  assert(
    STATS[KIND.ARTILLERY].upgradeRequires === UPGRADE.ARTILLERY_UNLOCK,
    "Artillery training requires Heavy Guns",
  );
  assert(
    ABILITIES[ABILITY.POINT_FIRE].carriers.includes(KIND.ARTILLERY) &&
      ABILITIES[ABILITY.POINT_FIRE].rangeTiles === ARTILLERY_MAX_RANGE_TILES &&
      ABILITIES[ABILITY.POINT_FIRE].minRangeTiles === ARTILLERY_MIN_RANGE_TILES &&
      ABILITIES[ABILITY.POINT_FIRE].delayTicks === ARTILLERY_SHELL_DELAY_TICKS &&
      ARTILLERY_SHELL_DELAY_TICKS === 150,
    "Point Fire ability exposes Artillery carrier, max range, minimum range, and 5-second delay",
  );
  assert(
    ABILITIES[ABILITY.BLANKET_FIRE].carriers.includes(KIND.ARTILLERY) &&
      ABILITIES[ABILITY.BLANKET_FIRE].rangeTiles === ARTILLERY_MAX_RANGE_TILES &&
      ABILITIES[ABILITY.BLANKET_FIRE].minRangeTiles === ARTILLERY_MIN_RANGE_TILES &&
      ABILITIES[ABILITY.BLANKET_FIRE].radiusTiles === ARTILLERY_BLANKET_RADIUS_TILES &&
      ABILITIES[ABILITY.BLANKET_FIRE].cost === ABILITIES[ABILITY.POINT_FIRE].cost &&
      ABILITIES[ABILITY.BLANKET_FIRE].cooldownTicks === ABILITIES[ABILITY.POINT_FIRE].cooldownTicks &&
      ABILITIES[ABILITY.BLANKET_FIRE].queued === true &&
      ABILITIES[ABILITY.BLANKET_FIRE].hotkey === "C" &&
      ARTILLERY_BLANKET_RADIUS_TILES === 15,
    "Blanket Fire descriptor exposes Artillery carrier, range band, radius, cost, cooldown, queueability, and hotkey",
  );
  assert(
    configExports.commandCardAbilitiesForFaction().some((entry) => entry.ability === ABILITY.BLANKET_FIRE),
    "Blanket Fire descriptor is exposed in command-card ability lists",
  );
  assert(
    ABILITIES[ABILITY.EKAT_TELEPORT].queued === true &&
      ABILITIES[ABILITY.EKAT_LINE_SHOT].queued === true &&
      ABILITIES[ABILITY.EKAT_MAGIC_ANCHOR].queued === true,
    "Ekat abilities expose queued command support",
  );
  assert(
    STATS[KIND.STEELWORKS].footW === 3 && STATS[KIND.STEELWORKS].footH === 3,
    "Gun Works should be a 3x3 building",
  );
  assert(
    STATS[KIND.STEELWORKS].cost.steel === 150 && STATS[KIND.STEELWORKS].cost.oil === 100,
    "Gun Works cost mirrors server",
  );
  assert(STATS[KIND.STEELWORKS].buildTicks === 599, "Gun Works build time mirrors server");
  assert(
    STATS[KIND.STEELWORKS].trains.includes(KIND.ANTI_TANK_GUN),
    "Gun Works should train Anti-Tank Guns after the unlock",
  );
  assert(
    !STATS[KIND.STEELWORKS].researches,
    "Gun Works should no longer expose advanced unlock research",
  );
  assert(
    !STATS[KIND.BARRACKS].trains.includes(KIND.ANTI_TANK_GUN),
    "Barracks should no longer train Anti-Tank Guns",
  );
  assert(
    STATS[KIND.STEELWORKS].requires.includes(KIND.TRAINING_CENTRE),
    "Gun Works should require Training Centre tech in the command card",
  );
  assert(
    STATS[KIND.RESEARCH_COMPLEX].label === "R&D Complex" &&
      STATS[KIND.RESEARCH_COMPLEX].footW === 3 &&
      STATS[KIND.RESEARCH_COMPLEX].footH === 3,
    "R&D Complex should be a 3x3 command-card building",
  );
  assert(
    STATS[KIND.RESEARCH_COMPLEX].cost.steel === 100 &&
      STATS[KIND.RESEARCH_COMPLEX].cost.oil === 100 &&
      STATS[KIND.RESEARCH_COMPLEX].buildTicks === 450,
    "R&D Complex cost and build time mirror server",
  );
  assertDeepEqual(
    STATS[KIND.RESEARCH_COMPLEX].researches,
    [
      UPGRADE.ANTI_TANK_GUN_UNLOCK,
      UPGRADE.BALLISTIC_TABLES,
      UPGRADE.TANK_UNLOCK,
      UPGRADE.MORTAR_AUTOCAST,
      UPGRADE.SMOKE_PLUS,
      UPGRADE.ARTILLERY_UNLOCK,
    ],
    "R&D Complex should expose Medium Guns, Artillery Fire Control, Tank, Mortar Autocast, Smoke Plus, and Heavy Guns research",
  );
  assert(!ABILITIES[ABILITY.CHARGE], "client no longer exposes Rifleman Charge as a command-card ability");
  assert(
    STATS[KIND.TRAINING_CENTRE].researches.includes(UPGRADE.METHAMPHETAMINES),
    "Training Centre should expose Methamphetamines research",
  );
  assert(
    STATS[KIND.TRAINING_CENTRE].researches.includes(UPGRADE.ENTRENCHMENT),
    "Training Centre should expose Entrenchment research",
  );
  assert(
    UPGRADES[UPGRADE.METHAMPHETAMINES].cost.steel === 100 &&
      UPGRADES[UPGRADE.METHAMPHETAMINES].cost.oil === 100 &&
      UPGRADES[UPGRADE.METHAMPHETAMINES].researchTicks === 600,
    "Methamphetamines research cost and time mirror server",
  );
  assert(
    UPGRADES[UPGRADE.ENTRENCHMENT].cost.steel === 200 &&
      UPGRADES[UPGRADE.ENTRENCHMENT].cost.oil === 0 &&
      UPGRADES[UPGRADE.ENTRENCHMENT].researchTicks === ENTRENCHMENT_RESEARCH_TICKS &&
      ENTRENCHMENT_RESEARCH_TICKS === TICK_HZ * 30,
    "Entrenchment research cost and time mirror server",
  );
  assert(
    ENTRENCHMENT_DIG_IN_TICKS === TICK_HZ * 3 &&
      ENTRENCHMENT_RANGE_BONUS_TILES === 1 &&
      ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION === 0.50 &&
      ENTRENCHMENT_AREA_DAMAGE_REDUCTION === 0.25 &&
      ENTRENCHMENT_TRENCH_RADIUS_TILES === 0.375,
    "Entrenchment constants mirror server",
  );
  assert(
    UPGRADES[UPGRADE.MORTAR_AUTOCAST].cost.steel === 150 &&
      UPGRADES[UPGRADE.MORTAR_AUTOCAST].cost.oil === 150 &&
      UPGRADES[UPGRADE.MORTAR_AUTOCAST].researchTicks === 600,
    "Mortar Autocast research cost and time mirror server",
  );
  assert(
    UPGRADES[UPGRADE.SMOKE_PLUS].cost.steel === 150 &&
      UPGRADES[UPGRADE.SMOKE_PLUS].cost.oil === 150 &&
      UPGRADES[UPGRADE.SMOKE_PLUS].researchTicks === SMOKE_PLUS_RESEARCH_TICKS &&
      SMOKE_PLUS_RESEARCH_TICKS === TICK_HZ * 20,
    "Smoke Plus research cost and time mirror server",
  );
  assert(
    ABILITIES[ABILITY.SMOKE].upgradedRadiusTiles === SMOKE_PLUS_CLOUD_RADIUS_TILES &&
      ABILITIES[ABILITY.SMOKE].upgradedDurationTicks === SMOKE_PLUS_CLOUD_DURATION_TICKS &&
      SMOKE_PLUS_CLOUD_RADIUS_TILES === SMOKE_CLOUD_RADIUS_TILES * 1.5 &&
      SMOKE_PLUS_CLOUD_DURATION_TICKS === SMOKE_CLOUD_DURATION_TICKS * 2,
    "Smoke Plus ability effect values mirror the base Smoke cloud upgrade",
  );
  assert(
    UPGRADES[UPGRADE.ANTI_TANK_GUN_UNLOCK].label === "Medium Guns" &&
      UPGRADES[UPGRADE.ANTI_TANK_GUN_UNLOCK].cost.steel === 100 &&
      UPGRADES[UPGRADE.ANTI_TANK_GUN_UNLOCK].cost.oil === 50 &&
      UPGRADES[UPGRADE.ANTI_TANK_GUN_UNLOCK].researchTicks === ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS &&
      ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS === TICK_HZ * 10,
    "Medium Guns research cost and time mirror server",
  );
  assert(
    UPGRADES[UPGRADE.ARTILLERY_UNLOCK].label === "Heavy Guns" &&
      UPGRADES[UPGRADE.ARTILLERY_UNLOCK].cost.steel === 200 &&
      UPGRADES[UPGRADE.ARTILLERY_UNLOCK].cost.oil === 100 &&
      UPGRADES[UPGRADE.ARTILLERY_UNLOCK].researchTicks === ARTILLERY_UNLOCK_RESEARCH_TICKS &&
      ARTILLERY_UNLOCK_RESEARCH_TICKS === TICK_HZ * 25,
    "Heavy Guns research cost and time mirror server",
  );
  assert(
    UPGRADES[UPGRADE.ARTILLERY_UNLOCK].requiresUpgrade === UPGRADE.ANTI_TANK_GUN_UNLOCK &&
      UPGRADES[UPGRADE.ARTILLERY_UNLOCK].requiresText === "Requires Medium Guns" &&
      UPGRADES[UPGRADE.ARTILLERY_UNLOCK].replacesUpgrade === UPGRADE.ANTI_TANK_GUN_UNLOCK,
    "Heavy Guns research replaces Medium Guns and keeps its prerequisite explicit",
  );
  assert(
    UPGRADES[UPGRADE.BALLISTIC_TABLES].cost.steel === 300 &&
      UPGRADES[UPGRADE.BALLISTIC_TABLES].cost.oil === 200 &&
      UPGRADES[UPGRADE.BALLISTIC_TABLES].researchTicks === 1200,
    "Artillery Fire Control research cost and time mirror server",
  );
  assert(
    UPGRADES[UPGRADE.BALLISTIC_TABLES].label === "Artillery Fire Control" &&
      UPGRADES[UPGRADE.BALLISTIC_TABLES].icon === "AFC",
    "Artillery Fire Control research uses the renamed client label and icon",
  );
  assert(
    UPGRADES[UPGRADE.BALLISTIC_TABLES].requiresUpgrade === UPGRADE.ARTILLERY_UNLOCK &&
      UPGRADES[UPGRADE.BALLISTIC_TABLES].requiresText === "Requires Heavy Guns",
    "Artillery Fire Control research should mirror its Heavy Guns prerequisite",
  );
  assert(
    STATS[KIND.ANTI_TANK_GUN].upgradeRequiresText === "Requires research in R&D Complex",
    "Anti-Tank Gun training should explain the R&D Complex research requirement",
  );
  assert(
    STATS[KIND.TANK].upgradeRequiresText === "Requires research in R&D Complex",
    "Tank training should explain the R&D Complex research requirement",
  );
  assert(
    STATS[KIND.TANK_TRAP].label === "Tank Trap" &&
      STATS[KIND.TANK_TRAP].footW === 1 &&
      STATS[KIND.TANK_TRAP].footH === 1 &&
      STATS[KIND.TANK_TRAP].sight === 0 &&
      STATS[KIND.TANK_TRAP].cost.steel === 30 &&
      STATS[KIND.TANK_TRAP].cost.oil === 0 &&
      STATS[KIND.TANK_TRAP].buildTicks === TICK_HZ * 10 &&
      STATS[KIND.TANK_TRAP].requires === KIND.TRAINING_CENTRE,
    "Tank Trap active metadata mirrors server rules",
  );
  assert(
    WORKER_BUILDABLE.includes(KIND.TANK_TRAP),
    "Tank Trap is available in the worker build menu",
  );
  assert(
    !WORKER_BUILDABLE.includes(KIND.DEPOT) &&
      WORKER_BUILD_CARD_SLOTS[1] == null &&
      WORKER_BUILD_CARD_SLOTS.filter(Boolean).join(",") === WORKER_BUILDABLE.join(","),
    "Supply Depot is unavailable while its worker-card W slot remains empty",
  );
  const playerId = 1;
  const underConstructionTrainingCentre = [
    { owner: playerId, kind: KIND.CITY_CENTRE, buildProgress: null },
    { owner: playerId, kind: KIND.TRAINING_CENTRE, buildProgress: 0.5 },
  ];
  assert(
    !playerHasCompletedKind(underConstructionTrainingCentre, playerId, KIND.TRAINING_CENTRE),
    "Vehicle Works should not unlock while the Training Centre is still under construction",
  );
  const underConstructionBarracks = [
    { owner: playerId, kind: KIND.CITY_CENTRE, buildProgress: null },
    { owner: playerId, kind: KIND.BARRACKS, buildProgress: 0.5 },
  ];
  assert(
    !playerHasCompletedKind(underConstructionBarracks, playerId, KIND.BARRACKS),
    "Training Centre should not unlock while the Barracks is still under construction",
  );
  const completedTrainingCentre = [
    { owner: playerId, kind: KIND.CITY_CENTRE, buildProgress: null },
    { owner: playerId, kind: KIND.TRAINING_CENTRE, buildProgress: null },
  ];
  assert(
    playerHasCompletedKind(completedTrainingCentre, playerId, KIND.TRAINING_CENTRE),
    "Vehicle Works should unlock once the Training Centre is complete",
  );
  assert(formatTankOilUsed(0.04) === "0.0", "tank oil panel rounds tiny values to tenths");
  assert(formatTankOilUsed(9.94) === "9.9", "tank oil panel keeps tenths below ten oil");
  assert(formatTankOilUsed(10.4) === "10", "tank oil panel rounds whole values above ten oil");
  assert(formatTankOilUsed(-2) === "0.0", "tank oil panel clamps negative values");
  assert(formatTankOilUsed(Number.NaN) === "0.0", "tank oil panel tolerates missing oilUsed");
  const genericCooldownTicks = TICK_HZ * 5;
  const groupedNearlySameCooldowns = groupCooldownClocks([150, 149, 146], genericCooldownTicks);
  assert(groupedNearlySameCooldowns.length === 1, "nearby cooldowns share one clock arm");
  assert(groupedNearlySameCooldowns[0].count === 3, "clock grouping keeps the grouped unit count");
  const groupedDistinctCooldowns = groupCooldownClocks([150, 120, 60], genericCooldownTicks);
  assert(groupedDistinctCooldowns.length === 3, "visibly different cooldowns get separate clock arms");
  const groupedIgnoringReady = groupCooldownClocks([0, 0, 30, 31], genericCooldownTicks);
  assert(groupedIgnoringReady.length === 1 && groupedIgnoringReady[0].count === 2, "ready entries do not create cooldown clocks");

  const trained = [];
  let selectedProductionBuildings = [
    { id: 20, owner: playerId, kind: KIND.BARRACKS },
    { id: 22, owner: playerId, kind: KIND.BARRACKS, buildProgress: 0.5 },
    { id: 21, owner: playerId, kind: KIND.BARRACKS },
    { id: 30, owner: playerId, kind: KIND.FACTORY },
  ];
  const hud = Object.create(HUD.prototype);
  hud.state = {
    playerId,
    selectedEntities: () => selectedProductionBuildings,
  };
  hud.commandIssuer = {
    command: (command) => trained.push(command),
  };
  hud._trainRoundRobin = new Map();
  hud._cancelRoundRobin = new Map();

  hud._issueTrain(KIND.RIFLEMAN);
  hud._issueTrain(KIND.MACHINE_GUNNER);
  hud._issueTrain(KIND.RIFLEMAN);
  hud._issueTrain(KIND.SCOUT_CAR);
  assert(
    trained.map((command) => command.building).join(",") === "20,21,20,30",
    "selected production buildings should receive train commands round-robin by compatible producer set",
  );

  selectedProductionBuildings = [
    { id: 21, owner: playerId, kind: KIND.BARRACKS },
    { id: 20, owner: playerId, kind: KIND.BARRACKS },
  ];
  hud._issueTrain(KIND.RIFLEMAN);
  assert(
    trained[4].building === 21,
    "changing selected producer order should start the new round-robin set at its first building",
  );

  selectedProductionBuildings = [
    { id: 20, owner: playerId, kind: KIND.BARRACKS, prodQueue: 1 },
    { id: 21, owner: playerId, kind: KIND.BARRACKS, prodQueue: 2 },
    { id: 30, owner: playerId, kind: KIND.FACTORY, prodQueue: 1 },
  ];
  hud._issueCancelProduction(KIND.BARRACKS);
  hud._issueCancelProduction(KIND.BARRACKS);
  hud._issueCancelProduction(KIND.BARRACKS);
  assert(
    trained.slice(5).map((command) => command.building).join(",") === "21,20,21",
    "selected producing buildings should receive cancel commands reverse round-robin by producer kind",
  );

  const priorDocument = globalThis.document;
  const priorMouseEvent = globalThis.MouseEvent;
  const renderedButtons = [];
  function fakeElement(tagName) {
    const listeners = new Map();
    return {
      tagName: tagName.toUpperCase(),
      children: [],
      className: "",
      dataset: {},
      disabled: false,
      innerHTML: "",
      style: {
        values: {},
        setProperty(name, value) {
          this.values[name] = value;
        },
      },
      appendChild(child) {
        if (child?.nodeType === "fragment") this.children.push(...child.children);
        else this.children.push(child);
      },
      querySelector(selector) {
        const abilityMatch = selector.match(/^button\[data-ability="([^"]+)"\]$/);
        if (abilityMatch) {
          return this.children.find((child) => child.dataset?.ability === abilityMatch[1]) || null;
        }
        return null;
      },
      querySelectorAll() {
        return [];
      },
      addEventListener(type, listener) {
        listeners.set(type, listener);
      },
      dispatchEvent(ev) {
        listeners.get(ev.type)?.(ev);
        return true;
      },
      click(ev = {}) {
        listeners.get("click")?.({
          type: "click",
          preventDefault() {},
          shiftKey: !!ev.shiftKey,
        });
      },
    };
  }
  function renderCommandCard(hud) {
    if (!hud.elCommand) hud.elCommand = fakeElement("div");
    if (!hud.clientIntent) hud.clientIntent = new ClientIntent();
    hud._renderCommandCard();
    return hud.elCommand;
  }
  try {
    globalThis.document = {
      createDocumentFragment() {
        return {
          nodeType: "fragment",
          children: [],
          appendChild(child) {
            this.children.push(child);
          },
        };
      },
      createElement(tagName) {
        const el = fakeElement(tagName);
        if (tagName === "button") renderedButtons.push(el);
        return el;
      },
    };
    globalThis.MouseEvent = class {
      constructor(type, init = {}) {
        this.type = type;
        this.altKey = !!init.altKey;
        this.ctrlKey = !!init.ctrlKey;
        this.metaKey = !!init.metaKey;
        this.shiftKey = !!init.shiftKey;
        this.bubbles = !!init.bubbles;
        this.cancelable = !!init.cancelable;
      }
      preventDefault() {}
    };

    const sent = [];
    const selectedTrainingCentre = {
      id: 77,
      owner: playerId,
      kind: KIND.TRAINING_CENTRE,
      buildProgress: null,
    };
    const researchHud = Object.create(HUD.prototype);
    researchHud.state = {
      playerId,
      resources: { steel: 100, oil: 100 },
      upgrades: [],
      commandTarget: null,
      selectedEntities: () => [selectedTrainingCentre],
      entitiesInterpolated: () => [selectedTrainingCentre],
      endCommandTarget() {
        this.commandTarget = null;
      },
    };
    researchHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    researchHud._cardSig = null;
    researchHud._resourceIcons = {};

    renderCommandCard(researchHud);
    const researchButton = renderedButtons.find((button) => button.innerHTML.includes("Methamphetamines"));
    assert(researchButton && !researchButton.disabled, "Methamphetamines command-card button renders enabled");
    assert(researchButton.dataset.hotkey === "Q", "Methamphetamines command-card button uses Q as its hotkey");
    assert(researchButton.innerHTML.includes("Research time"), "Methamphetamines tooltip includes research time");
    const entrenchmentButton = renderedButtons.find((button) => button.innerHTML.includes("Entrenchment"));
    assert(entrenchmentButton && !entrenchmentButton.disabled, "Entrenchment command-card button renders enabled");
    assert(entrenchmentButton.dataset.hotkey === "W", "Entrenchment command-card button uses W as its hotkey");
    researchButton.click({ shiftKey: true });
    assert(
      sent.length === 1 &&
        sent[0].c === "research" &&
        sent[0].building === 77 &&
        sent[0].upgrade === UPGRADE.METHAMPHETAMINES,
      "Clicking Methamphetamines should send a research command",
    );
    entrenchmentButton.click({ shiftKey: false });
    assert(
      sent.length === 2 &&
        sent[1].c === "research" &&
        sent[1].building === 77 &&
        sent[1].upgrade === UPGRADE.ENTRENCHMENT,
      "Clicking Entrenchment should send a research command",
    );

    const mortarButtonsBefore = renderedButtons.length;
    const selectedMortar = {
      id: 501,
      owner: playerId,
      kind: KIND.MORTAR_TEAM,
      abilities: [{
        ability: ABILITY.MORTAR_FIRE,
        cooldownLeft: 30,
        autocastEnabled: true,
      }],
    };
    const mortarHud = Object.create(HUD.prototype);
    mortarHud.state = {
      playerId,
      resources: { steel: 100, oil: 100 },
      commandTarget: null,
      selectedEntities: () => [selectedMortar],
      entitiesInterpolated: () => [selectedMortar],
      beginCommandTarget(target) {
        this.commandTarget = target;
      },
      endCommandTarget() {
        this.commandTarget = null;
      },
    };
    mortarHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    mortarHud.audio = null;
    mortarHud._cardSig = null;
    renderCommandCard(mortarHud);
    const mortarButtonCount = renderedButtons.length;
    assert(
      mortarButtonCount > mortarButtonsBefore,
      "selected Mortar Team should render an ability command button",
    );
    const coolingMortarButton = renderedButtons.find((button) => button.innerHTML.includes("Fire"));
    assert(coolingMortarButton?.dataset.contextAction === "true", "Mortar Fire button exposes its context hotkey action");
    assert(!coolingMortarButton?.dataset.shiftContextAction, "Shift keeps Mortar Fire on its queued manual-fire action");
    assert(!coolingMortarButton?.disabled, "cooling-down Mortar Fire remains armable for queued manual fire");
    const coolingMortarCard = buildCommandCardDescriptors({
      playerId,
      resources: { steel: 100, oil: 100 },
      selection: [selectedMortar],
      groupCooldownClocks,
    });
    const coolingMortarDescriptor = coolingMortarCard.slots.find((slot) => slot?.ability === ABILITY.MORTAR_FIRE);
    assert(coolingMortarDescriptor?.enabled, "cooling-down Mortar Fire descriptor remains enabled for queued manual fire");
    assertDeepEqual(
      coolingMortarDescriptor?.intent?.readyIds,
      [selectedMortar.id],
      "cooling-down Mortar Fire descriptor targets the waitable mortar carrier",
    );
    globalThis.document.getElementById = (id) => {
      assert(id === "command-card", "Mortar autocast hotkey should query the command card");
      return {
        querySelectorAll(selector) {
          assert(selector === "button[data-hotkey]", "Mortar autocast hotkey should query hotkey buttons");
          return [coolingMortarButton];
        },
      };
    };
    const disableAutocastEv = {
      code: `Key${coolingMortarButton.dataset.hotkey}`,
      altKey: true,
      ctrlKey: false,
      metaKey: false,
      shiftKey: false,
      repeat: false,
      preventDefault() { this.prevented = true; },
    };
    const input = Object.create(Input.prototype);
    input.state = mortarHud.state;
    const disableAutocastResult = input._activateCommandHotkey(disableAutocastEv);
    const disableAutocastCommand = sent[sent.length - 1];
    assert(disableAutocastResult?.contextAction === true, "Alt+Mortar Fire hotkey should take the context-action path");
    assert(disableAutocastEv.prevented, "Alt+Mortar Fire hotkey should prevent browser handling");
    assert(
      disableAutocastCommand?.c === "setAutocast" &&
        disableAutocastCommand.ability === ABILITY.MORTAR_FIRE &&
        disableAutocastCommand.enabled === false &&
        disableAutocastCommand.units[0] === selectedMortar.id,
      "Alt+Mortar Fire hotkey should disable selected mortar autocast even while manual fire is cooling down",
    );

    selectedMortar.abilities[0].cooldownLeft = 29;
    renderCommandCard(mortarHud);
    assert(
      renderedButtons.length === mortarButtonCount,
      "Mortar Fire cooldown ticks should update in place without rebuilding the command button",
    );
    selectedMortar.abilities[0].cooldownLeft = 0;
    selectedMortar.abilities[0].autocastEnabled = false;
    mortarHud._cardSig = null;
    renderedButtons.length = 0;
    renderCommandCard(mortarHud);
    const readyMortarButton = renderedButtons.find((button) => button.innerHTML.includes("Fire"));
    globalThis.document.getElementById = (id) => {
      assert(id === "command-card", "Mortar autocast enable hotkey should query the command card");
      return {
        querySelectorAll(selector) {
          assert(selector === "button[data-hotkey]", "Mortar autocast enable hotkey should query hotkey buttons");
          return [readyMortarButton];
        },
      };
    };
    input._activateCommandHotkey({
      code: `Key${readyMortarButton.dataset.hotkey}`,
      altKey: true,
      ctrlKey: false,
      metaKey: false,
      shiftKey: false,
      repeat: false,
      preventDefault() {},
    });
    const enableAutocastCommand = sent[sent.length - 1];
    assert(
      enableAutocastCommand?.c === "setAutocast" &&
        enableAutocastCommand.ability === ABILITY.MORTAR_FIRE &&
        enableAutocastCommand.enabled === true &&
        enableAutocastCommand.units[0] === selectedMortar.id,
      "Alt+Mortar Fire hotkey should enable selected mortar autocast when it is currently off",
    );

    renderedButtons.length = 0;
    const selectedCommandCar = {
      id: 601,
      owner: playerId,
      kind: KIND.COMMAND_CAR,
      abilities: [{
        ability: ABILITY.BREAKTHROUGH,
        cooldownLeft: 0,
      }],
    };
    const commandCarHud = Object.create(HUD.prototype);
    commandCarHud.state = {
      playerId,
      resources: { steel: 100, oil: 100 },
      map: { tileSize: 32 },
      commandTarget: null,
      selectedEntities: () => [selectedCommandCar],
      entitiesInterpolated: () => [selectedCommandCar],
      updateAbilityTargetPreview(preview) {
        this.abilityTargetPreview = preview;
      },
      endCommandTarget() {
        this.commandTarget = null;
      },
    };
    commandCarHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    commandCarHud.audio = null;
    commandCarHud._cardSig = null;
    renderCommandCard(commandCarHud);
    const breakthroughButton = renderedButtons.find((button) => button.innerHTML.includes("Breakthrough"));
    assert(breakthroughButton?.dataset.hotkey === "E", "Breakthrough should use the E command-card slot");
    breakthroughButton.click({ shiftKey: true });
    const breakthroughCommand = sent[sent.length - 1];
    assert(
      breakthroughCommand?.c === "useAbility" &&
        breakthroughCommand.ability === ABILITY.BREAKTHROUGH &&
        breakthroughCommand.units[0] === selectedCommandCar.id &&
        breakthroughCommand.queued === true &&
        !("x" in breakthroughCommand) &&
        !("y" in breakthroughCommand),
      "Clicking Breakthrough should issue a queued self-target ability command without coordinates",
    );

    const leftCommandCar = {
      ...selectedCommandCar,
      id: 602,
      x: 0,
      y: 0,
    };
    const centralCommandCar = {
      ...selectedCommandCar,
      id: 603,
      x: 9,
      y: 0,
    };
    const rightCommandCar = {
      ...selectedCommandCar,
      id: 604,
      x: 30,
      y: 0,
    };
    const coolingDownCommandCar = {
      ...selectedCommandCar,
      id: 605,
      x: 10,
      y: 0,
      abilities: [{
        ability: ABILITY.BREAKTHROUGH,
        cooldownLeft: 5,
      }],
    };
    commandCarHud.state.selectedEntities = () => [
      leftCommandCar,
      centralCommandCar,
      rightCommandCar,
      coolingDownCommandCar,
    ];
    commandCarHud.state.entitiesInterpolated = commandCarHud.state.selectedEntities;
    commandCarHud._cardSig = null;
    renderedButtons.length = 0;
    renderCommandCard(commandCarHud);
    const multiBreakthroughButton = renderedButtons.find((button) => button.innerHTML.includes("Breakthrough"));
    multiBreakthroughButton.dispatchEvent({ type: "mouseenter" });
    assert(
      commandCarHud.clientIntent.abilityTargetPreview?.areaOrigins.length === 1 &&
        commandCarHud.clientIntent.abilityTargetPreview.areaOrigins[0].id === centralCommandCar.id,
      "Breakthrough hover preview should show only the Command Car that would activate",
    );
    multiBreakthroughButton.click({});
    const multiBreakthroughCommand = sent[sent.length - 1];
    assert(
      multiBreakthroughCommand.units.length === 1 &&
        multiBreakthroughCommand.units[0] === centralCommandCar.id,
      "Breakthrough should issue from the most central ready Command Car only",
    );

    globalThis.document.getElementById = (id) => {
      assert(id === "command-card", "Methamphetamines hotkey should query the command card");
      return {
        querySelectorAll(selector) {
          assert(selector === "button[data-hotkey]", "Methamphetamines hotkey should query hotkey buttons");
          return [researchButton];
        },
      };
    };
    input.state = researchHud.state;
    const hotkeyEv = {
      code: "KeyQ",
      shiftKey: false,
      repeat: false,
      preventDefault() {},
    };
    const hotkeyResult = input._activateCommandHotkey(hotkeyEv);
    assert(hotkeyResult?.handled === true, "Methamphetamines hotkey should activate the command-card button");
    const hotkeyCommand = sent[sent.length - 1];
    assert(
      hotkeyCommand?.c === "research" &&
        hotkeyCommand.building === 77 &&
        hotkeyCommand.upgrade === UPGRADE.METHAMPHETAMINES,
      "Methamphetamines hotkey should send a research command",
    );

    renderedButtons.length = 0;
    const selectedFactory = {
      id: 78,
      owner: playerId,
      kind: KIND.FACTORY,
      buildProgress: null,
    };
    const factoryHud = Object.create(HUD.prototype);
    factoryHud.state = {
      playerId,
      resources: { steel: 300, oil: 150 },
      upgrades: [],
      selectedEntities: () => [selectedFactory],
      entitiesInterpolated: () => [selectedFactory],
    };
    factoryHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    factoryHud._cardSig = null;
    factoryHud._trainRoundRobin = new Map();
    factoryHud._cancelRoundRobin = new Map();
    factoryHud._resourceIcons = {};
    renderCommandCard(factoryHud);
    const scoutCarButton = renderedButtons.find((button) => button.innerHTML.includes("Scout Car"));
    const tankButton = renderedButtons.find((button) => button.innerHTML.includes("Tank"));
    const commandCarButton = renderedButtons.find((button) => button.innerHTML.includes("Command Car"));
    const tankResearchButton = renderedButtons.find((button) => button.innerHTML.includes("TK+"));
    assert(scoutCarButton?.dataset.hotkey === "Q", "Scout Car training should keep the Q slot");
    assert(tankButton?.dataset.hotkey === "W", "Tank training should occupy the top-middle W slot");
    assert(commandCarButton?.dataset.hotkey === "E", "Command Car training should occupy the top-right E slot");
    assert(
      commandCarButton && !commandCarButton.disabled && commandCarButton.className.includes("primary-disabled"),
      "Command Car training should keep its primary action disabled while allowing auto-build allocation before unlock",
    );
    assert(
      commandCarButton?.dataset.contextAction === "true",
      "production buttons expose their allocation context action to the hotkey layer",
    );
    assert(
      commandCarButton?.dataset.shiftContextAction === "true",
      "production buttons expose Shift as their decrement hotkey modifier",
    );
    globalThis.document.getElementById = () => ({
      querySelectorAll() {
        return [commandCarButton];
      },
    });
    input.state = factoryHud.state;
    const addAutoBuildEv = {
      code: "KeyE",
      altKey: true,
      ctrlKey: false,
      metaKey: false,
      shiftKey: false,
      repeat: false,
      preventDefault() { this.prevented = true; },
    };
    const addAutoBuildResult = input._activateCommandHotkey(addAutoBuildEv);
    const addAutoBuildCommand = sent[sent.length - 1];
    assert(
      addAutoBuildResult?.contextAction === true && addAutoBuildEv.prevented &&
        addAutoBuildCommand?.c === "adjustProductionRepeat" &&
        addAutoBuildCommand.delta === 1 && addAutoBuildCommand.buildings[0] === selectedFactory.id,
      "Alt+production hotkeys dispatch one signed addition through the context-action path",
    );
    const removeAutoBuildEv = {
      code: "KeyE",
      altKey: false,
      ctrlKey: false,
      metaKey: false,
      shiftKey: true,
      repeat: false,
      preventDefault() { this.prevented = true; },
    };
    const removeAutoBuildResult = input._activateCommandHotkey(removeAutoBuildEv);
    const removeAutoBuildCommand = sent[sent.length - 1];
    assert(
      removeAutoBuildResult?.contextAction === true && removeAutoBuildEv.prevented &&
        removeAutoBuildCommand?.c === "adjustProductionRepeat" &&
        removeAutoBuildCommand.delta === -1 && removeAutoBuildCommand.buildings[0] === selectedFactory.id,
      "Shift+production hotkeys dispatch one signed removal through the context-action path",
    );
    assert(!tankResearchButton, "Tank Production research should move out of Vehicle Works");

    renderedButtons.length = 0;
    factoryHud.state.upgrades = [UPGRADE.TANK_UNLOCK];
    factoryHud._cardSig = null;
    renderCommandCard(factoryHud);
    assert(
      !renderedButtons.some((button) => button.innerHTML.includes("TK+")),
      "completed Tank Production research should disappear from the command card",
    );
    const unlockedCommandCarButton = renderedButtons.find((button) => button.innerHTML.includes("Command Car"));
    assert(
      unlockedCommandCarButton && !unlockedCommandCarButton.disabled,
      "Tank Production should enable Command Car training",
    );

    renderedButtons.length = 0;
    const selectedGunWorks = {
      id: 79,
      owner: playerId,
      kind: KIND.STEELWORKS,
      buildProgress: null,
    };
    const gunWorksHud = Object.create(HUD.prototype);
    gunWorksHud.state = {
      playerId,
      resources: { steel: 300, oil: 200 },
      upgrades: [],
      selectedEntities: () => [selectedGunWorks],
      entitiesInterpolated: () => [selectedGunWorks],
    };
    gunWorksHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    gunWorksHud._cardSig = null;
    gunWorksHud._trainRoundRobin = new Map();
    gunWorksHud._cancelRoundRobin = new Map();
    gunWorksHud._resourceIcons = {};
    renderCommandCard(gunWorksHud);
    const mortarButton = renderedButtons.find((button) => button.innerHTML.includes("Mortar Team"));
    const antiTankGunButton = renderedButtons.find((button) => button.innerHTML.includes("Anti-Tank Gun"));
    const artilleryButton = renderedButtons.find((button) => button.innerHTML.includes("Artillery"));
    const heavyGunsResearchButton = renderedButtons.find((button) => button.innerHTML.includes("HG+"));
    const artilleryResearchButton = renderedButtons.find((button) => button.innerHTML.includes("AR+"));
    assert(mortarButton?.dataset.hotkey === "Q", "Mortar Team training should occupy the top-left Q slot");
    assert(
      mortarButton?.innerHTML.includes("Indirect fire, extremely inaccurate without vision. Upgrade auto cast in R&D."),
      "Mortar Team tooltip should explain indirect fire inaccuracy and R&D autocast",
    );
    assert(antiTankGunButton?.dataset.hotkey === "W", "Anti-Tank Gun training should occupy the top-middle W slot");
    assert(artilleryButton?.dataset.hotkey === "E", "Artillery training should occupy the top-right E slot");
    assert(!heavyGunsResearchButton, "Heavy Guns research should stay out of Gun Works");
    assert(!artilleryResearchButton, "Separate Artillery research should not appear in Gun Works");

    renderedButtons.length = 0;
    const selectedResearchComplex = {
      id: 80,
      owner: playerId,
      kind: KIND.RESEARCH_COMPLEX,
      buildProgress: null,
    };
    const rdHud = Object.create(HUD.prototype);
    rdHud.state = {
      playerId,
      resources: { steel: 500, oil: 500 },
      upgrades: [],
      selectedEntities: () => [selectedResearchComplex],
      entitiesInterpolated: () => [selectedResearchComplex],
    };
    rdHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    rdHud._cardSig = null;
    rdHud._trainRoundRobin = new Map();
    rdHud._cancelRoundRobin = new Map();
    rdHud._resourceIcons = {};
    renderCommandCard(rdHud);
    const rdMediumGunsResearchButton = renderedButtons.find((button) => button.innerHTML.includes("MD+"));
    const rdHeavyGunsResearchButton = renderedButtons.find((button) => button.innerHTML.includes("HG+"));
    const rdArtilleryResearchButton = renderedButtons.find((button) => button.innerHTML.includes("AR+"));
    const rdArtilleryFireControlButton = renderedButtons.find((button) => button.innerHTML.includes("AFC"));
    const rdTankResearchButton = renderedButtons.find((button) => button.innerHTML.includes("TK+"));
    const rdMortarAutocastButton = renderedButtons.find((button) => button.innerHTML.includes("MT+"));
    const rdSmokePlusButton = renderedButtons.find((button) => button.innerHTML.includes("SMK+"));
    assert(rdArtilleryFireControlButton?.dataset.hotkey === "W", "Artillery Fire Control research should appear in R&D Complex");
    assert(rdMediumGunsResearchButton?.dataset.hotkey === "Q", "Medium Guns research should appear in R&D Complex");
    assert(!rdHeavyGunsResearchButton, "Heavy Guns research should be hidden before Medium Guns");
    assert(rdTankResearchButton?.dataset.hotkey === "E", "Tank Production research should appear in R&D Complex");
    assert(rdMortarAutocastButton?.dataset.hotkey === "A", "Mortar Autocast research should appear in R&D Complex");
    assert(rdSmokePlusButton?.dataset.hotkey === "S", "Smoke Plus research should appear in R&D Complex");
    assert(!renderedButtons.some((button) => button.innerHTML.includes("CC+")), "R&D Complex should not expose Command Car research");
    assert(rdArtilleryFireControlButton?.disabled, "Artillery Fire Control research should be disabled before Heavy Guns");
    assert(rdArtilleryFireControlButton?.title === "Requires Heavy Guns", "Artillery Fire Control research should name Heavy Guns prerequisite");
    assert(!rdArtilleryResearchButton, "R&D Complex should not expose separate Artillery research");

    renderedButtons.length = 0;
    rdHud.state.upgrades = [UPGRADE.ANTI_TANK_GUN_UNLOCK];
    rdHud._cardSig = null;
    renderCommandCard(rdHud);
    const unlockedHeavyGunsResearchButton = renderedButtons.find((button) => button.innerHTML.includes("HG+"));
    const mediumUnlockedArtilleryFireControlButton = renderedButtons.find((button) => button.innerHTML.includes("AFC"));
    assert(unlockedHeavyGunsResearchButton?.dataset.hotkey === "Q", "Heavy Guns should replace Medium Guns in the Q slot");
    assert(unlockedHeavyGunsResearchButton && !unlockedHeavyGunsResearchButton.disabled, "Heavy Guns should enable after Medium Guns");
    assert(mediumUnlockedArtilleryFireControlButton?.disabled, "Artillery Fire Control should still require Heavy Guns after Medium Guns");

    renderedButtons.length = 0;
    rdHud.state.upgrades = [UPGRADE.ANTI_TANK_GUN_UNLOCK, UPGRADE.ARTILLERY_UNLOCK];
    rdHud._cardSig = null;
    renderCommandCard(rdHud);
    const unlockedArtilleryFireControlButton = renderedButtons.find((button) => button.innerHTML.includes("AFC"));
    assert(unlockedArtilleryFireControlButton && !unlockedArtilleryFireControlButton.disabled, "Artillery Fire Control should enable after Heavy Guns");

    renderedButtons.length = 0;
    const playedNotices = [];
    let placements = 0;
    const selectedWorker = { id: 90, owner: playerId, kind: KIND.WORKER };
    const completedCityCentre = { id: 91, owner: playerId, kind: KIND.CITY_CENTRE, buildProgress: null };
    const shortResourceHud = Object.create(HUD.prototype);
    shortResourceHud.state = {
      playerId,
      resources: { steel: 100, oil: 0 },
      selectedEntities: () => [selectedWorker],
      entitiesInterpolated: () => [selectedWorker, completedCityCentre],
      beginPlacement() {
        placements += 1;
      },
    };
    shortResourceHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    shortResourceHud.audio = {
      play(id) {
        playedNotices.push(id);
      },
    };
    shortResourceHud._cardSig = null;
    shortResourceHud._resourceIcons = {};

    shortResourceHud.clientIntent = new ClientIntent();
    shortResourceHud.clientIntent.openWorkerBuildMenu();
    renderCommandCard(shortResourceHud);
    const barracksButton = renderedButtons.find((button) => button.innerHTML.includes("Barracks"));
    const factoryButton = renderedButtons.find((button) => button.innerHTML.includes("Vehicle Works"));
    assert(barracksButton && !barracksButton.disabled, "unlocked unaffordable build button stays clickable");
    assert(
      barracksButton.className.includes("unaffordable"),
      "unlocked unaffordable build button gets the intermediate visual class",
    );
    assert(factoryButton?.disabled, "tech-locked build button stays hard-disabled");

    barracksButton.click();
    assert(
      shortResourceHud.clientIntent.placement?.building === KIND.BARRACKS,
      "clicking an unaffordable build button enters placement",
    );
    assert(
      playedNotices.length === 0,
      "clicking an unaffordable build button creates intent instead of shortage feedback",
    );

    globalThis.document.getElementById = (id) => {
      assert(id === "command-card", "unaffordable build hotkey should query the command card");
      return {
        querySelectorAll(selector) {
          assert(selector === "button[data-hotkey]", "unaffordable build hotkey should query hotkey buttons");
          return [barracksButton];
        },
      };
    };
    input.state = shortResourceHud.state;
    input._activateCommandHotkey({
      code: `Key${barracksButton.dataset.hotkey}`,
      shiftKey: false,
      repeat: false,
      preventDefault() {},
    });
    assert(
      shortResourceHud.clientIntent.placement?.building === KIND.BARRACKS,
      "unaffordable build hotkey enters placement",
    );
    assert(playedNotices.length === 0, "unaffordable build hotkey does not play a shortage voice line");

    assert(
      shortResourceHud._missingResourceSoundId(
        { steel: 50, oil: 0 },
        { steel: 50, oil: 0, supplyUsed: 10, supplyCap: 10 },
        1,
      ) === "notice_supply",
      "train unavailable feedback should play the supply voice line when resources are available",
    );

    renderedButtons.length = 0;
    sent.length = 0;
    const selectedAntiTankGun = { id: 88, owner: playerId, kind: KIND.ANTI_TANK_GUN, setupState: SETUP.DEPLOYED };
    const selectedArtillery = { id: 89, owner: playerId, kind: KIND.ARTILLERY, setupState: SETUP.PACKED };
    const antiTankGunHud = Object.create(HUD.prototype);
    antiTankGunHud.state = {
      playerId,
      resources: { steel: 0, oil: 0 },
      commandTarget: null,
      selectedEntities: () => [selectedAntiTankGun, selectedArtillery],
      entitiesInterpolated: () => [selectedAntiTankGun, selectedArtillery],
      beginCommandTarget(kind) {
        this.commandTarget = kind;
      },
      endCommandTarget() {
        this.commandTarget = null;
      },
    };
    antiTankGunHud.commandIssuer = { issueCommand: (command) => sent.push(command) };
    antiTankGunHud._cardSig = null;

    renderCommandCard(antiTankGunHud);
    const setupButton = renderedButtons.find((button) => button.innerHTML.includes("Set Up"));
    const tearDownButton = renderedButtons.find((button) => button.innerHTML.includes("Tear Down"));
    assert(setupButton?.dataset.hotkey, "anti-tank gun Set Up button should keep its command-card hotkey");
    assert(!tearDownButton, "anti-tank gun Tear Down should not occupy a command-card slot");

    const setupCommands = [];
    const setupInput = Object.create(Input.prototype);
    setupInput.state = {
      playerId,
      selectedEntities: () => [selectedAntiTankGun, selectedArtillery],
    };
    setupInput.clientIntent = new ClientIntent();
    setupInput.clientIntent.beginCommandTarget("setupAntiTankGuns");
    setupInput.clientIntent.addCommandFeedback = () => {};
    setupInput.commandIssuer = { issueCommand: (command) => setupCommands.push(command) };
    setupInput._groundAtScreen = (x, y) => ({ x, y });
    setupInput._entityAtScreen = () => null;
    setupInput._selectedOwnUnitIds = () => [selectedAntiTankGun.id, selectedArtillery.id];
    setupInput._issueTargetedCommand({ x: 160, y: 192 }, { shiftKey: true });
    assert(
      setupCommands[0]?.c === "setupAntiTankGuns" &&
        setupCommands[0].units.includes(selectedAntiTankGun.id) &&
        setupCommands[0].units.includes(selectedArtillery.id) &&
        setupCommands[0].queued === true,
      "setupAntiTankGuns targeting includes selected artillery as setup-capable support weapons",
    );

    const movingAntiTankGun = {
      ...selectedAntiTankGun,
      x: 100,
      y: 120,
      orderPlan: [
        { kind: ORDER_STAGE.MOVE, x: 320, y: 192 },
        { kind: ORDER_STAGE.SETUP_ANTI_TANK_GUNS, x: 640, y: 192 },
      ],
    };
    const movingArtillery = {
      ...selectedArtillery,
      x: 140,
      y: 120,
      orderPlan: [
        { kind: ORDER_STAGE.ATTACK_MOVE, x: 352, y: 224 },
      ],
    };
    const stationaryAntiTankGun = {
      id: 90,
      owner: playerId,
      kind: KIND.ANTI_TANK_GUN,
      x: 180,
      y: 120,
    };
    const previewInput = Object.create(Input.prototype);
    previewInput.mouse = { x: 500, y: 300 };
    previewInput.state = {
      playerId,
      selectedEntities: () => [movingAntiTankGun, movingArtillery, stationaryAntiTankGun],
    };
    previewInput.clientIntent = new ClientIntent();
    previewInput.clientIntent.beginCommandTarget("setupAntiTankGuns");
    previewInput._groundAtScreen = (x, y) => ({ x, y });
    previewInput._refreshAntiTankGunSetupPreview();
    const unqueuedPreviewGuns = previewInput.clientIntent.antiTankGunSetupPreview?.guns || [];
    assert(
      unqueuedPreviewGuns[0]?.x === 100 && unqueuedPreviewGuns[0]?.y === 120,
      "unqueued support setup preview keeps the current gun position",
    );

    previewInput.camera = {
      projectionSnapshot: () => ({
        groundAtScreen: ({ x, y }) => ({ x: x + 96, y: y - 64 }),
      }),
    };
    previewInput._groundAtScreen = () => ({ x: 0, y: 0 });
    previewInput._shiftKeyDown = false;
    previewInput._refreshAntiTankGunSetupPreview();
    assert(
      previewInput.clientIntent.antiTankGunSetupPreview?.mouseX === 596 &&
        previewInput.clientIntent.antiTankGunSetupPreview?.mouseY === 236,
      "support setup preview follows the current renderer projection instead of a stale selection scene",
    );

    let staleGroundReads = 0;
    previewInput.camera = {
      projectionSnapshot: () => ({ groundAtScreen: () => null }),
    };
    previewInput._groundAtScreen = () => {
      staleGroundReads += 1;
      return { x: 0, y: 0 };
    };
    previewInput._refreshAntiTankGunSetupPreview();
    assert(
      previewInput.clientIntent.antiTankGunSetupPreview === null && staleGroundReads === 0,
      "support setup preview clears for a current no-ground-hit instead of reusing stale selection geometry",
    );

    previewInput.camera = {
      projectionSnapshot: () => ({
        groundAtScreen: ({ x, y }) => ({ x: x + 96, y: y - 64 }),
      }),
    };
    previewInput._shiftKeyDown = true;
    previewInput._refreshAntiTankGunSetupPreview();
    const previewGuns = previewInput.clientIntent.antiTankGunSetupPreview?.guns || [];
    assert(
      previewGuns[0]?.x === 320 &&
        previewGuns[0]?.y === 192 &&
        movingAntiTankGun.x === 100 &&
        movingAntiTankGun.y === 120,
      "queued anti-tank gun setup preview uses the accepted movement endpoint without mutating the selected entity",
    );
    assert(
      previewGuns[1]?.x === 352 && previewGuns[1]?.y === 224,
      "artillery setup preview uses attack-move formation endpoints as projected origins",
    );
    assert(
      previewGuns[2]?.x === 180 && previewGuns[2]?.y === 120,
      "support setup preview falls back to current position when no movement plan is accepted",
    );
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorMouseEvent === undefined) delete globalThis.MouseEvent;
    else globalThis.MouseEvent = priorMouseEvent;
  }
}

// ---------------------------------------------------------------------------
