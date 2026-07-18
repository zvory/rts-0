#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

import {
  ABILITIES,
  ANTI_TANK_GUN_BODY,
  ANTI_TANK_GUN_DEPLOYED_RANGE_TILES,
  ANTI_TANK_GUN_FIELD_OF_FIRE_RAD,
  ARTILLERY_AMMO_COST,
  ARTILLERY_BLANKET_RADIUS_TILES,
  ARTILLERY_BODY,
  ARTILLERY_FIELD_OF_FIRE_RAD,
  ARTILLERY_MAX_RANGE_TILES,
  ARTILLERY_MIN_RANGE_TILES,
  ARTILLERY_OUTER_RADIUS_TILES,
  ARTILLERY_SETUP_TICKS,
  ARTILLERY_SHELL_DELAY_TICKS,
  BREAKTHROUGH_COOLDOWN_TICKS,
  BREAKTHROUGH_DURATION_TICKS,
  BREAKTHROUGH_RADIUS_TILES,
  BASE_COMMAND_SUPPLY_CAP,
  COMMAND_CAR_BODY,
  COMMAND_CAR_SUPPLY_CAP_BONUS,
  ENTRENCHMENT_AREA_DAMAGE_REDUCTION,
  ENTRENCHMENT_DIG_IN_TICKS,
  ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION,
  ENTRENCHMENT_RANGE_BONUS_TILES,
  ENTRENCHMENT_RESEARCH_TICKS,
  ENTRENCHMENT_TRENCH_RADIUS_TILES,
  EKAT_CONSUME_GOLEM_RANGE_TILES,
  EKAT_LINE_SHOT_COOLDOWN_TICKS,
  EKAT_LINE_SHOT_DAMAGE,
  EKAT_LINE_SHOT_RANGE_TILES,
  EKAT_LINE_SHOT_SPEED_PX_PER_TICK,
  EKAT_LINE_SHOT_WIDTH_TILES,
  EKAT_MAGIC_ANCHOR_DURATION_TICKS,
  EKAT_MAGIC_ANCHOR_PULL_AWAY_MULTIPLIER,
  EKAT_MAGIC_ANCHOR_PULL_TOWARD_MULTIPLIER,
  EKAT_MAGIC_ANCHOR_RADIUS_TILES,
  EKAT_MAGIC_ANCHOR_RANGE_TILES,
  EKAT_TELEPORT_COOLDOWN_TICKS,
  EKAT_TELEPORT_RANGE_TILES,
  FACTION_CATALOGS,
  MINING_CC_RANGE_TILES,
  METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS,
  MORTAR_FIRE_COOLDOWN_TICKS,
  MORTAR_FIELD_OF_FIRE_RAD,
  MORTAR_INNER_RADIUS_TILES,
  MORTAR_MIN_RANGE_TILES,
  MORTAR_OUTER_RADIUS_TILES,
  MORTAR_SETUP_TICKS,
  MORTAR_SHELL_DELAY_TICKS,
  MORTAR_TEARDOWN_TICKS,
  PANZERFAUST_ARMOR_PENETRATION,
  PANZERFAUST_DAMAGE,
  PANZERFAUST_RANGE_TILES,
  PANZERFAUSTS_RESEARCH_TICKS,
  PANZERFAUST_TRAVEL_TICKS,
  PANZERFAUST_WINDUP_TICKS,
  RESOURCE_AMOUNTS,
  SCOUT_CAR_BODY,
  SCOUT_PLANE_ABILITY_COOLDOWN_TICKS,
  SCOUT_PLANE_BODY,
  SCOUT_PLANE_LIFETIME_TICKS,
  SCOUT_PLANE_ORBIT_RADIUS_TILES,
  SCOUT_PLANE_SPEED_PX_PER_TICK,
  SMOKE_PLUS_RESEARCH_TICKS,
  SMOKE_ABILITY_COOLDOWN_TICKS,
  SMOKE_ABILITY_COST,
  SMOKE_ABILITY_RANGE_TILES,
  SMOKE_CLOUD_DURATION_TICKS,
  SMOKE_CLOUD_RADIUS_TILES,
  SMOKE_LAUNCH_MAX_DELAY_MS,
  STATS,
  TANK_BODY,
  TICK_HZ,
  UPGRADES,
  WORKER_BUILDABLE,
  METHAMPHETAMINES_RESEARCH_TICKS,
  ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS,
  ARTILLERY_UNLOCK_RESEARCH_TICKS,
  BALLISTIC_TABLES_RESEARCH_TICKS,
  TANK_UNLOCK_RESEARCH_TICKS,
  MORTAR_AUTOCAST_RESEARCH_TICKS,
  commandCardAbilitiesForFaction,
  researchableUpgradesForFaction,
  trainableUnitsForFaction,
  workerBuildablesForFaction,
} from "../client/src/config.js";
import { PLAYABLE_FACTIONS } from "../client/src/lobby_view.js";
import {
  ABILITY,
  ABILITY_CODE,
  DEFAULT_FACTION_ID,
  KIND,
  ORDER_STAGE_CODE,
  UPGRADE,
} from "../client/src/protocol.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const cargoTargetDir = path.resolve(repoRoot, process.env.CARGO_TARGET_DIR ?? path.join("server", "target"));
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const dumpFactionCatalogBin = path.join(cargoTargetDir, "debug", `dump-faction-catalog${executableSuffix}`);
const dumpCommandBudgetBin = path.join(cargoTargetDir, "debug", `dump-command-budget${executableSuffix}`);

execFileSync("cargo", [
  "build",
  "--manifest-path",
  "server/Cargo.toml",
  "-p",
  "rts-rules",
  "-p",
  "rts-sim",
  "--bins",
  "--quiet",
], { cwd: repoRoot, stdio: "inherit" });

const rustCatalog = JSON.parse(execFileSync(dumpFactionCatalogBin, [], {
  cwd: repoRoot,
  encoding: "utf8",
}));

const allRustCatalogs = JSON.parse(execFileSync(dumpFactionCatalogBin, [
  "--all",
], {
  cwd: repoRoot,
  encoding: "utf8",
}));

const rustCommandBudget = JSON.parse(execFileSync(dumpCommandBudgetBin, [], {
  cwd: repoRoot,
  encoding: "utf8",
}));

const kindByStableId = new Map(Object.entries(KIND).map(([, value]) => [value, value]));
const upgradeByStableId = new Map(Object.entries(UPGRADE).map(([, value]) => [value, value]));
const abilityByStableId = new Map(Object.entries(ABILITY).map(([, value]) => [value, value]));
const EXPECTED_CLIENT_CATALOG_IDS = Object.freeze([
  DEFAULT_FACTION_ID,
  "ekat",
  "phase2_empty_fixture",
]);
const EXPECTED_PLAYABLE_FACTION_IDS = Object.freeze([
  DEFAULT_FACTION_ID,
  "ekat",
]);
const EXPECTED_CLIENT_CONFIG_SECTIONS = Object.freeze([
  "abilityEffects",
  "bodies",
  "buildingStats",
  "constants",
  "resourceAmounts",
  "unitStats",
  "upgrades",
]);
const EXPECTED_CLIENT_CONFIG_CONSTANT_KEYS = Object.freeze([
  "antiTankGunDeployedRangeTiles",
  "antiTankGunFieldOfFireRad",
  "artilleryAmmoCost",
  "artilleryBlanketRadiusTiles",
  "artilleryFieldOfFireRad",
  "artilleryMaxRangeTiles",
  "artilleryMinRangeTiles",
  "artilleryOuterRadiusTiles",
  "artillerySetupTicks",
  "artilleryShellDelayTicks",
  "breakthroughCooldownTicks",
  "breakthroughDurationTicks",
  "breakthroughRadiusTiles",
  "entrenchmentAreaDamageReduction",
  "entrenchmentDigInTicks",
  "entrenchmentDirectDamageReduction",
  "entrenchmentRangeBonusTiles",
  "entrenchmentTrenchRadiusTiles",
  "ekatConsumeGolemRangeTiles",
  "ekatLineShotCooldownTicks",
  "ekatLineShotDamage",
  "ekatLineShotRangeTiles",
  "ekatLineShotSpeedPxPerTick",
  "ekatLineShotWidthTiles",
  "ekatMagicAnchorDurationTicks",
  "ekatMagicAnchorPullAwayMultiplier",
  "ekatMagicAnchorPullTowardMultiplier",
  "ekatMagicAnchorRadiusTiles",
  "ekatMagicAnchorRangeTiles",
  "ekatTeleportCooldownTicks",
  "ekatTeleportRangeTiles",
  "miningCcRangeTiles",
  "mortarFieldOfFireRad",
  "mortarFireCooldownTicks",
  "mortarInnerRadiusTiles",
  "mortarMinRangeTiles",
  "mortarOuterRadiusTiles",
  "mortarSetupTicks",
  "mortarShellDelayTicks",
  "mortarTeardownTicks",
  "methamphetaminesPanzerfaustWindupTicks",
  "panzerfaustArmorPenetration",
  "panzerfaustDamage",
  "panzerfaustRangeTiles",
  "panzerfaustTravelTicks",
  "panzerfaustWindupTicks",
  "scoutPlaneAbilityCooldownTicks",
  "scoutPlaneLifetimeTicks",
  "scoutPlaneOrbitRadiusTiles",
  "scoutPlaneSpeedPxPerTick",
  "smokeAbilityCooldownTicks",
  "smokeAbilityCost",
  "smokeAbilityRangeTiles",
  "smokeCloudDurationTicks",
  "smokeCloudRadiusTiles",
  "smokeLaunchMaxDelayMs",
  "tickHz",
]);
const EXPECTED_COMMAND_BUDGET_KEYS = Object.freeze([
  "baseCommandSupplyCap",
  "commandCarSupplyCapBonus",
]);
const EXPECTED_UNIT_STAT_FIELDS = Object.freeze([
  "buildTicks",
  "cost",
  "rangeTiles",
  "sight",
  "size",
  "supply",
]);
const EXPECTED_BUILDING_STAT_FIELDS = Object.freeze([
  "buildTicks",
  "cost",
  "footH",
  "footW",
  "sight",
]);
const EXPECTED_BODY_FIELDS = Object.freeze(["clearance", "length", "width"]);
const EXPECTED_UPGRADE_FIELDS = Object.freeze(["cost", "researchTicks", "requiresUpgrade"]);
const EXPECTED_ABILITY_EFFECT_FIELDS_BY_ID = Object.freeze({
  [ABILITY.SMOKE]: Object.freeze([
    "durationTicks",
    "radiusTiles",
    "upgradedDurationTicks",
    "upgradedRadiusTiles",
  ]),
  [ABILITY.MORTAR_FIRE]: Object.freeze(["radiusTiles"]),
  [ABILITY.POINT_FIRE]: Object.freeze(["delayTicks", "radiusTiles"]),
  [ABILITY.BLANKET_FIRE]: Object.freeze(["radiusTiles"]),
  [ABILITY.BREAKTHROUGH]: Object.freeze(["durationTicks", "radiusTiles"]),
  [ABILITY.EKAT_TELEPORT]: Object.freeze([]),
  [ABILITY.EKAT_LINE_SHOT]: Object.freeze(["damage", "radiusTiles", "speedPxPerTick"]),
  [ABILITY.EKAT_MAGIC_ANCHOR]: Object.freeze([
    "durationTicks",
    "pullAwayMultiplier",
    "pullTowardMultiplier",
    "radiusTiles",
  ]),
  [ABILITY.EKAT_CONSUME_GOLEM]: Object.freeze(["radiusTiles"]),
});
const EXPECTED_EXTRA_UNIT_STATS = Object.freeze([]);

function asClientKinds(kinds) {
  return kinds.map((kind) => {
    assert(kindByStableId.has(kind), `client KIND is missing ${kind}`);
    return kindByStableId.get(kind);
  });
}

function assertApprox(actual, expected, message) {
  assert.equal(Number.isFinite(actual), true, `${message}: actual is finite`);
  assert.equal(Number.isFinite(expected), true, `${message}: expected is finite`);
  assert(Math.abs(actual - expected) < 0.000001, `${message}: ${actual} !== ${expected}`);
}

function assertClientStatField(clientStats, field, expected, message) {
  if (typeof expected === "number" && !Number.isInteger(expected)) {
    assertApprox(clientStats?.[field], expected, message);
  } else {
    assert.deepEqual(clientStats?.[field], expected, message);
  }
}

function assertClientObjectFields(actual, expected, message) {
  for (const [field, value] of Object.entries(expected)) {
    if (value && typeof value === "object" && !Array.isArray(value)) {
      assert.deepEqual(actual?.[field], value, `${message} ${field}`);
    } else if (typeof value === "number" && !Number.isInteger(value)) {
      assertApprox(actual?.[field], value, `${message} ${field}`);
    } else {
      assert.equal(actual?.[field], value, `${message} ${field}`);
    }
  }
}

function sorted(values) {
  return [...values].sort();
}

function assertObjectKeys(actual, expected, message) {
  assert.deepEqual(Object.keys(actual || {}), expected, message);
}

function assertSortedObjectKeys(actual, expected, message) {
  assert.deepEqual(sorted(Object.keys(actual || {})), sorted(expected), message);
}

function assertAbilityDescriptor(entry, factionId) {
  assert(abilityByStableId.has(entry.id), `client ABILITY is missing ${entry.id}`);
  const ability = abilityByStableId.get(entry.id);
  assert.equal(ABILITY_CODE[ability], entry.protocolCode, `${factionId} ${entry.id} compact ability code mirrors Rust registry`);
  assert.equal(
    ORDER_STAGE_CODE[ability],
    entry.orderStageCode,
    `${factionId} ${entry.id} order stage code mirrors Rust registry`,
  );
  const descriptor = ABILITIES[ability];
  if (!entry.commandCard && !descriptor) {
    assert.equal(
      descriptor,
      undefined,
      `${factionId} ${entry.id} is registry-only and should not render a command-card descriptor`,
    );
    return;
  }
  if (!entry.commandCard) {
    assert.equal(
      descriptor.commandCard,
      false,
      `${factionId} ${entry.id} descriptor must stay hidden from the command card`,
    );
  }
  assert(descriptor, `client ABILITIES is missing command-card descriptor for ${factionId} ${entry.id}`);
  assert.equal(descriptor.ability, ability, `${factionId} ${entry.id} descriptor identity mirrors protocol ability`);
  assert.equal(descriptor.label, entry.label, `${factionId} ${entry.id} label mirrors Rust registry`);
  assert.equal(descriptor.icon, entry.icon, `${factionId} ${entry.id} icon mirrors Rust registry`);
  assert.equal(descriptor.hotkey ?? null, entry.hotkey, `${factionId} ${entry.id} hotkey mirrors Rust registry`);
  assert.equal(descriptor.title, entry.title, `${factionId} ${entry.id} title mirrors Rust registry`);
  assert.deepEqual(
    descriptor.carriers,
    asClientKinds(entry.carriers),
    `${factionId} ${entry.id} carriers mirror Rust catalog`,
  );
  assert.equal(descriptor.targetMode, entry.targetMode, `${factionId} ${entry.id} target mode mirrors Rust registry`);
  assert.equal(descriptor.rangeTiles ?? null, entry.rangeTiles, `${factionId} ${entry.id} range mirrors Rust registry`);
  assert.equal(descriptor.minRangeTiles ?? null, entry.minRangeTiles, `${factionId} ${entry.id} min range mirrors Rust registry`);
  assert.equal(descriptor.cooldownTicks, entry.cooldownTicks, `${factionId} ${entry.id} cooldown mirrors Rust registry`);
  assert.equal(descriptor.charges ?? null, entry.charges, `${factionId} ${entry.id} charges mirror Rust registry`);
  assert.deepEqual(descriptor.cost, entry.cost, `${factionId} ${entry.id} cost mirrors Rust registry`);
  assert.equal(descriptor.techRequirement ?? null, entry.techRequirement, `${factionId} ${entry.id} tech requirement mirrors Rust registry`);
  assert.equal(descriptor.queued, entry.mayQueue, `${factionId} ${entry.id} queue behavior mirrors Rust registry`);
  assert.equal(descriptor.queuePolicy, entry.queuePolicy, `${factionId} ${entry.id} queue policy mirrors Rust registry`);
  assert.equal(!!descriptor.autocast, entry.autocast, `${factionId} ${entry.id} autocast flag mirrors Rust registry`);
}

assert.equal(rustCatalog.id, DEFAULT_FACTION_ID, "default faction id mirrors client protocol");
assert.equal(
  allRustCatalogs.catalogs.some((catalog) => catalog.id === DEFAULT_FACTION_ID),
  true,
  "all-catalog dump includes the default faction",
);
assert.equal(
  allRustCatalogs.catalogs.some((catalog) => catalog.id === "phase2_empty_fixture"),
  true,
  "all-catalog dump exposes fixture catalogs for explicit unsupported handling",
);
assert.deepEqual(
  allRustCatalogs.catalogs.map((catalog) => catalog.id),
  EXPECTED_CLIENT_CATALOG_IDS,
  "Rust all-catalog dump exposes the expected client-mirrored catalog ids",
);
assert.deepEqual(
  sorted(Object.keys(FACTION_CATALOGS)),
  sorted(EXPECTED_CLIENT_CATALOG_IDS),
  "client FACTION_CATALOGS must not add or omit catalog ids silently",
);
assert.deepEqual(
  PLAYABLE_FACTIONS.map((entry) => entry.id),
  EXPECTED_PLAYABLE_FACTION_IDS,
  "client playable faction selector must only expose product-playable ids",
);
assert.equal(
  PLAYABLE_FACTIONS.some((entry) => entry.id === "phase2_empty_fixture"),
  false,
  "fixture-only faction id must not appear as a playable client option",
);
assert.deepEqual(
  WORKER_BUILDABLE,
  asClientKinds(rustCatalog.buildables.map((entry) => entry.kind)),
  "client worker build menu mirrors Rust faction catalog",
);

for (const rustFaction of allRustCatalogs.catalogs) {
  const clientFaction = FACTION_CATALOGS[rustFaction.id];
  assert(clientFaction, `client FACTION_CATALOGS is missing ${rustFaction.id}`);
  assert.equal(clientFaction.id, rustFaction.id, `${rustFaction.id} catalog id mirrors Rust catalog`);
  assert.equal(clientFaction.loadoutId, rustFaction.loadoutId, `${rustFaction.id} loadoutId mirrors Rust catalog`);
  assert.deepEqual(clientFaction.units, asClientKinds(rustFaction.units), `${rustFaction.id} units mirror Rust catalog`);
  assert.deepEqual(clientFaction.buildings, asClientKinds(rustFaction.buildings), `${rustFaction.id} buildings mirror Rust catalog`);
  assert.deepEqual(
    workerBuildablesForFaction(rustFaction.id),
    asClientKinds(rustFaction.buildables.map((entry) => entry.kind)),
    `${rustFaction.id} worker buildables mirror Rust catalog`,
  );
  for (const entry of rustFaction.trainables) {
    assert.deepEqual(
      trainableUnitsForFaction(rustFaction.id, kindByStableId.get(entry.building)),
      asClientKinds(entry.units),
      `${rustFaction.id} ${entry.building} trainables mirror Rust catalog`,
    );
  }
  assertObjectKeys(
    clientFaction.trainables,
    rustFaction.trainables.map((entry) => kindByStableId.get(entry.building)),
    `${rustFaction.id} trainable producer keys mirror Rust catalog`,
  );
  for (const building of rustFaction.buildings) {
    const clientBuilding = kindByStableId.get(building);
    const expectedResearch = rustFaction.research
      .filter((entry) => entry.researchedAt === building)
      .map((entry) => upgradeByStableId.get(entry.id));
    assert.deepEqual(
      researchableUpgradesForFaction(rustFaction.id, clientBuilding),
      expectedResearch,
      `${rustFaction.id} ${building} research list mirrors Rust catalog`,
    );
  }
  assertObjectKeys(
    clientFaction.research,
    rustFaction.research
      .map((entry) => kindByStableId.get(entry.researchedAt))
      .filter((building, index, buildings) => buildings.indexOf(building) === index),
    `${rustFaction.id} research building keys mirror Rust catalog`,
  );
  assert.deepEqual(
    commandCardAbilitiesForFaction(rustFaction.id).map((entry) => entry.ability),
    rustFaction.abilities
      .filter((entry) => entry.commandCard)
      .map((entry) => abilityByStableId.get(entry.id)),
    `${rustFaction.id} command-card abilities mirror Rust catalog`,
  );
  assert.deepEqual(
    clientFaction.abilities.filter((ability) => ABILITIES[ability]),
    rustFaction.abilities
      .filter((entry) => {
        const ability = abilityByStableId.get(entry.id);
        return entry.commandCard || ABILITIES[ability]?.commandCard === false;
      })
      .map((entry) => abilityByStableId.get(entry.id)),
    `${rustFaction.id} descriptor-backed abilities mirror Rust catalog, including hidden descriptors`,
  );
  assert.deepEqual(
    rustFaction.builders.map((kind) => kindByStableId.get(kind)),
    rustFaction.id === DEFAULT_FACTION_ID ? [KIND.WORKER] : [],
    `${rustFaction.id} builder set remains explicit`,
  );
  assert.deepEqual(
    rustFaction.gatherers.map((kind) => kindByStableId.get(kind)),
    rustFaction.id === DEFAULT_FACTION_ID
      ? [KIND.WORKER]
      : rustFaction.id === "ekat"
        ? [KIND.GOLEM]
        : [],
    `${rustFaction.id} gatherer set remains explicit`,
  );
  assert.deepEqual(
    rustFaction.productionAnchors.map((kind) => kindByStableId.get(kind)),
    rustFaction.trainables.map((entry) => kindByStableId.get(entry.building)),
    `${rustFaction.id} production anchors match exported trainable producers`,
  );
  for (const entry of rustFaction.abilities) {
    assertAbilityDescriptor(entry, rustFaction.id);
  }
  for (const [kind, cost] of Object.entries(rustFaction.costs)) {
    const clientKind = kindByStableId.get(kind);
    assert.deepEqual(STATS[clientKind]?.cost, cost, `${rustFaction.id} ${kind} cost mirrors Rust catalog`);
  }
}

for (const entry of rustCatalog.buildables) {
  const clientKind = kindByStableId.get(entry.kind);
  const clientRequires = STATS[clientKind]?.requires ?? [];
  const expectedRequires = asClientKinds(entry.requires);
  assert.deepEqual(
    Array.isArray(clientRequires) ? clientRequires : [clientRequires],
    expectedRequires,
    `${entry.kind} build requirements mirror Rust catalog`,
  );
}

for (const entry of rustCatalog.trainables) {
  const clientBuilding = kindByStableId.get(entry.building);
  assert.deepEqual(
    STATS[clientBuilding]?.trains ?? [],
    asClientKinds(entry.units),
    `${entry.building} train list mirrors Rust catalog`,
  );
}

for (const entry of rustCatalog.research) {
  assert(upgradeByStableId.has(entry.id), `client UPGRADE is missing ${entry.id}`);
  const upgrade = upgradeByStableId.get(entry.id);
  assert.equal(
    UPGRADES[upgrade]?.researchedAt,
    kindByStableId.get(entry.researchedAt),
    `${entry.id} research building mirrors Rust catalog`,
  );
}

for (const [kind, cost] of Object.entries(rustCatalog.costs)) {
  const clientKind = kindByStableId.get(kind);
  assert.deepEqual(STATS[clientKind]?.cost, cost, `${kind} cost mirrors Rust catalog`);
}

const rustClientConfig = rustCatalog.clientConfig;
assert(rustClientConfig, "rules dump includes clientConfig parity payload");
assertSortedObjectKeys(
  rustClientConfig,
  EXPECTED_CLIENT_CONFIG_SECTIONS,
  "rules dump clientConfig sections remain explicit",
);
assertSortedObjectKeys(
  rustClientConfig.constants,
  EXPECTED_CLIENT_CONFIG_CONSTANT_KEYS,
  "rules dump clientConfig constants guard the current mirrored scalar surface",
);

const clientConstants = {
  tickHz: TICK_HZ,
  miningCcRangeTiles: MINING_CC_RANGE_TILES,
  antiTankGunDeployedRangeTiles: ANTI_TANK_GUN_DEPLOYED_RANGE_TILES,
  antiTankGunFieldOfFireRad: ANTI_TANK_GUN_FIELD_OF_FIRE_RAD,
  mortarMinRangeTiles: MORTAR_MIN_RANGE_TILES,
  mortarFieldOfFireRad: MORTAR_FIELD_OF_FIRE_RAD,
  mortarSetupTicks: MORTAR_SETUP_TICKS,
  mortarTeardownTicks: MORTAR_TEARDOWN_TICKS,
  artilleryMinRangeTiles: ARTILLERY_MIN_RANGE_TILES,
  artilleryMaxRangeTiles: ARTILLERY_MAX_RANGE_TILES,
  artilleryFieldOfFireRad: ARTILLERY_FIELD_OF_FIRE_RAD,
  artillerySetupTicks: ARTILLERY_SETUP_TICKS,
  artilleryShellDelayTicks: ARTILLERY_SHELL_DELAY_TICKS,
  artilleryOuterRadiusTiles: ARTILLERY_OUTER_RADIUS_TILES,
  artilleryBlanketRadiusTiles: ARTILLERY_BLANKET_RADIUS_TILES,
  artilleryAmmoCost: ARTILLERY_AMMO_COST,
  smokeAbilityRangeTiles: SMOKE_ABILITY_RANGE_TILES,
  smokeLaunchMaxDelayMs: SMOKE_LAUNCH_MAX_DELAY_MS,
  smokeCloudRadiusTiles: SMOKE_CLOUD_RADIUS_TILES,
  smokeCloudDurationTicks: SMOKE_CLOUD_DURATION_TICKS,
  smokeAbilityCooldownTicks: SMOKE_ABILITY_COOLDOWN_TICKS,
  scoutPlaneOrbitRadiusTiles: SCOUT_PLANE_ORBIT_RADIUS_TILES,
  scoutPlaneSpeedPxPerTick: SCOUT_PLANE_SPEED_PX_PER_TICK,
  scoutPlaneLifetimeTicks: SCOUT_PLANE_LIFETIME_TICKS,
  scoutPlaneAbilityCooldownTicks: SCOUT_PLANE_ABILITY_COOLDOWN_TICKS,
  smokeAbilityCost: SMOKE_ABILITY_COST,
  mortarShellDelayTicks: MORTAR_SHELL_DELAY_TICKS,
  mortarOuterRadiusTiles: MORTAR_OUTER_RADIUS_TILES,
  mortarInnerRadiusTiles: MORTAR_INNER_RADIUS_TILES,
  mortarFireCooldownTicks: MORTAR_FIRE_COOLDOWN_TICKS,
  panzerfaustRangeTiles: PANZERFAUST_RANGE_TILES,
  panzerfaustDamage: PANZERFAUST_DAMAGE,
  panzerfaustArmorPenetration: PANZERFAUST_ARMOR_PENETRATION,
  panzerfaustWindupTicks: PANZERFAUST_WINDUP_TICKS,
  panzerfaustTravelTicks: PANZERFAUST_TRAVEL_TICKS,
  methamphetaminesPanzerfaustWindupTicks: METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS,
  entrenchmentDigInTicks: ENTRENCHMENT_DIG_IN_TICKS,
  entrenchmentRangeBonusTiles: ENTRENCHMENT_RANGE_BONUS_TILES,
  entrenchmentDirectDamageReduction: ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION,
  entrenchmentAreaDamageReduction: ENTRENCHMENT_AREA_DAMAGE_REDUCTION,
  entrenchmentTrenchRadiusTiles: ENTRENCHMENT_TRENCH_RADIUS_TILES,
  ekatConsumeGolemRangeTiles: EKAT_CONSUME_GOLEM_RANGE_TILES,
  ekatTeleportRangeTiles: EKAT_TELEPORT_RANGE_TILES,
  ekatTeleportCooldownTicks: EKAT_TELEPORT_COOLDOWN_TICKS,
  ekatLineShotRangeTiles: EKAT_LINE_SHOT_RANGE_TILES,
  ekatLineShotWidthTiles: EKAT_LINE_SHOT_WIDTH_TILES,
  ekatLineShotSpeedPxPerTick: EKAT_LINE_SHOT_SPEED_PX_PER_TICK,
  ekatLineShotDamage: EKAT_LINE_SHOT_DAMAGE,
  ekatLineShotCooldownTicks: EKAT_LINE_SHOT_COOLDOWN_TICKS,
  ekatMagicAnchorRangeTiles: EKAT_MAGIC_ANCHOR_RANGE_TILES,
  ekatMagicAnchorDurationTicks: EKAT_MAGIC_ANCHOR_DURATION_TICKS,
  ekatMagicAnchorRadiusTiles: EKAT_MAGIC_ANCHOR_RADIUS_TILES,
  ekatMagicAnchorPullAwayMultiplier: EKAT_MAGIC_ANCHOR_PULL_AWAY_MULTIPLIER,
  ekatMagicAnchorPullTowardMultiplier: EKAT_MAGIC_ANCHOR_PULL_TOWARD_MULTIPLIER,
  breakthroughRadiusTiles: BREAKTHROUGH_RADIUS_TILES,
  breakthroughDurationTicks: BREAKTHROUGH_DURATION_TICKS,
  breakthroughCooldownTicks: BREAKTHROUGH_COOLDOWN_TICKS,
};
assertClientObjectFields(
  clientConstants,
  rustClientConfig.constants,
  "client config constant mirrors Rust rules",
);

assertSortedObjectKeys(
  rustCommandBudget,
  EXPECTED_COMMAND_BUDGET_KEYS,
  "sim command-budget dump fields remain explicit",
);
assertClientObjectFields(
  {
    baseCommandSupplyCap: BASE_COMMAND_SUPPLY_CAP,
    commandCarSupplyCapBonus: COMMAND_CAR_SUPPLY_CAP_BONUS,
  },
  rustCommandBudget,
  "client command-budget constants mirror sim command admission policy",
);

assertSortedObjectKeys(
  rustClientConfig.unitStats,
  new Set([
    ...allRustCatalogs.catalogs.flatMap((catalog) => catalog.units),
    ...EXPECTED_EXTRA_UNIT_STATS,
  ]),
  "rules dump unitStats cover every client-mirrored catalog unit plus explicit extra unit stats",
);
for (const [kind, expected] of Object.entries(rustClientConfig.unitStats)) {
  assertSortedObjectKeys(
    expected,
    EXPECTED_UNIT_STAT_FIELDS,
    `${kind} unitStats payload fields remain explicit`,
  );
  const clientKind = kindByStableId.get(kind);
  assert(clientKind, `client KIND is missing ${kind}`);
  const clientStats = STATS[clientKind];
  assert(clientStats, `client STATS is missing ${kind}`);
  if (!clientStats.body) {
    assertClientStatField(clientStats, "size", expected.size, `${kind} render size mirrors Rust radius`);
  }
  assertClientStatField(clientStats, "sight", expected.sight, `${kind} sight mirrors Rust rules`);
  assertClientStatField(clientStats, "rangeTiles", expected.rangeTiles, `${kind} range mirrors Rust rules`);
  assert.deepEqual(clientStats.cost, expected.cost, `${kind} cost mirrors Rust rules`);
  assert.equal(clientStats.supply, expected.supply, `${kind} supply mirrors Rust rules`);
  assert.equal(clientStats.buildTicks, expected.buildTicks, `${kind} build ticks mirror Rust rules`);
}

assertSortedObjectKeys(
  rustClientConfig.buildingStats,
  new Set(allRustCatalogs.catalogs.flatMap((catalog) => catalog.buildings)),
  "rules dump buildingStats cover every client-mirrored catalog building",
);
for (const [kind, expected] of Object.entries(rustClientConfig.buildingStats)) {
  assertSortedObjectKeys(
    expected,
    EXPECTED_BUILDING_STAT_FIELDS,
    `${kind} buildingStats payload fields remain explicit`,
  );
  const clientKind = kindByStableId.get(kind);
  assert(clientKind, `client KIND is missing ${kind}`);
  const clientStats = STATS[clientKind];
  assert(clientStats, `client STATS is missing ${kind}`);
  assert.equal(clientStats.footW, expected.footW, `${kind} footprint width mirrors Rust rules`);
  assert.equal(clientStats.footH, expected.footH, `${kind} footprint height mirrors Rust rules`);
  assert.equal(clientStats.sight, expected.sight, `${kind} sight mirrors Rust rules`);
  assert.deepEqual(clientStats.cost, expected.cost, `${kind} cost mirrors Rust rules`);
  assert.equal(clientStats.buildTicks, expected.buildTicks, `${kind} build ticks mirror Rust rules`);
}

assert.deepEqual(
  RESOURCE_AMOUNTS,
  Object.fromEntries(Object.entries(rustClientConfig.resourceAmounts).map(([kind, amount]) => [
    kindByStableId.get(kind),
    amount,
  ])),
  "resource starting amounts mirror Rust node defs",
);

const clientBodies = {
  [KIND.TANK]: TANK_BODY,
  [KIND.ANTI_TANK_GUN]: ANTI_TANK_GUN_BODY,
  [KIND.ARTILLERY]: ARTILLERY_BODY,
  [KIND.SCOUT_CAR]: SCOUT_CAR_BODY,
  [KIND.SCOUT_PLANE]: SCOUT_PLANE_BODY,
  [KIND.COMMAND_CAR]: COMMAND_CAR_BODY,
};
assertSortedObjectKeys(
  rustClientConfig.bodies,
  Object.keys(clientBodies),
  "rules dump bodies cover every client-mirrored vehicle body",
);
for (const [kind, expected] of Object.entries(rustClientConfig.bodies)) {
  assertSortedObjectKeys(
    expected,
    EXPECTED_BODY_FIELDS,
    `${kind} body payload fields remain explicit`,
  );
  const clientKind = kindByStableId.get(kind);
  assert(clientKind, `client KIND is missing ${kind}`);
  assertClientObjectFields(clientBodies[clientKind], expected, `${kind} body mirror`);
}

const clientUpgradeResearchTicks = {
  [UPGRADE.METHAMPHETAMINES]: METHAMPHETAMINES_RESEARCH_TICKS,
  [UPGRADE.PANZERFAUSTS]: PANZERFAUSTS_RESEARCH_TICKS,
  [UPGRADE.ENTRENCHMENT]: ENTRENCHMENT_RESEARCH_TICKS,
  [UPGRADE.ANTI_TANK_GUN_UNLOCK]: ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS,
  [UPGRADE.ARTILLERY_UNLOCK]: ARTILLERY_UNLOCK_RESEARCH_TICKS,
  [UPGRADE.BALLISTIC_TABLES]: BALLISTIC_TABLES_RESEARCH_TICKS,
  [UPGRADE.TANK_UNLOCK]: TANK_UNLOCK_RESEARCH_TICKS,
  [UPGRADE.MORTAR_AUTOCAST]: MORTAR_AUTOCAST_RESEARCH_TICKS,
  [UPGRADE.SMOKE_PLUS]: SMOKE_PLUS_RESEARCH_TICKS,
};
assertSortedObjectKeys(
  rustClientConfig.upgrades,
  new Set(allRustCatalogs.catalogs.flatMap((catalog) => catalog.research.map((entry) => entry.id))),
  "rules dump upgrades cover every client-mirrored catalog upgrade",
);
for (const [upgradeId, expected] of Object.entries(rustClientConfig.upgrades)) {
  assertSortedObjectKeys(
    expected,
    EXPECTED_UPGRADE_FIELDS,
    `${upgradeId} upgrade payload fields remain explicit`,
  );
  const upgrade = upgradeByStableId.get(upgradeId);
  assert(upgrade, `client UPGRADE is missing ${upgradeId}`);
  assert.deepEqual(UPGRADES[upgrade]?.cost, expected.cost, `${upgradeId} cost mirrors Rust rules`);
  assert.equal(
    clientUpgradeResearchTicks[upgrade],
    expected.researchTicks,
    `${upgradeId} exported research ticks mirror Rust rules`,
  );
  assert.equal(UPGRADES[upgrade]?.researchTicks, expected.researchTicks, `${upgradeId} research ticks mirror Rust rules`);
  assert.equal(
    UPGRADES[upgrade]?.requiresUpgrade ?? null,
    expected.requiresUpgrade,
    `${upgradeId} upgrade prerequisite mirrors Rust rules`,
  );
}

assertSortedObjectKeys(
  rustClientConfig.abilityEffects,
  Object.keys(EXPECTED_ABILITY_EFFECT_FIELDS_BY_ID),
  "rules dump abilityEffects cover every client-mirrored ability effect record",
);
for (const [abilityId, expected] of Object.entries(rustClientConfig.abilityEffects)) {
  assertSortedObjectKeys(
    expected,
    EXPECTED_ABILITY_EFFECT_FIELDS_BY_ID[abilityId],
    `${abilityId} ability effect payload fields remain explicit`,
  );
  const ability = abilityByStableId.get(abilityId);
  assert(ability, `client ABILITY is missing ${abilityId}`);
  assertClientObjectFields(ABILITIES[ability], expected, `${abilityId} ability effect mirror`);
}

console.log("faction catalog parity check passed");
