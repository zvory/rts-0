#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");

function read(relPath, label = relPath) {
  const absPath = path.join(repoRoot, relPath);
  try {
    return fs.readFileSync(absPath, "utf8");
  } catch (err) {
    if (err?.code === "ENOENT") {
      throw new Error(
        `${label} is missing at ${relPath}. Update scripts/check-faction-assumptions.mjs if the active source of truth moved.`,
      );
    }
    throw err;
  }
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertIncludes(source, text, label) {
  const normalizedSource = source.replace(/\s+/g, " ");
  const normalizedText = text.replace(/\s+/g, " ");
  assert(
    source.includes(text) || normalizedSource.includes(normalizedText),
    `${label} is missing required faction boundary text: ${text}`,
  );
}

function assertNotIncludes(source, text, label) {
  assert(!source.includes(text), `${label} contains stale or contradictory faction boundary text: ${text}`);
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
  "Active Faction Boundary Sources",
  "Catalog Id Statuses",
  "Current Entity Identity",
  "Current Economy Shape",
  "Current Starting Loadout",
  "Current Tech Tree",
  "Current Ability Surface",
  "Current Client Command Cards",
  "Current AI Coupling",
  "Current Prediction And WASM Coupling",
  "Lifecycle Paths",
  "Current Guardrail Checks",
  "Guardrail Map For Future Faction Work",
]) {
  assert(
    inventory.includes(`## ${anchor}`),
    `docs/design/faction-architecture-inventory.md is missing section: ## ${anchor}`,
  );
}

for (const requiredText of [
  "Lifecycle status is explicit and separate from catalog existence",
  "`playable-human-only`: allowed for human selection but not for AI, prediction, or replay-capable",
  "`reserved/future`: named for future work but not admitted",
  "`plans/archive/faction/*` files are historical-only evidence",
  "they must not read archived phase files as the source",
  "| `kriegsia` | playable |",
  "| `ekat` | playable |",
  "| `phase2_empty_fixture` | test-fixture-only |",
  "| `plans/archive/faction/*` | historical-only |",
  "The Rust faction catalog in",
  "Kriegsia and Ekat command ids are namespaced by faction",
  "Public AI seats default to `kriegsia`; no public Ekat selector",
  "Enabled only for local Kriegsia",
  "When faction behavior changes, update the owning source and its guard in the same change",
  "Do not import lifecycle policy, status tables, checker allowlists, or source paths from archived plan files",
]) {
  assertIncludes(inventory, requiredText, "docs/design/faction-architecture-inventory.md");
}
for (const staleText of [
  "Phase 6 should make the faction-aware Rust ability registry",
  "Until Phase 2 adds a generated or mechanically checked catalog mirror",
  "Phase 10 client catalog path",
]) {
  assertNotIncludes(inventory, staleText, "docs/design/faction-architecture-inventory.md");
}

const archivedFactionDir = path.join(repoRoot, "plans/archive/faction");
assert(
  fs.existsSync(archivedFactionDir) && fs.statSync(archivedFactionDir).isDirectory(),
  "plans/archive/faction must exist as historical-only evidence; do not point active lifecycle checks at moved plan files",
);

const balanceDoc = read("docs/design/balance.md");
for (const anchor of [
  "### 5.0 Faction economy contract",
  "Approved direct Steel/Oil/Supply modules",
  "Generic resources are deferred.",
  "Catalog existence is not lifecycle admission.",
  "reserved/future ids must not inherit Kriegsia economy behavior",
]) {
  assert(balanceDoc.includes(anchor), `balance design is missing resource policy note: ${anchor}`);
}

const protocolDoc = read("docs/design/protocol.md");
for (const anchor of [
  "Protocol vocabulary is not lifecycle admission",
  "Fixture-only, reserved/future, and historical-only ids must not become valid `setFaction`",
  "the `phase2_empty_fixture` test fixture",
]) {
  assertIncludes(protocolDoc, anchor, "docs/design/protocol.md");
}

const clientUiDoc = read("docs/design/client-ui.md");
for (const anchor of [
  "The client mirror is a checked projection, not lifecycle admission",
  "fixture-only ids remain test harness data",
  "public AI controls do not expose a faction selector",
  "local prediction remains disabled for unsupported local faction ids",
]) {
  assertIncludes(clientUiDoc, anchor, "docs/design/client-ui.md");
}
assertNotIncludes(clientUiDoc, "Phase 10 may keep this checked mirror", "docs/design/client-ui.md");

for (const pathName of [
  "Normal lobby start",
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
  assert(inventory.includes(`| ${pathName} |`), `inventory lifecycle table missing row: ${pathName}`);
}
assert(!inventory.includes("| TBD |"), "inventory lifecycle table should name owning checks instead of TBD");

const rulesKind = read("server/crates/rules/src/kind.rs");
const allCount = rulesKind.match(/pub const ALL: \[EntityKind; (\d+)\]/);
const allBody = rulesKind.match(/pub const ALL: \[EntityKind; \d+\] = \[([\s\S]*?)\];/);
assert(allCount && allBody, "EntityKind::ALL declaration shape changed; update faction assumption parser");
const allEntries = allBody[1].match(/EntityKind::[A-Za-z]+/g) ?? [];
assert(
  Number(allCount[1]) === allEntries.length,
  `EntityKind::ALL count ${allCount[1]} does not match listed entries ${allEntries.length}`,
);
assert(
  inventory.includes(`The current roster has ${allEntries.length} global kinds`),
  `inventory must document current EntityKind::ALL count ${allEntries.length}`,
);

const protocol = [
  read("server/crates/protocol/src/lib.rs"),
  read("server/crates/protocol/src/contract_metadata.rs"),
].join("\n");
const clientProtocol = [
  read("client/src/protocol.js"),
  read("client/src/protocol_constants.js"),
].join("\n");
for (const token of ["WORKER", "GOLEM", "EKAT", "CITY_CENTRE", "ZAMOK", "STEEL", "OIL"]) {
  assert(protocol.includes(`pub const ${token}`), `Rust protocol missing ${token}`);
  assert(clientProtocol.includes(`${token}:`), `client protocol missing ${token}`);
}
const rustCompactVersion = protocol.match(/COMPACT_SNAPSHOT_VERSION: u8 = (\d+)/)?.[1];
const clientCompactVersion = clientProtocol.match(/COMPACT_SNAPSHOT_VERSION = (\d+)/)?.[1];
assert(
  rustCompactVersion && clientCompactVersion,
  "compact snapshot version declaration missing in Rust or client protocol mirror",
);
assert(
  rustCompactVersion === clientCompactVersion,
  `compact snapshot version mismatch: Rust ${rustCompactVersion}, client ${clientCompactVersion}`,
);

const currentFactionSpecialCase = /\b(?:EntityKind|AbilityKind)::(?:Worker|Golem|CityCentre|Depot|Barracks|TrainingCentre|ResearchComplex|Factory|Steelworks|Steel|Oil|Tank|ScoutCar|CommandCar|MortarTeam|AntiTankGun|Artillery|Rifleman|MachineGunner|Smoke|MortarFire|PointFire|Breakthrough|Charge)\b|\b(?:STARTING_STEEL|STARTING_OIL|STARTING_WORKERS)\b/;
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
  // Turtle AI is a Kriegsia-only current-roster profile until AI faction catalog routing exists.
  "server/crates/ai/src/ai_core/decision/turtle.rs",
  "server/crates/ai/src/ai_core/profiles/turtle.rs",
  "server/crates/ai/src/ai_core/facts.rs",
  "server/crates/ai/src/ai_core/observation.rs",
  "server/crates/ai/src/ai_core/profiles.rs",
  // Kriegsia-only AI resource scanner; non-Kriegsia AI remains unsupported by public lobby flow.
  "server/crates/ai/src/ai_core/resource_availability.rs",
  "server/crates/ai/src/ai_shared.rs",
  "server/crates/ai/src/selfplay/milestones.rs",
  "server/crates/ai/src/selfplay/pending_build.rs",
  "server/crates/ai/src/selfplay/player_view.rs",
  "server/crates/ai/src/selfplay/replay.rs",
  "server/crates/ai/src/selfplay/scripts.rs",
  "server/crates/rules/src/balance.rs",
  // Balance Phase 4 split: these modules contain constants/helpers moved from balance.rs without
  // changing current-faction ownership.
  "server/crates/rules/src/balance/economy.rs",
  // Entrenchment Phase 1 names the current eligible infantry set for the first feature pass; the
  // helper is the rules API that downstream sim/client mirrors consume.
  "server/crates/rules/src/balance/entrenchment.rs",
  "server/crates/rules/src/balance/stats.rs",
  // Catalog dump tool projects current catalog stats for parity checks.
  "server/crates/rules/src/bin/dump-faction-catalog.rs",
  "server/crates/rules/src/combat.rs",
  "server/crates/rules/src/defs.rs",
  "server/crates/rules/src/economy.rs",
  "server/crates/rules/src/faction.rs",
  "server/crates/rules/src/kind.rs",
  // Target facts centralize current-roster target groups until catalog combat roles exist.
  "server/crates/rules/src/target.rs",
  "server/crates/sim/src/game/ability.rs",
  // Entrenchment Phase 3 gates trench creation on the current player's Entrenchment research.
  "server/crates/sim/src/game/services/entrenchment.rs",
  // Inline projectile tests spawn current units directly.
  "server/crates/sim/src/game/ability_projectile.rs",
  "server/crates/sim/src/game/artillery.rs",
  "server/crates/sim/src/game/building_memory.rs",
  "server/crates/sim/src/game/command.rs",
  "server/crates/sim/src/game/entity/entity.rs",
  "server/crates/sim/src/game/entity/store.rs",
  "server/crates/sim/src/game/fog.rs",
  "server/crates/sim/src/game/invariants.rs",
  // Lab API tests stage current catalog entities while runtime validation stays catalog-routed.
  "server/crates/sim/src/game/lab.rs",
  // Lab scenario setup import/export preserves existing support-weapon setup states for current catalog kinds.
  "server/crates/sim/src/game/lab/orientation.rs",
  "server/crates/sim/src/game/lab/scenario.rs",
  "server/crates/sim/src/game/mod.rs",
  "server/crates/sim/src/game/mortar.rs",
  // Delayed Panzerfaust resolution and its inline regression test cover the current Tank target.
  "server/crates/sim/src/game/panzerfaust_shot.rs",
  "server/crates/sim/src/game/player_state.rs",
  "server/crates/sim/src/game/services/ability_orders.rs",
  "server/crates/sim/src/game/services/combat/acquisition.rs",
  // Combat target legality centralizes the existing Mortar Team indirect-fire exception moved
  // out of acquisition.rs; it does not expand faction admission or target policy.
  "server/crates/sim/src/game/services/combat/target_legality.rs",
  "server/crates/sim/src/game/services/combat/damage.rs",
  "server/crates/sim/src/game/services/combat/events.rs",
  "server/crates/sim/src/game/services/combat/mod.rs",
  // Tank coax is a Tank-only secondary weapon until catalog combat roles/weapon slots exist.
  "server/crates/sim/src/game/services/combat/coax.rs",
  // Panzerfaust research installs Rifleman-specific disposable-weapon behavior; the global
  // catalog still owns Rifleman admission while this service owns its loaded-shot runtime.
  "server/crates/sim/src/game/services/combat/panzerfaust.rs",
  // Declarative combat target policy owns current-roster ranking groups until catalog combat roles exist.
  "server/crates/sim/src/game/services/combat/target_policy.rs",
  // Default combat target policy is still current-roster based until catalog combat roles exist.
  "server/crates/sim/src/game/services/combat/priority.rs",
  "server/crates/sim/src/game/services/combat/projection.rs",
  "server/crates/sim/src/game/services/combat/weapons.rs",
  "server/crates/sim/src/game/services/commands.rs",
  // Command helper extraction preserves existing support-weapon setup timing and staged-state
  // cleanup special cases moved out of commands.rs; it does not expand faction admission.
  "server/crates/sim/src/game/services/commands/command_helpers.rs",
  // Command-budget guard extraction keeps the existing Command Car cap special case.
  "server/crates/sim/src/game/services/commands/guards.rs",
  // Command planner fact extraction keeps existing current-catalog capability facts split out of commands.rs.
  "server/crates/sim/src/game/services/commands/planner_facts.rs",
  "server/crates/sim/src/game/services/construction.rs",
  "server/crates/sim/src/game/services/economy.rs",
  // Pump Jack extraction is a deliberate current-catalog oil special case until generic resource
  // extractors exist in catalog data.
  "server/crates/sim/src/game/services/economy/pump_jack.rs",
  "server/crates/sim/src/game/services/geometry.rs",
  "server/crates/sim/src/game/services/move_coordinator.rs",
  "server/crates/sim/src/game/services/movement/mod.rs",
  // Armor-reaction runtime eligibility is rules-routed; focused inline tests use the Tank entry.
  "server/crates/sim/src/game/services/movement/armor_reaction.rs",
  "server/crates/sim/src/game/services/movement/pivot_drive.rs",
  "server/crates/sim/src/game/services/movement/scout_car.rs",
  "server/crates/sim/src/game/services/movement/standability.rs",
  "server/crates/sim/src/game/services/movement/waypoints.rs",
  // Setup and artillery command execution is still global-kind based.
  "server/crates/sim/src/game/services/order_execution.rs",
  "server/crates/sim/src/game/services/order_queue.rs",
  // Queued direct attacks account for the Rifleman's optional loaded Panzerfaust cycle.
  "server/crates/sim/src/game/services/order_queue/attack.rs",
  "server/crates/sim/src/game/services/occupancy.rs",
  "server/crates/sim/src/game/services/pathing.rs",
  "server/crates/sim/src/game/services/production.rs",
  // Scout Plane sorties still use a dedicated runtime kind outside normal ground-unit systems.
  "server/crates/sim/src/game/services/scout_plane.rs",
  "server/crates/sim/src/game/services/standability.rs",
  // Pump Jack placement is deliberately constrained to current oil resource nodes.
  "server/crates/sim/src/game/services/standability/pump_jack.rs",
  "server/crates/sim/src/game/services/world_query.rs",
  // Inline occupancy cache coverage uses a representative current-catalog Rifleman.
  "server/crates/sim/src/game/systems/occupancy_phase_cache.rs",
  "server/crates/sim/src/game/setup.rs",
  "server/crates/sim/src/game/setup/dev_scenarios.rs",
  // Attack-move reload acquisition deliberately reproduces current Tank behavior; public faction
  // admission remains routed through the catalog-aware dev scenario launcher.
  "server/crates/sim/src/game/setup/dev_scenarios/attack_move_reload.rs",
  // Dynamic construction path-block is an intentionally Kriegsia-specific worker/building
  // fixture; public faction admission remains routed through the dev scenario launcher.
  "server/crates/sim/src/game/setup/dev_scenarios/dynamic_construction_path_block.rs",
  // Replay-derived Factory fixture intentionally enumerates today's vehicle roster; public
  // faction admission remains routed through the catalog-aware dev scenario launcher.
  "server/crates/sim/src/game/setup/dev_scenarios/factory_wall_rally_spawn.rs",
  // Replay-derived vehicle-lock fixture intentionally recreates a current Kriegsia formation and
  // base landmark; public faction admission remains routed through the dev scenario launcher.
  "server/crates/sim/src/game/setup/dev_scenarios/replay_142_vehicle_lock.rs",
  // Tank coax inspection deliberately seeds current roster targets as a no-fog dev fixture;
  // public faction admission still routes through the dev scenario launcher.
  "server/crates/sim/src/game/setup/dev_scenarios/tank_coax.rs",
  "server/crates/sim/src/game/setup/dev_scenarios/layouts.rs",
  "server/crates/sim/src/game/setup/dev_scenarios/layouts/tank_traps.rs",
  "server/crates/sim/src/game/systems.rs",
  "server/crates/sim/src/game/upgrade.rs",
  "server/crates/sim/src/protocol.rs",
  "server/crates/sim/src/rules/projection.rs",
  "server/src/dev_scenarios.rs",
  // This catalog entry is an intentionally Kriegsia-specific Command Car corner fixture; public
  // faction admission remains routed through the catalog-aware dev scenario launcher.
  "server/src/dev_scenarios/command_car_corner.rs",
  // The frame-budget hellhole generator deliberately materializes a fixed current-roster Kriegsia
  // Lab checkpoint for benchmarking; runtime faction admission remains outside this offline tool.
  "server/src/bin/generate_supply_300_hellhole.rs",
  // The matching Lab driver deliberately churns that fixed current-roster stress fixture.
  "server/src/lobby/lab_scenario_driver.rs",
  "server/src/lobby/room_task.rs",
  "server/src/protocol.rs",
  // Deterministic Hellhole stress fixtures intentionally construct the current Kriegsia roster.
  "server/src/tools/hellhole_spec.rs",
  // Server-rendered wiki examples intentionally cite current catalog kinds.
  "server/src/wiki.rs",
]);

const sourceRoots = [
  "server/crates/ai/src",
  "server/crates/rules/src",
  "server/crates/sim/src",
  "server/src",
];
const offenders = [];
const approvedSpecialCaseBudgets = new Map([
  // Phase 7 final audit: catalog, economy helper, and command-service references intentionally
  // grew during the fixture, ability, and client-surface phases; keep the ratchet explicit until
  // those helpers shrink or move behind catalog APIs.
  // Tank Trap phases add current-catalog construction eligibility and gameplay command handling
  // for a default-faction obstacle before a broader catalog API can absorb obstacle placement.
  // Artillery Fire Control adds one R&D catalog entry for the current Kriegsia tech tree.
  // Scout Plane ability adds a Command Car carrier to the catalog.
  // Entrenchment Phase 1 adds one Training Centre research entry and its negative Ekat assertion.
  // Stewardship Phase 3 moved ability and upgrade identity into this authoritative catalog owner;
  // the typed enum declarations and catalog rows intentionally raise the owner-file baseline.
  ["server/crates/rules/src/faction.rs", 120],
  ["server/crates/rules/src/economy.rs", 109],
  ["server/crates/sim/src/game/setup.rs", 30],
  ["server/crates/sim/src/game/services/ability_orders.rs", 18],
  // Tank Trap deconstruction adds worker-only command validation and trap target admission.
  ["server/crates/sim/src/game/services/commands.rs", 259],
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
