#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

import {
  ABILITIES,
  STATS,
  UPGRADES,
  WORKER_BUILDABLE,
} from "../client/src/config.js";
import { ABILITY, DEFAULT_FACTION_ID, KIND, UPGRADE } from "../client/src/protocol.js";

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
  false,
  "reserved future factions are not client-exposed until their catalog exists",
);
assert.deepEqual(
  WORKER_BUILDABLE,
  asClientKinds(rustCatalog.buildables.map((entry) => entry.kind)),
  "client worker build menu mirrors Rust faction catalog",
);

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
  assert.deepEqual(
    ABILITIES[ability]?.carriers ?? [],
    asClientKinds(entry.carriers),
    `${entry.id} carriers mirror Rust catalog`,
  );
}

for (const [kind, cost] of Object.entries(rustCatalog.costs)) {
  const clientKind = kindByStableId.get(kind);
  assert.deepEqual(STATS[clientKind]?.cost, cost, `${kind} cost mirrors Rust catalog`);
}

console.log("faction catalog parity check passed");
