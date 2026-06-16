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
  COMMAND_CAR_BODY,
  EKAT_LINE_SHOT_COOLDOWN_TICKS,
  EKAT_LINE_SHOT_DAMAGE,
  EKAT_LINE_SHOT_RANGE_TILES,
  EKAT_LINE_SHOT_SPEED_PX_PER_TICK,
  EKAT_LINE_SHOT_WIDTH_TILES,
  EKAT_MAGIC_ANCHOR_DURATION_TICKS,
  EKAT_MAGIC_ANCHOR_HP,
  EKAT_MAGIC_ANCHOR_LOCKOUT_TICKS,
  EKAT_MAGIC_ANCHOR_RADIUS_TILES,
  EKAT_MAGIC_ANCHOR_RANGE_TILES,
  EKAT_REGEN_TICKS,
  EKAT_TELEPORT_COOLDOWN_TICKS,
  EKAT_TELEPORT_RANGE_TILES,
  FACTION_CATALOGS,
  MINING_CC_RANGE_TILES,
  MORTAR_FIRE_COOLDOWN_TICKS,
  MORTAR_INNER_RADIUS_TILES,
  MORTAR_OUTER_RADIUS_TILES,
  MORTAR_SHELL_DELAY_TICKS,
  RESOURCE_AMOUNTS,
  RIFLEMAN_CHARGE_COOLDOWN_TICKS,
  SCOUT_CAR_BODY,
  SCOUT_CAR_SMOKE_USES,
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
  TANK_UNLOCK_RESEARCH_TICKS,
  COMMAND_CAR_UNLOCK_RESEARCH_TICKS,
  MORTAR_AUTOCAST_RESEARCH_TICKS,
  commandCardAbilitiesForFaction,
  researchableUpgradesForFaction,
  trainableUnitsForFaction,
  workerBuildablesForFaction,
} from "../client/src/config.js";
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

const rustCatalog = JSON.parse(execFileSync("cargo", [
  "run",
  "--manifest-path",
  "server/Cargo.toml",
  "-p",
  "rts-rules",
  "--bin",
  "dump-faction-catalog",
  "--quiet",
], {
  cwd: repoRoot,
  encoding: "utf8",
}));

const allRustCatalogs = JSON.parse(execFileSync("cargo", [
  "run",
  "--manifest-path",
  "server/Cargo.toml",
  "-p",
  "rts-rules",
  "--bin",
  "dump-faction-catalog",
  "--quiet",
  "--",
  "--all",
], {
  cwd: repoRoot,
  encoding: "utf8",
}));

const kindByStableId = new Map(Object.entries(KIND).map(([, value]) => [value, value]));
const upgradeByStableId = new Map(Object.entries(UPGRADE).map(([, value]) => [value, value]));
const abilityByStableId = new Map(Object.entries(ABILITY).map(([, value]) => [value, value]));

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
  WORKER_BUILDABLE,
  asClientKinds(rustCatalog.buildables.map((entry) => entry.kind)),
  "client worker build menu mirrors Rust faction catalog",
);

for (const rustFaction of allRustCatalogs.catalogs) {
  const clientFaction = FACTION_CATALOGS[rustFaction.id];
  assert(clientFaction, `client FACTION_CATALOGS is missing ${rustFaction.id}`);
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
  assert.deepEqual(
    commandCardAbilitiesForFaction(rustFaction.id).map((entry) => entry.ability),
    rustFaction.abilities
      .filter((entry) => entry.commandCard)
      .map((entry) => abilityByStableId.get(entry.id)),
    `${rustFaction.id} command-card abilities mirror Rust catalog`,
  );
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

for (const entry of rustCatalog.abilities) {
  assert(abilityByStableId.has(entry.id), `client ABILITY is missing ${entry.id}`);
  const ability = abilityByStableId.get(entry.id);
  assert.equal(ABILITY_CODE[ability], entry.protocolCode, `${entry.id} compact ability code mirrors Rust registry`);
  assert.equal(
    ORDER_STAGE_CODE[ability],
    entry.orderStageCode,
    `${entry.id} order stage code mirrors Rust registry`,
  );
  if (!entry.commandCard) {
    assert.equal(
      ABILITIES[ability],
      undefined,
      `${entry.id} is registry-only and should not render a command-card descriptor`,
    );
    continue;
  }
  const descriptor = ABILITIES[ability];
  assert(descriptor, `client ABILITIES is missing command-card descriptor for ${entry.id}`);
  assert.equal(descriptor.label, entry.label, `${entry.id} label mirrors Rust registry`);
  assert.equal(descriptor.icon, entry.icon, `${entry.id} icon mirrors Rust registry`);
  assert.equal(descriptor.hotkey ?? null, entry.hotkey, `${entry.id} hotkey mirrors Rust registry`);
  assert.equal(descriptor.title, entry.title, `${entry.id} title mirrors Rust registry`);
  assert.deepEqual(
    descriptor.carriers,
    asClientKinds(entry.carriers),
    `${entry.id} carriers mirror Rust catalog`,
  );
  assert.equal(descriptor.targetMode, entry.targetMode, `${entry.id} target mode mirrors Rust registry`);
  assert.equal(descriptor.rangeTiles ?? null, entry.rangeTiles, `${entry.id} range mirrors Rust registry`);
  assert.equal(descriptor.minRangeTiles ?? null, entry.minRangeTiles, `${entry.id} min range mirrors Rust registry`);
  assert.equal(descriptor.cooldownTicks, entry.cooldownTicks, `${entry.id} cooldown mirrors Rust registry`);
  assert.deepEqual(descriptor.cost, entry.cost, `${entry.id} cost mirrors Rust registry`);
  assert.equal(descriptor.queued, entry.mayQueue, `${entry.id} queue behavior mirrors Rust registry`);
  assert.equal(!!descriptor.autocast, entry.autocast, `${entry.id} autocast flag mirrors Rust registry`);
}

for (const [kind, cost] of Object.entries(rustCatalog.costs)) {
  const clientKind = kindByStableId.get(kind);
  assert.deepEqual(STATS[clientKind]?.cost, cost, `${kind} cost mirrors Rust catalog`);
}

const rustClientConfig = rustCatalog.clientConfig;
assert(rustClientConfig, "rules dump includes clientConfig parity payload");

const clientConstants = {
  tickHz: TICK_HZ,
  miningCcRangeTiles: MINING_CC_RANGE_TILES,
  antiTankGunDeployedRangeTiles: ANTI_TANK_GUN_DEPLOYED_RANGE_TILES,
  antiTankGunFieldOfFireRad: ANTI_TANK_GUN_FIELD_OF_FIRE_RAD,
  artilleryMinRangeTiles: ARTILLERY_MIN_RANGE_TILES,
  artilleryMaxRangeTiles: ARTILLERY_MAX_RANGE_TILES,
  artilleryFieldOfFireRad: ARTILLERY_FIELD_OF_FIRE_RAD,
  artillerySetupTicks: ARTILLERY_SETUP_TICKS,
  artilleryShellDelayTicks: ARTILLERY_SHELL_DELAY_TICKS,
  artilleryOuterRadiusTiles: ARTILLERY_OUTER_RADIUS_TILES,
  artilleryAmmoCost: ARTILLERY_AMMO_COST,
  riflemanChargeCooldownTicks: RIFLEMAN_CHARGE_COOLDOWN_TICKS,
  smokeAbilityRangeTiles: SMOKE_ABILITY_RANGE_TILES,
  smokeLaunchMaxDelayMs: SMOKE_LAUNCH_MAX_DELAY_MS,
  smokeCloudRadiusTiles: SMOKE_CLOUD_RADIUS_TILES,
  smokeCloudDurationTicks: SMOKE_CLOUD_DURATION_TICKS,
  smokeAbilityCooldownTicks: SMOKE_ABILITY_COOLDOWN_TICKS,
  scoutCarSmokeUses: SCOUT_CAR_SMOKE_USES,
  smokeAbilityCost: SMOKE_ABILITY_COST,
  mortarShellDelayTicks: MORTAR_SHELL_DELAY_TICKS,
  mortarOuterRadiusTiles: MORTAR_OUTER_RADIUS_TILES,
  mortarInnerRadiusTiles: MORTAR_INNER_RADIUS_TILES,
  mortarFireCooldownTicks: MORTAR_FIRE_COOLDOWN_TICKS,
  ekatRegenTicks: EKAT_REGEN_TICKS,
  ekatTeleportRangeTiles: EKAT_TELEPORT_RANGE_TILES,
  ekatTeleportCooldownTicks: EKAT_TELEPORT_COOLDOWN_TICKS,
  ekatLineShotRangeTiles: EKAT_LINE_SHOT_RANGE_TILES,
  ekatLineShotWidthTiles: EKAT_LINE_SHOT_WIDTH_TILES,
  ekatLineShotSpeedPxPerTick: EKAT_LINE_SHOT_SPEED_PX_PER_TICK,
  ekatLineShotDamage: EKAT_LINE_SHOT_DAMAGE,
  ekatLineShotCooldownTicks: EKAT_LINE_SHOT_COOLDOWN_TICKS,
  ekatMagicAnchorRangeTiles: EKAT_MAGIC_ANCHOR_RANGE_TILES,
  ekatMagicAnchorDurationTicks: EKAT_MAGIC_ANCHOR_DURATION_TICKS,
  ekatMagicAnchorLockoutTicks: EKAT_MAGIC_ANCHOR_LOCKOUT_TICKS,
  ekatMagicAnchorHp: EKAT_MAGIC_ANCHOR_HP,
  ekatMagicAnchorRadiusTiles: EKAT_MAGIC_ANCHOR_RADIUS_TILES,
  breakthroughRadiusTiles: BREAKTHROUGH_RADIUS_TILES,
  breakthroughDurationTicks: BREAKTHROUGH_DURATION_TICKS,
  breakthroughCooldownTicks: BREAKTHROUGH_COOLDOWN_TICKS,
};
assertClientObjectFields(
  clientConstants,
  rustClientConfig.constants,
  "client config constant mirrors Rust rules",
);

for (const [kind, expected] of Object.entries(rustClientConfig.unitStats)) {
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

for (const [kind, expected] of Object.entries(rustClientConfig.buildingStats)) {
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
  [KIND.COMMAND_CAR]: COMMAND_CAR_BODY,
};
for (const [kind, expected] of Object.entries(rustClientConfig.bodies)) {
  const clientKind = kindByStableId.get(kind);
  assert(clientKind, `client KIND is missing ${kind}`);
  assertClientObjectFields(clientBodies[clientKind], expected, `${kind} body mirror`);
}

const clientUpgradeResearchTicks = {
  [UPGRADE.METHAMPHETAMINES]: METHAMPHETAMINES_RESEARCH_TICKS,
  [UPGRADE.ANTI_TANK_GUN_UNLOCK]: ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS,
  [UPGRADE.ARTILLERY_UNLOCK]: ARTILLERY_UNLOCK_RESEARCH_TICKS,
  [UPGRADE.TANK_UNLOCK]: TANK_UNLOCK_RESEARCH_TICKS,
  [UPGRADE.COMMAND_CAR_UNLOCK]: COMMAND_CAR_UNLOCK_RESEARCH_TICKS,
  [UPGRADE.MORTAR_AUTOCAST]: MORTAR_AUTOCAST_RESEARCH_TICKS,
};
for (const [upgradeId, expected] of Object.entries(rustClientConfig.upgrades)) {
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

for (const [abilityId, expected] of Object.entries(rustClientConfig.abilityEffects)) {
  const ability = abilityByStableId.get(abilityId);
  assert(ability, `client ABILITY is missing ${abilityId}`);
  assertClientObjectFields(ABILITIES[ability], expected, `${abilityId} ability effect mirror`);
}

console.log("faction catalog parity check passed");
