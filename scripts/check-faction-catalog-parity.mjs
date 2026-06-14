#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

import {
  ABILITIES,
  FACTION_CATALOGS,
  STATS,
  UPGRADES,
  WORKER_BUILDABLE,
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
assert.equal(
  allRustCatalogs.catalogs.some((catalog) => catalog.id === "ekaterina"),
  true,
  "Ekaterina catalog is exposed once the Phase 10 slice exists",
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

console.log("faction catalog parity check passed");
