#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const lobbySrc = path.join(repoRoot, "server/src/lobby");

const allowedSnapshotCalls = new Map(Object.entries({
  "projection.rs": new Set([
    "snapshot_for",
    "snapshot_for_spectator",
    "snapshot_full_for",
  ]),
  // AI controllers need the same fog-filtered view as their player seat. This is not client fanout.
  "live_tick.rs": new Set(["snapshot_for"]),
}));

const snapshotCallRe = /\.\s*(snapshot_full_for|snapshot_for_spectator|snapshot_for)\s*\(/g;
const labMutationCallRe = /\.\s*(apply_lab_op|issue_lab_command_as|restore_lab_scenario)\s*\(/g;
const failures = [];

for (const file of listRustFiles(lobbySrc)) {
  const abs = path.join(lobbySrc, file);
  const source = fs.readFileSync(abs, "utf8");
  const stripped = stripCfgTestModules(source);
  const allowed = allowedSnapshotCalls.get(file) ?? new Set();
  for (const match of stripped.matchAll(snapshotCallRe)) {
    const method = match[1];
    if (allowed.has(method)) continue;
    failures.push(
      `${path.posix.join("server/src/lobby", file)}:${lineNumberAt(stripped, match.index)}: ${method} must route through projection.rs`,
    );
  }

  if (file !== "room_task.rs" && file !== "room_task/lab.rs") {
    for (const match of stripped.matchAll(labMutationCallRe)) {
      failures.push(
        `${path.posix.join("server/src/lobby", file)}:${lineNumberAt(stripped, match.index)}: ${match[1]} must stay centralized in room_task/lab.rs lab request handling`,
      );
    }
  }

  checkGenericRoomHelperModeShortcuts(file, stripped);
  if (file === "room_task/lifecycle.rs") checkEndMatchPersistencePolicy(stripped);
}

if (failures.length > 0) {
  console.error("lobby architecture check failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log("lobby architecture check passed");

function listRustFiles(dir) {
  const out = [];
  walk(dir, "");
  return out.sort();

  function walk(absDir, relDir) {
    for (const entry of fs.readdirSync(absDir, { withFileTypes: true })) {
      const rel = relDir ? `${relDir}/${entry.name}` : entry.name;
      const abs = path.join(absDir, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === "tests") continue;
        walk(abs, rel);
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        out.push(rel.replaceAll(path.sep, "/"));
      }
    }
  }
}

function stripCfgTestModules(source) {
  const lines = source.split("\n");
  const output = [];
  let pendingCfgTest = false;
  let skipping = false;
  let braceDepth = 0;

  for (const line of lines) {
    if (!skipping && line.trim() === "#[cfg(test)]") {
      pendingCfgTest = true;
      output.push("");
      continue;
    }

    if (pendingCfgTest && !skipping && /^\s*mod\s+tests\s*\{/.test(line)) {
      skipping = true;
      pendingCfgTest = false;
      braceDepth = braceDelta(line);
      output.push("");
      if (braceDepth <= 0) skipping = false;
      continue;
    }

    pendingCfgTest = false;

    if (skipping) {
      braceDepth += braceDelta(line);
      output.push("");
      if (braceDepth <= 0) skipping = false;
      continue;
    }

    output.push(line);
  }

  return output.join("\n");
}

function checkGenericRoomHelperModeShortcuts(file, source) {
  const genericHelpers = new Set([
    "launch.rs",
    "live_tick.rs",
    "projection.rs",
    "snapshot_fanout.rs",
  ]);
  if (!genericHelpers.has(file)) return;
  if (source.includes("RoomMode::") || source.includes("SessionMode::")) {
    failures.push(
      `${path.posix.join("server/src/lobby", file)}: generic room helper must consume SessionPolicy/projection/capability metadata instead of product-mode names`,
    );
  }
}

function checkEndMatchPersistencePolicy(source) {
  const endMatch = extractFunctionBody(source, "end_match");
  if (!endMatch) {
    failures.push("server/src/lobby/room_task/lifecycle.rs: missing end_match persistence guardrail target");
    return;
  }
  const body = endMatch;
  if (
    body.includes("ReplayArtifactV1::capture_from_game") &&
    !body.includes("should_capture_post_match_replay")
  ) {
    failures.push(
      "server/src/lobby/room_task/lifecycle.rs: end_match replay capture must be gated by persistence policy",
    );
  }
  if (
    body.includes("MatchReplayRecord::from_artifact") &&
    !body.includes("should_attach_match_history_replay_artifact")
  ) {
    failures.push(
      "server/src/lobby/room_task/lifecycle.rs: match-history replay attachment must be gated by persistence policy",
    );
  }
}

function extractFunctionBody(source, functionName) {
  const signature = new RegExp(`\\n\\s*(?:pub(?:\\([^)]*\\))?\\s+)?fn\\s+${functionName}\\s*\\(`);
  const match = source.match(signature);
  if (!match) return null;

  const signatureStart = match.index + 1;
  const bodyStart = source.indexOf("{", signatureStart);
  if (bodyStart === -1) return null;

  let depth = 0;
  for (let i = bodyStart; i < source.length; i += 1) {
    const char = source[i];
    if (char === "{") depth += 1;
    if (char === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(signatureStart, i + 1);
    }
  }
  return null;
}

function braceDelta(line) {
  let delta = 0;
  for (const char of line) {
    if (char === "{") delta += 1;
    if (char === "}") delta -= 1;
  }
  return delta;
}

function lineNumberAt(source, index) {
  return source.slice(0, index).split("\n").length;
}
