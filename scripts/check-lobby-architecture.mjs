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
const allowedLabMutationFiles = new Set([
  "room_task/lab.rs",
  "room_task/lab/replay.rs",
]);
const roomTaskRootBudget = {
  file: "room_task.rs",
  maxLines: 550,
};
const roomTaskChildLineBudgets = new Map(Object.entries({
  "room_task/branch.rs": 340,
  "room_task/dev.rs": 470,
  "room_task/helpers.rs": 140,
  "room_task/lab.rs": 1400,
  "room_task/lab/replay.rs": 650,
  "room_task/lab/submission.rs": 180,
  "room_task/lifecycle.rs": 560,
  "room_task/live.rs": 750,
  "room_task/lobby.rs": 950,
  "room_task/match_history.rs": 180,
  "room_task/replay.rs": 720,
  "room_task/types.rs": 220,
}));
// Lab map-draft validation, replay rebasing, and peer refresh are intentionally centralized in
// room_task/lab.rs; keep the aggregate ratchet at the resulting proof-of-concept footprint.
const roomTaskTotalLineBudget = 6360;

const lobbyRustFiles = listRustFiles(lobbySrc);

for (const file of lobbyRustFiles) {
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

  if (!allowedLabMutationFiles.has(file)) {
    for (const match of stripped.matchAll(labMutationCallRe)) {
      failures.push(
        `${path.posix.join("server/src/lobby", file)}:${lineNumberAt(stripped, match.index)}: ${match[1]} must stay centralized in room_task/lab.rs lab request handling`,
      );
    }
  }

  checkGenericRoomHelperModeShortcuts(file, stripped);
  if (file === "room_task/lifecycle.rs") checkEndMatchPersistencePolicy(stripped);
}

checkRoomTaskModuleBudgets(lobbyRustFiles);

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
    body.includes("finalize_replay_artifact") &&
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

function checkRoomTaskModuleBudgets(files) {
  let totalLines = 0;
  const budgetedRoomTaskFiles = new Set([
    roomTaskRootBudget.file,
    ...roomTaskChildLineBudgets.keys(),
  ]);

  for (const file of files) {
    if (file !== "room_task.rs" && !file.startsWith("room_task/")) continue;
    const source = fs.readFileSync(path.join(lobbySrc, file), "utf8");
    const lineCount = countLines(source);
    totalLines += lineCount;

    if (!budgetedRoomTaskFiles.has(file)) {
      failures.push(
        `${path.posix.join("server/src/lobby", file)}: room-task module needs an explicit size budget in scripts/check-lobby-architecture.mjs`,
      );
      continue;
    }

    const maxLines =
      file === roomTaskRootBudget.file
        ? roomTaskRootBudget.maxLines
        : roomTaskChildLineBudgets.get(file);
    if (lineCount > maxLines) {
      failures.push(
        `${path.posix.join("server/src/lobby", file)}: ${lineCount} lines exceeds room-task budget of ${maxLines}; split responsibilities before growing this module`,
      );
    }
  }

  if (totalLines > roomTaskTotalLineBudget) {
    failures.push(
      `server/src/lobby/room_task*: ${totalLines} total lines exceeds room-task runtime budget of ${roomTaskTotalLineBudget}; keep new behavior in existing focused modules or plan another split`,
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

function countLines(source) {
  if (source.length === 0) return 0;
  const lines = source.split(/\r?\n/);
  return lines.at(-1) === "" ? lines.length - 1 : lines.length;
}
