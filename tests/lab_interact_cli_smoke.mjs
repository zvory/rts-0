// Live private-server CLI smoke. The daemon owns and closes Chrome and the Rust server.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cli = path.join(root, "scripts/lab-interact/cli.mjs");
let sessionId;

function call(command, input = {}) {
  const result = spawnSync(process.execPath, [cli, command, JSON.stringify(input)], { cwd: root, encoding: "utf8", maxBuffer: 2 * 1024 * 1024 });
  assert.equal(result.status, 0, `${command} succeeds: ${result.stderr}`);
  const response = JSON.parse(result.stdout);
  assert.equal(response.ok, true, `${command} returns success`);
  return response.result;
}

try {
  call("shutdown");
  const opened = call("open", { viewport: { width: 1000, height: 700, deviceScaleFactor: 1 } });
  sessionId = opened.sessionId;
  assert.equal(opened.workspace.root, fs.realpathSync(root), "CLI daemon serves the selected worktree");
  const catalog = call("catalog", { sessionId, categories: ["units", "players", "commands"] });
  assert.ok(catalog.categories.units.includes("rifleman"), "catalog exposes the normal lab unit catalog");
  call("time", { sessionId, control: { action: "pause" } });
  call("spawn", { sessionId, spawns: [
    { owner: 1, kind: "rifleman", x: 960, y: 960, alias: "shooter" },
    { owner: 2, kind: "rifleman", x: 1248, y: 960, alias: "target" },
  ] });
  call("camera", { sessionId, camera: { action: "focus", refs: ["shooter", "target"], padding: 64 } });
  const screenshot = call("screenshot", {
    sessionId,
    name: "cli-smoke",
    presentation: "clean",
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
    subjects: ["shooter", "target"],
  });
  assert.equal(screenshot.image.mimeType, "image/png", "screenshot identifies PNG metadata without embedding bytes");
  assert.ok(fs.statSync(screenshot.pngPath).size > 4096, "CLI writes a nontrivial PNG artifact");
  const manifest = JSON.parse(fs.readFileSync(screenshot.manifestPath, "utf8"));
  assert.deepEqual(manifest.errors.render, [], "capture manifest reports no render errors");
  call("order", { sessionId, playerId: 1, command: { c: "move", units: ["shooter"], x: 1088, y: 1088 } });
  call("time", { sessionId, control: { action: "step", ticks: 3 } });
  const inspected = call("inspect", { sessionId, refs: ["shooter", "target"], limit: 2 });
  assert.equal(inspected.entities.length, 2, "CLI inspection returns authoritative spawned entities");
} finally {
  if (sessionId) call("close", { sessionId });
  call("shutdown");
}

console.log("✅ lab_interact_cli_smoke.mjs: private CLI session, aliases, command, capture, and cleanup passed");
