// tests/protocol_parity.mjs
// Guard compact wire vocabularies against Rust encoder / JS decoder drift.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  ABILITY_CODE,
  EVENT_CODE,
  KIND_CODE,
  NOTICE_SEVERITY_CODE,
  ORDER_STAGE_CODE,
  C,
  S,
  SETUP_CODE,
  STATE_CODE,
  TERRAIN,
  UPGRADE_CODE,
  msg,
} from "../client/src/protocol.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const rustProtocolPath = path.join(repoRoot, "server/crates/protocol/src/lib.rs");
const rust = fs.readFileSync(rustProtocolPath, "utf8");

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

function lowerCamel(name) {
  return name[0].toLowerCase() + name.slice(1);
}

function extractBlock(startPattern, label) {
  const start = rust.search(startPattern);
  assert(start >= 0, `missing Rust block: ${label}`);
  const open = rust.indexOf("{", start);
  assert(open >= 0, `missing Rust block open: ${label}`);
  let depth = 0;
  for (let i = open; i < rust.length; i += 1) {
    const ch = rust[i];
    if (ch === "{") depth += 1;
    if (ch === "}") {
      depth -= 1;
      if (depth === 0) return rust.slice(open + 1, i);
    }
  }
  throw new Error(`missing Rust block close: ${label}`);
}

function extractModuleStringConstants(moduleName) {
  const block = extractBlock(new RegExp(`pub\\s+mod\\s+${moduleName}\\s*\\{`), moduleName);
  const constants = new Map();
  const re = /pub\s+const\s+([A-Z0-9_]+)\s*:\s*&str\s*=\s*"([^"]+)";/g;
  for (const match of block.matchAll(re)) {
    constants.set(`${moduleName}::${match[1]}`, match[2]);
  }
  return constants;
}

function extractTerrainCodes() {
  const block = extractBlock(/pub\s+mod\s+terrain\s*\{/, "terrain");
  const codes = {};
  const re = /pub\s+const\s+([A-Z0-9_]+)\s*:\s*u8\s*=\s*(\d+);/g;
  for (const match of block.matchAll(re)) {
    codes[match[1]] = Number(match[2]);
  }
  return codes;
}

const rustConstants = new Map([
  ...extractModuleStringConstants("kinds"),
  ...extractModuleStringConstants("states"),
  ...extractModuleStringConstants("abilities"),
  ...extractModuleStringConstants("upgrades"),
]);

function resolveRustPattern(pattern) {
  const trimmed = pattern.trim();
  const stringLiteral = trimmed.match(/^"([^"]+)"$/);
  if (stringLiteral) return stringLiteral[1];
  if (rustConstants.has(trimmed)) return rustConstants.get(trimmed);
  const noticeVariant = trimmed.match(/^NoticeSeverity::([A-Za-z0-9_]+)$/);
  if (noticeVariant) return lowerCamel(noticeVariant[1]);
  throw new Error(`cannot resolve Rust protocol pattern: ${trimmed}`);
}

function extractCodeFunction(functionName) {
  const block = extractBlock(new RegExp(`fn\\s+${functionName}\\s*\\(`), functionName);
  const codes = {};
  const re = /^[ \t]*([^_\s][^=\n]*?)\s*=>\s*(\d+),/gm;
  for (const match of block.matchAll(re)) {
    codes[resolveRustPattern(match[1])] = Number(match[2]);
  }
  return codes;
}

function extractEventCodes() {
  const block = extractBlock(/impl\s+Serialize\s+for\s+CompactEvent/, "CompactEvent");
  const codes = {};
  const re = /Event::([A-Za-z0-9_]+)\s*(?:\{[\s\S]*?\})?\s*=>\s*\{[\s\S]*?seq\.serialize_element\(&(\d+)u8\)\?/g;
  for (const match of block.matchAll(re)) {
    codes[lowerCamel(match[1])] = Number(match[2]);
  }
  return codes;
}

function assertSameCodes(label, rustCodes, jsCodes) {
  const actual = Object.fromEntries(Object.entries(jsCodes).sort(([a], [b]) => a.localeCompare(b)));
  const expected = Object.fromEntries(Object.entries(rustCodes).sort(([a], [b]) => a.localeCompare(b)));
  assert(
    JSON.stringify(actual) === JSON.stringify(expected),
    `${label} code map mismatch\nRust: ${JSON.stringify(expected)}\nJS:   ${JSON.stringify(actual)}`,
  );
  assertNoDuplicateCodes(label, actual);
}

function assertNoDuplicateCodes(label, codes) {
  const seen = new Map();
  for (const [name, code] of Object.entries(codes)) {
    assert(Number.isInteger(code), `${label}.${name} code must be an integer`);
    assert(code !== 255, `${label}.${name} must not use the unknown sentinel code`);
    assert(!seen.has(code), `${label} code ${code} is reused by ${seen.get(code)} and ${name}`);
    seen.set(code, name);
  }
}

assertSameCodes("terrain", extractTerrainCodes(), TERRAIN);
assertSameCodes("entity kind", extractCodeFunction("kind_code"), KIND_CODE);
assertSameCodes("entity state", extractCodeFunction("state_code"), STATE_CODE);
assertSameCodes("setup state", extractCodeFunction("setup_state_code"), SETUP_CODE);
assertSameCodes("event", extractEventCodes(), EVENT_CODE);
assertSameCodes("order stage", extractCodeFunction("order_stage_code"), ORDER_STAGE_CODE);
assertSameCodes("ability", extractCodeFunction("ability_code"), ABILITY_CODE);
assertSameCodes("upgrade", extractCodeFunction("upgrade_code"), UPGRADE_CODE);
assertSameCodes("notice severity", extractCodeFunction("notice_severity_code"), NOTICE_SEVERITY_CODE);

assert(
  rust.includes("RequestReplayBranch") && C.REQUEST_REPLAY_BRANCH === "requestReplayBranch",
  "requestReplayBranch client message tag must match Rust",
);
assert(
  JSON.stringify(msg.requestReplayBranch()) === JSON.stringify({ t: "requestReplayBranch" }),
  "requestReplayBranch builder must emit the exact wire shape",
);
assert(
  rust.includes("ClaimBranchSeat") && C.CLAIM_BRANCH_SEAT === "claimBranchSeat",
  "claimBranchSeat client message tag must match Rust",
);
assert(
  JSON.stringify(msg.claimBranchSeat(7)) === JSON.stringify({ t: "claimBranchSeat", playerId: 7 }),
  "claimBranchSeat builder must emit the exact wire shape",
);
assert(
  rust.includes("ReleaseBranchSeat") && C.RELEASE_BRANCH_SEAT === "releaseBranchSeat",
  "releaseBranchSeat client message tag must match Rust",
);
assert(
  JSON.stringify(msg.releaseBranchSeat(7)) === JSON.stringify({ t: "releaseBranchSeat", playerId: 7 }),
  "releaseBranchSeat builder must emit the exact wire shape",
);
assert(
  rust.includes("StartBranch") && C.START_BRANCH === "startBranch",
  "startBranch client message tag must match Rust",
);
assert(
  JSON.stringify(msg.startBranch()) === JSON.stringify({ t: "startBranch" }),
  "startBranch builder must emit the exact wire shape",
);
assert(
  rust.includes("ReplayBranchCreated") && S.REPLAY_BRANCH_CREATED === "replayBranchCreated",
  "replayBranchCreated server message tag must match Rust",
);
for (const field of ["branch_room", "source_tick", "seats", "player_id", "claimable"]) {
  assert(rust.includes(field), `replayBranchCreated Rust contract is missing ${field}`);
}
assert(
  rust.includes("BranchStaging") && S.BRANCH_STAGING === "branchStaging",
  "branchStaging server message tag must match Rust",
);
for (const field of ["host_id", "claimant_id", "occupants", "can_start"]) {
  assert(rust.includes(field), `branchStaging Rust contract is missing ${field}`);
}

console.log("✅ protocol_parity.mjs: Rust compact protocol codes match JS decoder maps");
