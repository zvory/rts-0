// tests/protocol_parity.mjs
// Guard compact wire vocabularies against Rust encoder / JS decoder drift.

import { MAX_COMPACT_ORDER_PLAN } from "../client/src/protocol_constants.js";
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { CLIENT_NET_REPORT_FIELDS } from "./client_net_report_fields.mjs";

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
  LAB_CHECKPOINT_SCENARIO,
  LAB_REPLAY,
  LAB_ROLE,
  LAB_VISION,
  LOBBY_KIND,
  MOVEMENT_PATH_DIAGNOSTICS,
  NOTICE,
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
  PASSABLE,
  ROAD_TERRAIN_CODES,
  TERRAIN,
  UPGRADE,
  UPGRADE_CODE,
  WEAPON_KIND,
  WEAPON_KIND_CODE,
  COMPACT_SNAPSHOT_VERSION,
  MAX_COMPACT_GROUND_DECALS,
  SNAPSHOT_CODEC,
  SNAPSHOT_CODEC_VERSION,
  SNAPSHOT_FRAME_KIND,
  PREDICTION_PROTOCOL_VERSION,
  DEFAULT_FACTION_ID,
  cmd,
  decodeServerMessage,
  msg,
} from "../client/src/protocol.js";
import * as protocolExports from "../client/src/protocol.js";
import { DOODAD_TYPE, DOODAD_TYPE_IDS, PLAYER_PALETTE } from "../client/src/config.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const rustProtocolPath = path.join(repoRoot, "server/crates/protocol/src/lib.rs");
const rustProtocolLabReplayPath = path.join(repoRoot, "server/crates/protocol/src/lab_replay.rs");
const rustProtocolLabScenarioPath = path.join(repoRoot, "server/crates/protocol/src/lab_scenario.rs");
const rustProtocolObserverAnalysisPath = path.join(repoRoot, "server/crates/protocol/src/observer_analysis.rs");
const rustProtocolServerMessagePath = path.join(repoRoot, "server/crates/protocol/src/server_message.rs");
const rust = [
  fs.readFileSync(rustProtocolPath, "utf8"),
  fs.readFileSync(rustProtocolLabReplayPath, "utf8"),
  fs.readFileSync(rustProtocolLabScenarioPath, "utf8"),
  fs.readFileSync(rustProtocolObserverAnalysisPath, "utf8"),
  fs.readFileSync(rustProtocolServerMessagePath, "utf8"),
].join("\n");
const rustClientNetReport = fs.readFileSync(
  path.join(repoRoot, "server/crates/protocol/src/client_net_report.rs"),
  "utf8",
);
const rustContractPath = path.join(repoRoot, "server/crates/contract/src/lib.rs");
const rustContract = fs.readFileSync(rustContractPath, "utf8");
const rustLobbyPath = path.join(repoRoot, "server/src/lobby/mod.rs");
const rustLobby = fs.readFileSync(rustLobbyPath, "utf8");
const roomCapabilitiesPath = path.join(repoRoot, "client/src/room_capabilities.js");
const roomCapabilities = fs.readFileSync(roomCapabilitiesPath, "utf8");
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

const STABLE_JS_PROTOCOL_EXPORTS = [
  "ABILITY",
  "ABILITY_CODE",
  "ABILITY_OBJECT_KIND",
  "ABILITY_OBJECT_KIND_CODE",
  "BUILDING_KINDS",
  "C",
  "CMD",
  "COMPACT_SNAPSHOT_VERSION",
  "MAX_COMPACT_GROUND_DECALS",
  "DEFAULT_FACTION_ID",
  "EVENT",
  "EVENT_CODE",
  "KIND",
  "KIND_CODE",
  "LAB_ROLE",
  "LAB_REPLAY",
  "LAB_VISION",
  "LOBBY_KIND",
  "MOVEMENT_PATH_DIAGNOSTICS",
  "NOTICE",
  "NOTICE_SEVERITY",
  "NOTICE_SEVERITY_CODE",
  "ORDER_STAGE",
  "ORDER_STAGE_CODE",
  "PASSABLE",
  "PREDICTION_PROTOCOL_VERSION",
  "VISION_SELECTION",
  "RESOURCE_KINDS",
  "S",
  "SETUP",
  "SETUP_CODE",
  "SNAPSHOT_CODEC",
  "SNAPSHOT_CODEC_VERSION",
  "SNAPSHOT_FRAME_KIND",
  "STATE",
  "STATE_CODE",
  "TERRAIN",
  "UNIT_KINDS",
  "UPGRADE",
  "UPGRADE_CODE",
  "WEAPON_KIND",
  "WEAPON_KIND_CODE",
  "cmd",
  "decodeServerMessage",
  "isBuilding",
  "isResource",
  "isUnit",
  "msg",
  "parseServerFrame",
];

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function assertExportsPresent(label, moduleExports, stableNames) {
  const missing = stableNames.filter((name) => !(name in moduleExports));
  assert(missing.length === 0, `${label} missing stable exports: ${missing.join(", ")}`);
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

function assertDeepChatBuilder() {
  assert(
    JSON.stringify(msg.chatSend("team", "hold")) ===
      JSON.stringify({ t: "chatSend", channel: "team", text: "hold" }),
    "chatSend builder preserves channel and text",
  );
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

function camelToSnake(name) {
  return name.replace(/[A-Z]/g, (ch) => `_${ch.toLowerCase()}`);
}

assert(protocolContract.schemaVersion === 1, "protocol contract schema version must be 1");
assertDeepChatBuilder();
assertExportsPresent("client protocol public surface", protocolExports, STABLE_JS_PROTOCOL_EXPORTS);
assert(protocolContract.unknownCodeSentinel === 255, "unknown compact code sentinel must stay 255");
assertSameMap("server message tags", protocolContract.messageTags.server, S);
assertSameMap("client message tags", protocolContract.messageTags.client, C);
assertSameMap("command tags", protocolContract.commandTags, CMD);
assert(
  JSON.stringify(protocolContract.vocabularies.doodadTypes) === JSON.stringify(DOODAD_TYPE_IDS),
  `doodad type vocabulary mismatch\nRust: ${JSON.stringify(protocolContract.vocabularies.doodadTypes)}\nJS:   ${JSON.stringify(DOODAD_TYPE_IDS)}`,
);
assert(
  JSON.stringify(Object.values(DOODAD_TYPE)) === JSON.stringify(DOODAD_TYPE_IDS),
  "named client doodad types must retain canonical protocol order",
);
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
assertSameMap("lobby kind vocabulary", protocolContract.vocabularies.lobbyKinds, LOBBY_KIND);
assertSameMap("upgrade vocabulary", protocolContract.vocabularies.upgrades, UPGRADE);
assertSameMap("weapon kind vocabulary", protocolContract.vocabularies.weaponKinds, WEAPON_KIND);
assertSameMap("notice vocabulary", protocolContract.vocabularies.notices, NOTICE);
assertSameMap("notice severity vocabulary", protocolContract.vocabularies.noticeSeverities, NOTICE_SEVERITY);
assertSameMap("order stage vocabulary", protocolContract.vocabularies.orderStages, ORDER_STAGE);
assertSameMap("terrain", protocolContract.compactCodes.terrain, TERRAIN);
assert(PASSABLE[TERRAIN.GRASS] === true, "grass must remain passable");
assert(PASSABLE[TERRAIN.ROCK] === false, "rock must remain impassable");
assert(PASSABLE[TERRAIN.WATER] === false, "water must remain impassable");
for (const road of [
  TERRAIN.ROAD_BARE,
  TERRAIN.ROAD_HORIZONTAL,
  TERRAIN.ROAD_VERTICAL,
  TERRAIN.ROAD_DIAGONAL_NW_SE,
  TERRAIN.ROAD_DIAGONAL_NE_SW,
]) {
  assert(PASSABLE[road] === true, `road terrain ${road} must be passable`);
}
for (const openVariant of [
  TERRAIN.GRAVEL_A,
  TERRAIN.GRAVEL_B,
  TERRAIN.GRAVEL_C,
  TERRAIN.DIRT_A,
  TERRAIN.DIRT_B,
  TERRAIN.DIRT_C,
  TERRAIN.MUD_A,
  TERRAIN.MUD_B,
  TERRAIN.MUD_C,
  TERRAIN.FROSTED_GROUND,
]) {
  assert(PASSABLE[openVariant] === true, `open terrain ${openVariant} must remain passable`);
  assert(!ROAD_TERRAIN_CODES.includes(openVariant), `open terrain ${openVariant} must not gain road speed`);
}
assertSameCodes("entity kind", protocolContract.compactCodes.kind, KIND_CODE);
assertSameCodes("entity state", protocolContract.compactCodes.state, STATE_CODE);
assertSameCodes("setup state", protocolContract.compactCodes.setupState, SETUP_CODE);
assertSameCodes("event", protocolContract.compactCodes.event, EVENT_CODE);
assertSameCodes("order stage", protocolContract.compactCodes.orderStage, ORDER_STAGE_CODE);
assertSameCodes("ability", protocolContract.compactCodes.ability, ABILITY_CODE);
assertSameCodes("ability object kind", protocolContract.compactCodes.abilityObjectKind, ABILITY_OBJECT_KIND_CODE);
assertSameCodes("upgrade", protocolContract.compactCodes.upgrade, UPGRADE_CODE);
assertSameCodes("weapon kind", protocolContract.compactCodes.weaponKind, WEAPON_KIND_CODE);
assertSameCodes("notice severity", protocolContract.compactCodes.noticeSeverity, NOTICE_SEVERITY_CODE);
assertSameCodes(
  "resource kind",
  protocolContract.compactCodes.resourceKind,
  { [KIND.STEEL]: KIND_CODE[KIND.STEEL], [KIND.OIL]: KIND_CODE[KIND.OIL] },
);
const serverOrderSource = fs.readFileSync(path.join(repoRoot, "server/crates/sim/src/game/entity/order.rs"), "utf8");
const serverOrderQueueLimit = Number(serverOrderSource.match(/pub const MAX_QUEUED_ORDERS: usize = (\d+);/)?.[1]);
assert(MAX_COMPACT_ORDER_PLAN === serverOrderQueueLimit + 1, "compact order plan bound must cover the server active stage plus queue");
assert(protocolContract.compactSnapshotVersion === COMPACT_SNAPSHOT_VERSION, "compact snapshot version must match Rust");
assert(
  protocolContract.maxGroundDecalsPerSnapshotDelta === MAX_COMPACT_GROUND_DECALS,
  "ground decal snapshot delta bound must match Rust",
);
assert(
  protocolContract.snapshotCodecs.defaultCodec === SNAPSHOT_CODEC.MESSAGEPACK_COMPACT,
  "default snapshot codec must match Rust",
);
assert(protocolContract.snapshotCodecs.codecVersion === SNAPSHOT_CODEC_VERSION, "snapshot codec version must match Rust");
assert(
  protocolContract.snapshotCodecs.defaultFrameKind === SNAPSHOT_FRAME_KIND.BINARY,
  "default snapshot frame kind must match Rust",
);
assert(
  protocolContract.snapshotCodecs.supported.join(",") === SNAPSHOT_CODEC.MESSAGEPACK_COMPACT,
  "supported snapshot codecs must match Rust",
);
assert(
  protocolDoc.includes(`MessagePack compact binary snapshot frames`) &&
    protocolDoc.includes(`compact snapshot version ${COMPACT_SNAPSHOT_VERSION}`) &&
    protocolDoc.includes(`"v": ${COMPACT_SNAPSHOT_VERSION}`),
  "protocol docs must list the current compact snapshot version",
);
assert(
  protocolContract.predictionProtocolVersion === PREDICTION_PROTOCOL_VERSION,
  "prediction protocol version must match Rust",
);
assert(protocolContract.defaultFactionId === DEFAULT_FACTION_ID, "default faction id must match Rust");
const clientNetReportStruct = rustClientNetReport.match(/pub struct ClientNetReport \{([\s\S]*?)\n\}/);
assert(clientNetReportStruct, "Rust protocol must define ClientNetReport");
for (const field of CLIENT_NET_REPORT_FIELDS) {
  const rustField = camelToSnake(field);
  assert(
    new RegExp(`\\bpub\\s+${rustField}\\s*:`).test(clientNetReportStruct[1]),
    `ClientNetReport Rust DTO missing ${rustField}`,
  );
  assert(protocolDoc.includes(`${field}:`), `protocol docs missing ClientNetReport.${field}`);
}
assert(
  protocolContract.compactSlotSchemas.snapshot.map((field) => field.name).join(",") ===
    "tick,steel,oil,supplyUsed,supplyCap",
  "compact snapshot scalar schema must match JS decoder",
);
assert(
  protocolContract.compactSlotSchemas.entity.at(-1).name === "chargeActive",
  "compact entity slot schema must include the latest appended field",
);
assert(
  protocolContract.compactSlotSchemas.abilityObject.map((field) => field.name).join(",") ===
    "id,owner,ability,kind,x,y,expiresIn,sourceCasterId,ownerState,vector,fieldSize",
  "compact ability object slots must match JS decoder",
);
assert(
  protocolContract.compactSlotSchemas.trench.map((field) => field.name).join(",") ===
    "id,x,y,radiusTiles",
  "compact trench slot schema must match JS decoder",
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
assertDocsCodeTable("weaponKind", protocolContract.compactCodes.weaponKind);
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
  JSON.stringify(msg.command(cmd.holdPosition([1], true), 11)) === JSON.stringify({ t: "command", clientSeq: 11, cmd: { c: "holdPosition", units: [1], queued: true } }),
  "queued holdPosition command builder must emit clientSeq envelope",
);
assert(S.COMMAND_RECEIPT === "commandReceipt", "command receipt server tag must be mirrored");
assert(
  rust.includes("CommandReceipt") && protocolDoc.includes("commandReceipt"),
  "command receipt server message must be mirrored in Rust and docs",
);
assert(
  C.PAUSE_GAME === "pauseGame" &&
    C.UNPAUSE_GAME === "unpauseGame" &&
    S.LIVE_PAUSE_STATE === "livePauseState",
  "live pause protocol tags must be mirrored",
);
assert(
  JSON.stringify(msg.pauseGame()) === JSON.stringify({ t: "pauseGame" }) &&
    JSON.stringify(msg.unpauseGame()) === JSON.stringify({ t: "unpauseGame" }),
  "live pause builders must emit exact wire shapes",
);
assert(
  rust.includes("LivePauseState") &&
    rust.includes("pauses_remaining") &&
    protocolDoc.includes("livePauseState"),
  "live pause state contract must be documented and mirrored in Rust",
);
assert(
  // Temporary source-text allowlist: start payload field-shape assertions are contract DTO checks,
  // not part of this phase's structured protocol export.
    rustContract.includes("prediction_build_id") &&
    rustContract.includes("prediction_version") &&
    rustContract.includes("match_run_id") &&
    rustContract.includes("pub is_ai: bool") &&
    protocolDoc.includes("isAi, startTileX"),
  "start payload must expose prediction compatibility and AI participant metadata",
);
assert(
  S.OBSERVATION_READY === "observationReady" &&
    rust.includes("ObservationReady") &&
    protocolDoc.includes("observationReady"),
  "AI observation completion message must be mirrored in Rust and docs",
);
assert(
  rustContract.includes("LabStartMetadata") &&
    rustContract.includes("operator_id") &&
    rustContract.includes("initial_camera") &&
    rustContract.includes("operation_count") &&
    LAB_ROLE.OPERATOR === "operator" &&
    LAB_ROLE.READ_ONLY === "readOnly" &&
    LAB_VISION.ALL === "all" &&
    LAB_VISION.TEAM === "team",
  "start payload must expose mirrored lab metadata",
);
assert(
  rustContract.includes("DiagnosticCapabilities") &&
    rustContract.includes("movement_paths") &&
    !rustContract.includes("debug_mode") &&
    MOVEMENT_PATH_DIAGNOSTICS.OWNER_ONLY === "ownerOnly" &&
    MOVEMENT_PATH_DIAGNOSTICS.ALL === "all",
  "start payload must expose diagnostic capability metadata instead of debugMode",
);
assert(
  rustContract.includes("RoomCapabilities") &&
    rustContract.includes("RoomTimeCapabilities") &&
    rustContract.includes("MatchControlCapabilities") &&
    rustContract.includes("VisibilityCapabilities") &&
    rustContract.includes("CommandCapabilities") &&
    rustContract.includes("ActionCapabilities") &&
    protocolDoc.includes("capabilities?:") &&
    roomCapabilities.includes("startPayload?.capabilities") &&
    roomCapabilities.includes("roomTime") &&
    roomCapabilities.includes("matchControls") &&
    roomCapabilities.includes("visionSelection") &&
    roomCapabilities.includes("gameplay") &&
    roomCapabilities.includes("branchFromTick") &&
    protocolDoc.includes("actions?: { branchFromTick?: bool }"),
  "start payload room capabilities must be documented and mirrored by the client parser",
);
assert(
  C.LAB === "lab" && S.LAB_STATE === "labState" && S.LAB_RESULT === "labResult",
  "lab protocol tags must be mirrored",
);
assert(
  JSON.stringify(msg.labSetVision(11, msg.labVisionAll())) ===
    JSON.stringify({ t: "lab", requestId: 11, op: { op: "setVision", vision: { mode: "all" } } }),
  "lab all-team vision builder must emit the exact wire shape",
);
assert(
  JSON.stringify(msg.labSetVision(12, msg.labVisionTeam(2))) ===
    JSON.stringify({ t: "lab", requestId: 12, op: { op: "setVision", vision: { mode: "team", teamId: 2 } } }),
  "lab vision builder must emit the exact wire shape",
);
assert(
  JSON.stringify(msg.labIssueCommandAs(13, 1, cmd.stop([7]))) ===
    JSON.stringify({
      t: "lab",
      requestId: 13,
      op: {
        op: "issueCommandAs",
        playerId: 1,
        cmd: { c: "stop", units: [7] },
        ignoreCommandLimits: false,
      },
    }),
  "lab issue-as builder must emit the exact wire shape",
);
assert(
  JSON.stringify(msg.labExportScenario(14, "saved setup")) ===
    JSON.stringify({ t: "lab", requestId: 14, op: { op: "exportScenario", name: "saved setup" } }),
  "lab setup export compatibility builder must emit the exact wire shape",
);
assert(
  JSON.stringify(msg.labImportScenario(15, { schemaVersion: LAB_CHECKPOINT_SCENARIO.SCHEMA_VERSION, kind: LAB_CHECKPOINT_SCENARIO.KIND })) ===
    JSON.stringify({ t: "lab", requestId: 15, op: { op: "importScenario", scenario: { schemaVersion: 1, kind: "labCheckpointScenario" } } }),
  "lab checkpoint setup import builder must emit the exact wire shape",
);
assert(
  JSON.stringify(msg.labValidateScenario(16, { slug: "new-lab", name: "New Lab", title: "New Lab", description: "Ready" })) ===
    JSON.stringify({
      t: "lab",
      requestId: 16,
      op: {
        op: "validateScenario",
        metadata: { slug: "new-lab", name: "New Lab", title: "New Lab", description: "Ready" },
      },
    }),
  "lab setup validation builder must emit the exact wire shape",
);
assert(
  rust.includes("ExportScenario") &&
    rust.includes("ImportScenario") &&
    rust.includes("ValidateScenario") &&
    rust.includes("LabScenarioPayload") &&
    rust.includes("LabCheckpointScenarioV1") &&
    !rust.includes("LabScenarioV1") &&
    rust.includes("god_mode_players") &&
    rust.includes("initial_camera") &&
    LAB_CHECKPOINT_SCENARIO.KIND === "labCheckpointScenario" &&
    LAB_CHECKPOINT_SCENARIO.SCHEMA_VERSION === 1 &&
    LAB_REPLAY.SCHEMA === "rts.labReplay" &&
    LAB_REPLAY.KIND === "labReplay" &&
    LAB_REPLAY.SCHEMA_VERSION === 1 &&
    LAB_REPLAY.TIMELINE_KEYFRAME_INTERVAL_TICKS === 2000 &&
    LAB_REPLAY.MAX_OPERATIONS === 50000 &&
    LAB_REPLAY.MAX_ARTIFACT_BYTES === 8 * 1024 * 1024 &&
    LAB_REPLAY.MAX_OPERATION_JSON_BYTES === 64 * 1024 &&
    LAB_REPLAY.MAX_CHECKPOINT_PAYLOAD_BYTES === 4 * 1024 * 1024 &&
    rust.includes("LabReplayArtifactV1") &&
    rust.includes("LAB_REPLAY_ARTIFACT_KIND") &&
    rust.includes("IssueCommandAs") &&
    rust.includes("SetPlayerGodMode") &&
    protocolDoc.includes("LabScenarioPayload") &&
    protocolDoc.includes("LabCheckpointScenarioV1") &&
    protocolDoc.includes("LabReplayArtifactV1") &&
    protocolDoc.includes("checkpoint-backed setup") &&
    protocolDoc.includes("Legacy labScenario JSON is rejected") &&
    protocolDoc.includes("Lab replay import validates") &&
    protocolDoc.includes("rts.labReplay") &&
    protocolDoc.includes("labReplay") &&
    protocolDoc.includes("issueCommandAs") &&
    protocolDoc.includes("setVision") &&
    !protocolDoc.includes("LabScenarioV1") &&
    !protocolDoc.includes("LabScenarioOrder") &&
    protocolDoc.includes("godModePlayers") &&
    protocolDoc.includes("initialCamera") &&
    protocolDoc.includes("validateScenario"),
  "lab setup/replay protocol surface must be documented and mirrored",
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
  JSON.stringify(msg.setName("Renamed")) === JSON.stringify({ t: "setName", name: "Renamed" }),
  "setName builder must emit the exact wire shape",
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
  rr: true,
  r: [[200, 0]],
  e: [],
  ma: [[91, 2, 320, 352, 0.75, 10]],
  ce: 123,
  pr: [[1, 325, 80, 7, 14, 42, [1], 456]],
  pf: [[1, "cultivators"], [2, "kriegsia"]],
  ao: [[70, 1, 6, 1, 384, 416, 90, 7, [45, null, null, null, null, null, null, null]],
    [71, 1, 17, 4, 400, 400, 300, 7, [null, 240, 67.2, null, null, null, 300, 300]],
    [72, 2, 27, 5, 480, 400, 299, null, null, [0, -1], 5]],
  n: [1, 2, 0, 3, 4, PREDICTION_PROTOCOL_VERSION, 7, 12],
});
assert(decodedAck.resourceReset === true && decodedAck.resourceDeltas[0].remaining === 0, "compact authoritative resource replacement decodes");
assert(decodedAck.netStatus.predictionVersion === PREDICTION_PROTOCOL_VERSION, "compact predictionVersion decodes");
assert(decodedAck.netStatus.lastSimConsumedClientSeq === 7, "compact consumed client seq decodes");
assert(decodedAck.netStatus.lastSimConsumedClientTick === 12, "compact consumed client tick decodes");
assert(decodedAck.abilityObjects[0].kind === "returnMarker", "compact ability object kind decodes");
assert(decodedAck.abilityObjects[0].sourceCasterId === 7, "compact ability source caster decodes");
assert(JSON.stringify(decodedAck.abilityObjects[2]) === JSON.stringify({
  id: 72, owner: 2, ability: ABILITY.VECTOR_FIELD, kind: ABILITY_OBJECT_KIND.VECTOR_FIELD,
  x: 480, y: 400, expiresIn: 299, vector: [0, -1], fieldSize: 5,
}), "visible enemy Vector Field decodes public geometry without private caster state");
assert(decodedAck.playerFactions[0].factionId === "cultivators", "compact authoritative faction decodes");
assert(decodedAck.cosmicEnergy === 123, "compact private Cosmic Energy decodes");
assert(decodedAck.playerResources[0].cosmicEnergy === 456, "compact observer Cosmic Energy decodes");
assert(decodedAck.playerResources[0].apm === 42, "compact observer player APM decodes");
assert(decodedAck.playerResources[0].upgrades.length === 1, "compact observer upgrades decode");
assert(
  decodedAck.rememberedAntiTankGuns[0].id === 91
    && decodedAck.rememberedAntiTankGuns[0].facing === 0.75
    && decodedAck.rememberedAntiTankGuns[0].observedTick === 10,
  "compact server-authoritative anti-tank-gun memory decodes",
);
assert(
  decodedAck.abilityObjects[0].ownerState.earliestReturnTick === 45,
  "compact ability owner state decodes",
);
assert(decodedAck.abilityObjects[1].ownerState.maxHp === 300
  && decodedAck.abilityObjects[1].ownerState.durationTicks === 300,
  "compact Master Shield cast-time maxima decode");
assert(
  C.SET_VISION_SELECTION === "setVisionSelection",
  "setVisionSelection client message tag must match Rust",
);
assert(
  JSON.stringify(msg.visionSelectionPlayer(7)) ===
    JSON.stringify({ t: "setVisionSelection", selection: { mode: "player", playerId: 7 } }),
  "setVisionSelection builder must emit the exact wire shape",
);
assert(
  JSON.stringify(msg.visionSelectionOmniscient()) ===
    JSON.stringify({ t: "setVisionSelection", selection: { mode: "omniscient" } }),
  "omniscient vision builder must emit the exact wire shape",
);
assert(
  C.REQUEST_BRANCH_FROM_TICK === "requestBranchFromTick",
  "requestBranchFromTick client message tag must match Rust",
);
assert(
  JSON.stringify(msg.requestBranchFromTick()) === JSON.stringify({ t: "requestBranchFromTick" }),
  "requestBranchFromTick builder must emit the exact wire shape",
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
  S.BRANCH_FROM_TICK_CREATED === "branchFromTickCreated",
  "branchFromTickCreated server message tag must match Rust",
);
// Temporary source-text allowlist: branch and observer-analysis payload field lists are still
// DTO shape smoke checks until a later phase expands structured contract-field exports.
for (const field of ["branch_room", "source_tick", "seats", "player_id", "team_id", "faction_id", "claimable"]) {
  assert(rust.includes(field), `branchFromTickCreated Rust contract is missing ${field}`);
}
assert(
  S.BRANCH_STAGING === "branchStaging",
  "branchStaging server message tag must match Rust",
);
for (const field of ["host_id", "team_id", "faction_id", "claimant_id", "occupants", "can_start"]) {
  assert(rust.includes(field), `branchStaging Rust contract is missing ${field}`);
}
assert(
  S.OBSERVER_ANALYSIS === "observerAnalysis",
  "observerAnalysis server message tag must match Rust",
);
assert(S.REPLAY_RESOURCE_HISTORY === "replayResourceHistory", "replay resource history tag matches Rust");
for (const field of [
  "units_lost",
  "buildings_lost",
  "resources_lost",
  "resources",
  "lifetime",
  "last_5s",
  "last_minute",
  "steel_value",
  "oil_value",
  "queue_depth",
  "upgrades",
  "ai_diagnostics",
  "profile_id",
  "trace_tick",
  "map_analysis",
  "map_width",
  "default_visible",
  "TileRect",
  "Marker",
]) {
  assert(rust.includes(field), `observerAnalysis Rust contract is missing ${field}`);
}
const observerAnalysis = decodeServerMessage({
  t: S.OBSERVER_ANALYSIS,
  tick: 9,
  players: [{
    id: 1,
    units: [{ kind: "worker", count: 2, steelValue: 100, oilValue: 0 }],
    production: [{ buildingId: 7, buildingKind: "resource_depot", itemKind: "worker", itemType: "unit", progress: 0.25, queueDepth: 1 }],
    upgrades: ["tank_unlock"],
    unitsLost: [],
    buildingsLost: [{ kind: "steel_mine", count: 2, steelValue: 0, oilValue: 0 }],
    resourcesLost: { steel: 0, oil: 0 },
    resources: {
      lifetime: { steel: 100, oil: 20 },
      last5s: { steel: 40, oil: 0 },
      lastMinute: { steel: 100, oil: 20 },
    },
    aiDiagnostics: {
      profileId: "ai_2_1",
      traceTick: 9,
      lines: ["profile=ai_2_1 tick=9"],
    },
  }],
  mapAnalysis: {
    mapWidth: 126,
    mapHeight: 126,
    tileSize: 32,
    layers: [{
      id: "components",
      label: "Components",
      defaultVisible: true,
      primitives: [{
        kind: "tileRect",
        id: "component:0",
        tileX: 0,
        tileY: 0,
        tileW: 10,
        tileH: 8,
        fill: "#3da5d9",
        stroke: "#3da5d9",
        alpha: 0.12,
        label: "C0 80t clr8",
      }],
    }],
  },
});
assert(observerAnalysis.t === "observerAnalysis" && observerAnalysis.players[0].production[0].queueDepth === 1, "observerAnalysis passes through decode");
assert(observerAnalysis.players[0].upgrades[0] === "tank_unlock", "observerAnalysis preserves completed research");
assert(
  observerAnalysis.players[0].aiDiagnostics?.profileId === "ai_2_1",
  "observerAnalysis preserves AI diagnostics rows",
);
assert(
  observerAnalysis.players[0].resources?.last5s?.steel === 40,
  "observerAnalysis preserves mined-resource windows",
);
assert(
  observerAnalysis.mapAnalysis?.layers?.[0]?.primitives?.[0]?.kind === "tileRect",
  "observerAnalysis preserves map-analysis overlay primitives",
);


assert(JSON.stringify(cmd.pointTanks([1, 2], 300, 400)) === JSON.stringify({ c: "pointTanks", units: [1, 2], x: 300, y: 400 }), "Point command preserves one group target");

assert(JSON.stringify(cmd.pointTanks([1], 300, 400, true)) === JSON.stringify({ c: "pointTanks", units: [1], x: 300, y: 400, queued: true }), "Point command supports queueing");

const pendingDaoRecord = [44, 1, 29, 320, 320, 135, 135, 1];
while (pendingDaoRecord.length < 50) pendingDaoRecord.push(null);
pendingDaoRecord[45] = 15;
pendingDaoRecord[49] = "magic";
const pendingDaoSnapshot = decodeServerMessage({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION,
  s: [1, 0, 0, 2, 10], e: [pendingDaoRecord], n: [0, 0, 0, 0, 0] });
assert(pendingDaoSnapshot.entities[0].chosenDao === "magic", "compact private Dao intent decodes at slot 49");

const tournamentRecord = [...pendingDaoRecord];
tournamentRecord[2] = KIND_CODE[KIND.TOURNAMENT_GROUNDS];
tournamentRecord[50] = 3;
const tournamentSnapshot = decodeServerMessage({t: "snapshot", v: COMPACT_SNAPSHOT_VERSION,
  s: [1, 0, 0, 2, 10], e: [tournamentRecord], n: [0, 0, 0, 0, 0]});
assert(tournamentSnapshot.entities[0].kind === KIND.TOURNAMENT_GROUNDS,
  "Tournament Grounds compact kind round trips");
assert(tournamentSnapshot.entities[0].tournamentEntrants === 3,
  "compact tournament entrant count decodes at slot 50");

const paidTrainingRecord = [...tournamentRecord];
paidTrainingRecord[2] = KIND_CODE[KIND.PROVEN_GROUNDS];
paidTrainingRecord[50] = null;
paidTrainingRecord[51] = [UPGRADE.DAO_OF_MAGIC, UPGRADE.DAO_OF_HEAVINESS];
const paidTrainingSnapshot = decodeServerMessage({t: "snapshot", v: COMPACT_SNAPSHOT_VERSION,
  s: [1, 0, 0, 2, 10], e: [paidTrainingRecord], n: [0, 0, 0, 0, 0]});
assert(paidTrainingSnapshot.entities[0].kind === KIND.PROVEN_GROUNDS, "Proving Grounds kind roundtrips");
assert(paidTrainingSnapshot.entities[0].daoTrainingQueue.join(",") === "dao_of_magic,dao_of_heaviness",
  "paid training plans decode at slot 51");

const daoMasterRecord = [501, 1, KIND_CODE[KIND.WARRIOR], 400, 400, 161, 161, STATE_CODE[STATE.IDLE]];
while (daoMasterRecord.length < 53) daoMasterRecord.push(null);
daoMasterRecord[46] = "wind";
daoMasterRecord[52] = true;
const daoMasterSnapshot = decodeServerMessage({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION,
  s: [1, 0, 0, 2, 300], e: [daoMasterRecord], ev: [], n: [0, 0, 0, 0, 0] });
assert(daoMasterSnapshot.entities[0].daoMaster === true, "public Master rank decodes at slot 52");
assert(daoMasterSnapshot.entities[0].insight === undefined, "rank does not reveal exact insight");

// A fully equipped Novice can project four innate entries plus five carried entries.
const equippedNovice = [...daoMasterRecord];
equippedNovice[23] = Array.from({ length: 9 }, (_, index) => [index + 1, 0]);
const equippedSnapshot = decodeServerMessage({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION,
  s: [1, 0, 0, 2, 300], e: [equippedNovice], ev: [], n: [0, 0, 0, 0, 0] });
assert(equippedSnapshot.entities[0].abilities.length === 9, "fully equipped units exceed the old eight-entry bound");

const escapeRecord = [...daoMasterRecord, true];
const escapeSnapshot = decodeServerMessage({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION,
  s: [1, 0, 0, 2, 300], e: [escapeRecord], ev: [], n: [0, 0, 0, 0, 0] });
assert(escapeSnapshot.entities[0].escapeTreasure === true, "escape protection decodes at slot 53");
assert(daoMasterSnapshot.entities[0].escapeTreasure === undefined, "absent escape protection stays absent");

for (let tier = 0; tier <= 6; tier++) {
  const record = [...daoMasterRecord, false, tier];
  const snapshot = decodeServerMessage({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION,
    s: [1, 0, 0, 2, 300], e: [record], ev: [], n: [0, 0, 0, 0, 0] });
  assert(snapshot.entities[0].itemRadianceTier === tier, "public item tier decodes at slot 54, including zero");
  assert(!snapshot.entities[0].abilities, "public tier does not require private ability entries");
}
for (const tier of [-1, 7, 1.5, "3"]) {
  let rejected = false;
  try {
    decodeServerMessage({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION,
      s: [1, 0, 0, 2, 300], e: [[...daoMasterRecord, false, tier]], ev: [], n: [0, 0, 0, 0, 0] });
  } catch { rejected = true; }
  assert(rejected, "out-of-range or malformed public item tiers are rejected");
}

// Ground decal records retain their independent ten-slot contract.
const groundDecal = [1, "wreck", KIND_CODE[KIND.TANK], 100, 100, 1, 7, null, null, 1];
const decalSnapshot = decodeServerMessage({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION,
  s: [1, 0, 0, 2, 300], e: [], n: [0, 0, 0, 0, 0], gr: 1, gd: [0, [groundDecal]] });
assert(decalSnapshot.groundDecalDelta.decals[0].radiusTiles === 1, "valid ground decal geometry decodes");
for (const record of [groundDecal.slice(0, 9), [...groundDecal, null]]) {
  let rejected = false;
  try {
    decodeServerMessage({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION,
      s: [1, 0, 0, 2, 300], e: [], n: [0, 0, 0, 0, 0], gr: 1, gd: [0, [record]] });
  } catch { rejected = true; }
  assert(rejected, "malformed ground decal slot counts are rejected");
}

console.log("✅ protocol_parity.mjs: Rust protocol contract dump matches JS mirror");

const chargeRecord = [...daoMasterRecord, false, null, true];
const chargeSnapshot = decodeServerMessage({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION,
  s: [1, 0, 0, 2, 300], e: [chargeRecord], ev: [], n: [0, 0, 0, 0, 0] });
assert(chargeSnapshot.entities[0].chargeActive === true, "active Charge decodes at slot 55");
assert(daoMasterSnapshot.entities[0].chargeActive === undefined, "absent Charge stays absent");
