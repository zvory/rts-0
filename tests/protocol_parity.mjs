// tests/protocol_parity.mjs
// Guard compact wire vocabularies against Rust encoder / JS decoder drift.

import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import {
  ABILITY_CODE,
  ABILITY,
  ABILITY_OBJECT_KIND,
  ABILITY_OBJECT_KIND_CODE,
  CMD,
  EVENT,
  EVENT_CODE,
  KIND,
  KIND_CODE,
  LAB_ROLE,
  LAB_VISION,
  NOTICE_SEVERITY,
  NOTICE_SEVERITY_CODE,
  ORDER_STAGE,
  ORDER_STAGE_CODE,
  C,
  S,
  SETUP,
  SETUP_CODE,
  STATE,
  STATE_CODE,
  TERRAIN,
  UPGRADE,
  UPGRADE_CODE,
  COMPACT_SNAPSHOT_VERSION,
  PREDICTION_PROTOCOL_VERSION,
  DEFAULT_FACTION_ID,
  cmd,
  decodeServerMessage,
  msg,
} from "../client/src/protocol.js";
import { PLAYER_PALETTE } from "../client/src/config.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const rustProtocolPath = path.join(repoRoot, "server/crates/protocol/src/lib.rs");
const rust = fs.readFileSync(rustProtocolPath, "utf8");
const rustContractPath = path.join(repoRoot, "server/crates/contract/src/lib.rs");
const rustContract = fs.readFileSync(rustContractPath, "utf8");
const rustLobbyPath = path.join(repoRoot, "server/src/lobby/mod.rs");
const rustLobby = fs.readFileSync(rustLobbyPath, "utf8");
const protocolDocPath = path.join(repoRoot, "docs/design/protocol.md");
const protocolDoc = fs.readFileSync(protocolDocPath, "utf8");
const protocolContract = JSON.parse(
  execFileSync(
    "cargo",
    [
      "run",
      "--manifest-path",
      "server/Cargo.toml",
      "-p",
      "rts-protocol",
      "--bin",
      "dump-protocol-contract",
      "--quiet",
    ],
    { cwd: repoRoot, encoding: "utf8" },
  ),
);

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function extractRustPlayerPalette() {
  const match = rustLobby.match(/const\s+PLAYER_PALETTE\s*:\s*\[&str;\s*\d+\]\s*=\s*\[([\s\S]*?)\];/);
  assert(match, "missing Rust player palette");
  return Array.from(match[1].matchAll(/"([^"]+)"/g), (entry) => entry[1]);
}

function assertSameMap(label, rustValues, jsValues) {
  const actual = Object.fromEntries(Object.entries(jsValues).sort(([a], [b]) => a.localeCompare(b)));
  const expected = Object.fromEntries(Object.entries(rustValues).sort(([a], [b]) => a.localeCompare(b)));
  assert(
    JSON.stringify(actual) === JSON.stringify(expected),
    `${label} code map mismatch\nRust: ${JSON.stringify(expected)}\nJS:   ${JSON.stringify(actual)}`,
  );
}

function assertSameCodes(label, rustCodes, jsCodes) {
  assertSameMap(label, rustCodes, jsCodes);
  const actual = Object.fromEntries(Object.entries(jsCodes).sort(([a], [b]) => a.localeCompare(b)));
  assertNoDuplicateCodes(label, actual);
}

function assertNoDuplicateCodes(label, codes) {
  const seen = new Map();
  for (const [name, code] of Object.entries(codes)) {
    assert(Number.isInteger(code), `${label}.${name} code must be an integer`);
    assert(
      code !== protocolContract.unknownCodeSentinel,
      `${label}.${name} must not use the unknown sentinel code`,
    );
    assert(!seen.has(code), `${label} code ${code} is reused by ${seen.get(code)} and ${name}`);
    seen.set(code, name);
  }
}

function extractMarkdownCodeRow(label) {
  const escaped = label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const row = protocolDoc.match(new RegExp(`^\\| \`${escaped}\` \\| ([^|]+) \\|$`, "m"));
  assert(row, `protocol docs must list compact ${label} codes`);
  return Object.fromEntries(
    Array.from(row[1].matchAll(/(\d+)\s+`([^`]+)`/g), ([, code, name]) => [name, Number(code)]),
  );
}

function assertDocsCodeTable(label, rustCodes) {
  assertSameMap(`docs compact ${label}`, rustCodes, extractMarkdownCodeRow(label));
}

assert(protocolContract.schemaVersion === 1, "protocol contract schema version must be 1");
assert(protocolContract.unknownCodeSentinel === 255, "unknown compact code sentinel must stay 255");
assertSameMap("server message tags", protocolContract.messageTags.server, S);
assertSameMap("client message tags", protocolContract.messageTags.client, C);
assertSameMap("command tags", protocolContract.commandTags, CMD);
assertSameMap("kind vocabulary", protocolContract.vocabularies.kinds, KIND);
assertSameMap("state vocabulary", protocolContract.vocabularies.states, STATE);
assertSameMap("setup vocabulary", protocolContract.vocabularies.setupStates, SETUP);
assertSameMap("event vocabulary", protocolContract.vocabularies.events, EVENT);
assertSameMap("ability vocabulary", protocolContract.vocabularies.abilities, ABILITY);
assertSameMap(
  "ability object kind vocabulary",
  protocolContract.vocabularies.abilityObjectKinds,
  ABILITY_OBJECT_KIND,
);
assertSameMap("upgrade vocabulary", protocolContract.vocabularies.upgrades, UPGRADE);
assertSameMap("notice severity vocabulary", protocolContract.vocabularies.noticeSeverities, NOTICE_SEVERITY);
assertSameMap("order stage vocabulary", protocolContract.vocabularies.orderStages, ORDER_STAGE);
assertSameMap("terrain", protocolContract.compactCodes.terrain, TERRAIN);
assertSameCodes("entity kind", protocolContract.compactCodes.kind, KIND_CODE);
assertSameCodes("entity state", protocolContract.compactCodes.state, STATE_CODE);
assertSameCodes("setup state", protocolContract.compactCodes.setupState, SETUP_CODE);
assertSameCodes("event", protocolContract.compactCodes.event, EVENT_CODE);
assertSameCodes("order stage", protocolContract.compactCodes.orderStage, ORDER_STAGE_CODE);
assertSameCodes("ability", protocolContract.compactCodes.ability, ABILITY_CODE);
assertSameCodes("ability object kind", protocolContract.compactCodes.abilityObjectKind, ABILITY_OBJECT_KIND_CODE);
assertSameCodes("upgrade", protocolContract.compactCodes.upgrade, UPGRADE_CODE);
assertSameCodes("notice severity", protocolContract.compactCodes.noticeSeverity, NOTICE_SEVERITY_CODE);
assertSameCodes(
  "resource kind",
  protocolContract.compactCodes.resourceKind,
  { [KIND.STEEL]: KIND_CODE[KIND.STEEL], [KIND.OIL]: KIND_CODE[KIND.OIL] },
);
assert(protocolContract.compactSnapshotVersion === COMPACT_SNAPSHOT_VERSION, "compact snapshot version must match Rust");
assert(
  protocolDoc.includes(`compact JSON text, version ${COMPACT_SNAPSHOT_VERSION}`) &&
    protocolDoc.includes(`"v": ${COMPACT_SNAPSHOT_VERSION}`),
  "protocol docs must list the current compact snapshot version",
);
assert(
  protocolContract.predictionProtocolVersion === PREDICTION_PROTOCOL_VERSION,
  "prediction protocol version must match Rust",
);
assert(protocolContract.defaultFactionId === DEFAULT_FACTION_ID, "default faction id must match Rust");
assert(
  protocolContract.compactSlotSchemas.snapshot.map((field) => field.name).join(",") ===
    "tick,steel,oil,supplyUsed,supplyCap",
  "compact snapshot scalar schema must match JS decoder",
);
assert(
  protocolContract.compactSlotSchemas.entity.at(-1).name === "buildActive",
  "compact entity slot schema must include the latest appended field",
);
assert(
  protocolContract.compactSlotSchemas.abilityObject.length === 9,
  "compact ability object slot count must match JS decoder",
);
assert(
  protocolContract.compactSlotSchemas.netStatus.length === 8,
  "compact net status slot count must match JS decoder",
);
assertDocsCodeTable("kind", protocolContract.compactCodes.kind);
assertDocsCodeTable("state", protocolContract.compactCodes.state);
assertDocsCodeTable("setupState", protocolContract.compactCodes.setupState);
assertDocsCodeTable("orderStage", protocolContract.compactCodes.orderStage);
assertDocsCodeTable("ability", protocolContract.compactCodes.ability);
assertDocsCodeTable("abilityObject.kind", protocolContract.compactCodes.abilityObjectKind);
assertDocsCodeTable("upgrade", protocolContract.compactCodes.upgrade);
assertDocsCodeTable("notice.severity", protocolContract.compactCodes.noticeSeverity);

assert(
  JSON.stringify(extractRustPlayerPalette()) === JSON.stringify(PLAYER_PALETTE),
  "client PLAYER_PALETTE must match server lobby PLAYER_PALETTE",
);

assert(
  JSON.stringify(msg.command({ c: "stop", units: [1] }, 9)) === JSON.stringify({ t: "command", clientSeq: 9, cmd: { c: "stop", units: [1] } }),
  "command builder must emit clientSeq envelope",
);
assert(
  JSON.stringify(msg.command(cmd.holdPosition([1]), 10)) === JSON.stringify({ t: "command", clientSeq: 10, cmd: { c: "holdPosition", units: [1] } }),
  "holdPosition command builder must emit clientSeq envelope",
);
assert(
  // Temporary source-text allowlist: start payload field-shape assertions are contract DTO checks,
  // not part of this phase's structured protocol export.
  rustContract.includes("prediction_build_id") && rustContract.includes("prediction_version"),
  "start payload must expose prediction compatibility metadata",
);
assert(
  rustContract.includes("LabStartMetadata") &&
    rustContract.includes("operator_id") &&
    rustContract.includes("operation_count") &&
    LAB_ROLE.OPERATOR === "operator" &&
    LAB_ROLE.READ_ONLY === "readOnly" &&
    LAB_VISION.FULL_WORLD === "fullWorld",
  "start payload must expose mirrored lab metadata",
);
assert(
  rustContract.includes("DEFAULT_FACTION_ID") &&
    rustContract.includes("faction_id") &&
    DEFAULT_FACTION_ID === "kriegsia",
  "start/player contract must expose the canonical default faction id",
);
assert(
  C.SET_TEAM_PRESET === "setTeamPreset",
  "setTeamPreset client message tag must match Rust",
);
assert(
  JSON.stringify(msg.setTeamPreset("2v2")) === JSON.stringify({ t: "setTeamPreset", preset: "2v2" }),
  "setTeamPreset builder must emit the exact wire shape",
);
assert(
  C.SET_TEAM === "setTeam",
  "setTeam client message tag must match Rust",
);
assert(
  JSON.stringify(msg.setTeam(7, 2)) === JSON.stringify({ t: "setTeam", id: 7, teamId: 2 }),
  "setTeam builder must emit the exact wire shape",
);
assert(
  C.SET_SPECTATOR === "setSpectator",
  "setSpectator client message tag must match Rust",
);
assert(
  JSON.stringify(msg.setSpectator(true)) === JSON.stringify({ t: "setSpectator", spectator: true }),
  "setSpectator builder must preserve the self-targeting wire shape",
);
assert(
  JSON.stringify(msg.setSpectator(true, 7)) === JSON.stringify({ t: "setSpectator", spectator: true, id: 7 }),
  "setSpectator builder must support optional host-targeted id",
);
assert(
  C.SET_FACTION === "setFaction",
  "setFaction client message tag must match Rust",
);
assert(
  JSON.stringify(msg.setFaction("ekat")) === JSON.stringify({ t: "setFaction", factionId: "ekat" }),
  "setFaction builder must emit the exact wire shape",
);
assert(
  JSON.stringify(msg.addAi(2)) === JSON.stringify({ t: "addAi", teamId: 2 }),
  "addAi builder must support optional teamId",
);
const decodedAck = decodeServerMessage({
  t: "snapshot",
  v: COMPACT_SNAPSHOT_VERSION,
  s: [12, 75, 0, 4, 10],
  e: [],
  ao: [[70, 1, 6, 1, 384, 416, 90, 7, [45, null, null, null, null, null]]],
  n: [1, 2, 0, 3, 4, PREDICTION_PROTOCOL_VERSION, 7, 12],
});
assert(decodedAck.netStatus.predictionVersion === PREDICTION_PROTOCOL_VERSION, "compact predictionVersion decodes");
assert(decodedAck.netStatus.lastSimConsumedClientSeq === 7, "compact consumed client seq decodes");
assert(decodedAck.netStatus.lastSimConsumedClientTick === 12, "compact consumed client tick decodes");
assert(decodedAck.abilityObjects[0].kind === "returnMarker", "compact ability object kind decodes");
assert(decodedAck.abilityObjects[0].sourceCasterId === 7, "compact ability source caster decodes");
assert(
  decodedAck.abilityObjects[0].ownerState.earliestReturnTick === 45,
  "compact ability owner state decodes",
);
assert(
  C.REQUEST_REPLAY_BRANCH === "requestReplayBranch",
  "requestReplayBranch client message tag must match Rust",
);
assert(
  JSON.stringify(msg.requestReplayBranch()) === JSON.stringify({ t: "requestReplayBranch" }),
  "requestReplayBranch builder must emit the exact wire shape",
);
assert(
  C.CLAIM_BRANCH_SEAT === "claimBranchSeat",
  "claimBranchSeat client message tag must match Rust",
);
assert(
  JSON.stringify(msg.claimBranchSeat(7)) === JSON.stringify({ t: "claimBranchSeat", playerId: 7 }),
  "claimBranchSeat builder must emit the exact wire shape",
);
assert(
  C.RELEASE_BRANCH_SEAT === "releaseBranchSeat",
  "releaseBranchSeat client message tag must match Rust",
);
assert(
  JSON.stringify(msg.releaseBranchSeat(7)) === JSON.stringify({ t: "releaseBranchSeat", playerId: 7 }),
  "releaseBranchSeat builder must emit the exact wire shape",
);
assert(
  C.START_BRANCH === "startBranch",
  "startBranch client message tag must match Rust",
);
assert(
  JSON.stringify(msg.startBranch()) === JSON.stringify({ t: "startBranch" }),
  "startBranch builder must emit the exact wire shape",
);
assert(
  S.REPLAY_BRANCH_CREATED === "replayBranchCreated",
  "replayBranchCreated server message tag must match Rust",
);
// Temporary source-text allowlist: branch and observer-analysis payload field lists are still
// DTO shape smoke checks until a later phase expands structured contract-field exports.
for (const field of ["branch_room", "source_tick", "seats", "player_id", "team_id", "faction_id", "claimable"]) {
  assert(rust.includes(field), `replayBranchCreated Rust contract is missing ${field}`);
}
assert(
  S.BRANCH_STAGING === "branchStaging",
  "branchStaging server message tag must match Rust",
);
for (const field of ["host_id", "team_id", "faction_id", "claimant_id", "occupants", "can_start"]) {
  assert(rust.includes(field), `branchStaging Rust contract is missing ${field}`);
}
assert(
  S.REPLAY_ANALYSIS === "replayAnalysis",
  "replayAnalysis server message tag must match Rust",
);
for (const field of ["units_lost", "resources_lost", "steel_value", "oil_value", "queue_depth"]) {
  assert(rust.includes(field), `replayAnalysis Rust contract is missing ${field}`);
}
const observerAnalysis = decodeServerMessage({
  t: S.REPLAY_ANALYSIS,
  tick: 9,
  players: [{
    id: 1,
    units: [{ kind: "worker", count: 2, steelValue: 100, oilValue: 0 }],
    production: [{ buildingId: 7, buildingKind: "city_centre", itemKind: "worker", itemType: "unit", progress: 0.25, queueDepth: 1 }],
    unitsLost: [],
    resourcesLost: { steel: 0, oil: 0 },
  }],
});
assert(observerAnalysis.t === "replayAnalysis" && observerAnalysis.players[0].production[0].queueDepth === 1, "replayAnalysis passes through decode");

console.log("✅ protocol_parity.mjs: Rust protocol contract dump matches JS mirror");
