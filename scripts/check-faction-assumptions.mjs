#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");

function read(relPath) {
  return fs.readFileSync(path.join(repoRoot, relPath), "utf8");
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function walk(dirRel, out = []) {
  const dir = path.join(repoRoot, dirRel);
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === "target" || entry.name === "node_modules") continue;
    const rel = path.posix.join(dirRel, entry.name);
    if (entry.isDirectory()) {
      walk(rel, out);
    } else {
      out.push(rel);
    }
  }
  return out;
}

const inventory = read("docs/design/faction-architecture-inventory.md");
for (const anchor of [
  "Compatibility Boundary",
  "Current Entity Identity",
  "Current Economy Shape",
  "Current Starting Loadout",
  "Current Tech Tree",
  "Current Ability Surface",
  "Current Client Command Cards",
  "Current AI Coupling",
  "Current Prediction And WASM Coupling",
  "Lifecycle Paths",
]) {
  assert(inventory.includes(`## ${anchor}`), `inventory is missing section: ${anchor}`);
}

const balanceDoc = read("docs/design/balance.md");
for (const anchor of [
  "### 5.0 Faction economy contract",
  "Approved direct Steel/Oil/Supply modules",
  "Generic resources are deferred.",
]) {
  assert(balanceDoc.includes(anchor), `balance design is missing resource policy note: ${anchor}`);
}

const phase = read("plans/faction/phase-0.md");
assert(
  phase.includes("Status: Done"),
  "phase 0 must only be marked Done after the faction assumption checker is wired in",
);

const matrix = read("plans/faction/lifecycle-matrix.md");
for (const pathName of [
  "Normal lobby start",
  "Quickstart/debug start",
  "AI add/remove/start",
  "Fixture/dev faction start",
  "Replay playback",
  "Replay branch staging/launch",
  "Dev scenarios",
  "Self-play",
  "Match history replay",
  "Spectator/no-fog view",
  "Post-match replay",
]) {
  assert(matrix.includes(`| ${pathName} |`), `lifecycle matrix missing row: ${pathName}`);
}
assert(!matrix.includes("| TBD |"), "lifecycle matrix should name owning checks instead of TBD");

const rulesKind = read("server/crates/rules/src/kind.rs");
assert(rulesKind.includes("pub const ALL: [EntityKind; 24]"), "EntityKind::ALL count changed");

const protocol = read("server/crates/protocol/src/lib.rs");
const clientProtocol = read("client/src/protocol.js");
for (const token of ["WORKER", "CITY_CENTRE", "STEEL", "OIL"]) {
  assert(protocol.includes(`pub const ${token}`), `Rust protocol missing ${token}`);
  assert(clientProtocol.includes(`${token}:`), `client protocol missing ${token}`);
}
assert(
  protocol.includes("COMPACT_SNAPSHOT_VERSION: u8 = 20") &&
    clientProtocol.includes("COMPACT_SNAPSHOT_VERSION = 20"),
  "compact snapshot version changed; update protocol parity and inventory deliberately",
);

const currentFactionSpecialCase = /\b(?:EntityKind|AbilityKind)::(?:Worker|CityCentre|Depot|Barracks|TrainingCentre|ResearchComplex|Factory|Steelworks|Steel|Oil|Tank|ScoutCar|CommandCar|MortarTeam|AntiTankGun|Artillery|Rifleman|MachineGunner|Smoke|MortarFire|PointFire|Breakthrough|Charge)\b|\b(?:STARTING_STEEL|STARTING_OIL|STARTING_WORKERS|QUICKSTART_STEEL|QUICKSTART_OIL)\b/;
const approvedCurrentFactionFiles = new Set([
  "server/crates/ai/src/ai_core/actions.rs",
  "server/crates/ai/src/ai_core/decision/defense.rs",
  "server/crates/ai/src/ai_core/decision/expansion.rs",
  "server/crates/ai/src/ai_core/decision/frontal.rs",
  "server/crates/ai/src/ai_core/decision/harassment.rs",
  "server/crates/ai/src/ai_core/decision/mod.rs",
  "server/crates/ai/src/ai_core/decision/production.rs",
  "server/crates/ai/src/ai_core/decision/proxy.rs",
  "server/crates/ai/src/ai_core/decision/raids.rs",
  "server/crates/ai/src/ai_core/decision/resources.rs",
  "server/crates/ai/src/ai_core/decision/trace.rs",
  "server/crates/ai/src/ai_core/facts.rs",
  "server/crates/ai/src/ai_core/observation.rs",
  "server/crates/ai/src/ai_core/profiles.rs",
  "server/crates/ai/src/ai_shared.rs",
  "server/crates/ai/src/selfplay/milestones.rs",
  "server/crates/ai/src/selfplay/pending_build.rs",
  "server/crates/ai/src/selfplay/player_view.rs",
  "server/crates/ai/src/selfplay/replay.rs",
  "server/crates/ai/src/selfplay/scripts.rs",
  "server/crates/rules/src/balance.rs",
  "server/crates/rules/src/combat.rs",
  "server/crates/rules/src/defs.rs",
  "server/crates/rules/src/economy.rs",
  "server/crates/rules/src/faction.rs",
  "server/crates/rules/src/kind.rs",
  "server/crates/sim/src/game/ability.rs",
  "server/crates/sim/src/game/artillery.rs",
  "server/crates/sim/src/game/building_memory.rs",
  "server/crates/sim/src/game/command.rs",
  "server/crates/sim/src/game/entity/entity.rs",
  "server/crates/sim/src/game/entity/store.rs",
  "server/crates/sim/src/game/fog.rs",
  "server/crates/sim/src/game/invariants.rs",
  "server/crates/sim/src/game/mod.rs",
  "server/crates/sim/src/game/mortar.rs",
  "server/crates/sim/src/game/player_state.rs",
  "server/crates/sim/src/game/services/ability_orders.rs",
  "server/crates/sim/src/game/services/combat/acquisition.rs",
  "server/crates/sim/src/game/services/combat/damage.rs",
  "server/crates/sim/src/game/services/combat/events.rs",
  "server/crates/sim/src/game/services/combat/mod.rs",
  "server/crates/sim/src/game/services/combat/projection.rs",
  "server/crates/sim/src/game/services/combat/weapons.rs",
  "server/crates/sim/src/game/services/commands.rs",
  "server/crates/sim/src/game/services/construction.rs",
  "server/crates/sim/src/game/services/economy.rs",
  "server/crates/sim/src/game/services/geometry.rs",
  "server/crates/sim/src/game/services/move_coordinator.rs",
  "server/crates/sim/src/game/services/movement/mod.rs",
  "server/crates/sim/src/game/services/movement/pivot_drive.rs",
  "server/crates/sim/src/game/services/movement/scout_car.rs",
  "server/crates/sim/src/game/services/movement/standability.rs",
  "server/crates/sim/src/game/services/movement/waypoints.rs",
  "server/crates/sim/src/game/services/order_queue.rs",
  "server/crates/sim/src/game/services/occupancy.rs",
  "server/crates/sim/src/game/services/pathing.rs",
  "server/crates/sim/src/game/services/production.rs",
  "server/crates/sim/src/game/services/standability.rs",
  "server/crates/sim/src/game/services/world_query.rs",
  "server/crates/sim/src/game/setup.rs",
  "server/crates/sim/src/game/setup/dev_scenarios.rs",
  "server/crates/sim/src/game/setup/dev_scenarios/layouts.rs",
  "server/crates/sim/src/game/systems.rs",
  "server/crates/sim/src/game/upgrade.rs",
  "server/crates/sim/src/protocol.rs",
  "server/crates/sim/src/rules/projection.rs",
  "server/src/dev_scenarios.rs",
  "server/src/lobby/room_task.rs",
  "server/src/protocol.rs",
]);

const sourceRoots = [
  "server/crates/ai/src",
  "server/crates/rules/src",
  "server/crates/sim/src",
  "server/src",
];
const offenders = [];
const approvedSpecialCaseBudgets = new Map([
  // Phase 10: the real Ekaterina catalog adds new explicit current/default catalog rows, and
  // commands.rs still carries in-file command-service tests that exercise cross-faction rejection.
  // Keep the ratchet explicit until those helpers shrink or test modules move out of counted files.
  ["server/crates/rules/src/faction.rs", 83],
  ["server/crates/rules/src/economy.rs", 98],
  ["server/crates/sim/src/game/setup.rs", 30],
  ["server/crates/sim/src/game/services/ability_orders.rs", 18],
  ["server/crates/sim/src/game/services/commands.rs", 234],
  ["server/crates/sim/src/game/invariants.rs", 13],
]);
const budgetOverruns = [];
for (const root of sourceRoots) {
  for (const relPath of walk(root)) {
    if (!relPath.endsWith(".rs")) continue;
    if (relPath.endsWith("/tests.rs") || relPath.endsWith("_tests.rs") || relPath.includes("/tests/")) {
      continue;
    }
    const source = read(relPath);
    const directSpecialCases = source.match(new RegExp(currentFactionSpecialCase.source, "g")) ?? [];
    if (approvedSpecialCaseBudgets.has(relPath) && directSpecialCases.length > approvedSpecialCaseBudgets.get(relPath)) {
      budgetOverruns.push(`${relPath}: ${directSpecialCases.length} > ${approvedSpecialCaseBudgets.get(relPath)}`);
    }
    if (directSpecialCases.length === 0) continue;
    if (!approvedCurrentFactionFiles.has(relPath)) offenders.push(relPath);
  }
}

assert(
  offenders.length === 0,
  `new current-faction special-case file(s) need catalog API or checker approval:\n${offenders.join("\n")}`,
);

assert(
  budgetOverruns.length === 0,
  `approved high-risk file(s) added direct current-faction special cases; route through catalog APIs or raise the budget deliberately:\n${budgetOverruns.join("\n")}`,
);

console.log("faction assumption inventory check passed");
