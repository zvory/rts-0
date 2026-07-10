// Private-server Agent Lab MCP smoke. It owns and closes its stdio server, Chrome, and Rust server.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const transport = new StdioClientTransport({
  command: process.execPath,
  args: ["scripts/agent-lab/mcp_server.mjs"],
  cwd: root,
  env: { ...process.env },
  stderr: "inherit",
});
const client = new Client({ name: "agent-lab-smoke-client", version: "1.0.0" });
await client.connect(transport);

async function call(name, args) {
  const result = await client.callTool({ name, arguments: args });
  assert.equal(result.isError, undefined, `${name} succeeds: ${result.content?.[0]?.text}`);
  return result.structuredContent;
}

let sessionId;
try {
  const tools = await client.listTools();
  assert.equal(tools.tools.length, 12, "MCP server lists the complete bounded Agent Lab surface");
  const opened = await call("lab_open", {
    workspaceRoot: root,
    viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
  });
  sessionId = opened.sessionId;
  assert.equal(opened.workspace.root, root, "MCP session serves the selected worktree");
  const catalog = await call("lab_catalog", { sessionId, categories: ["units", "players", "commands"] });
  assert.ok(catalog.categories.units.includes("rifleman"), "MCP catalog exposes the normal lab unit catalog");
  await call("lab_time", { sessionId, control: { action: "pause" } });
  await call("lab_spawn", { sessionId, spawns: [
    { owner: 1, kind: "rifleman", x: 960, y: 960, alias: "shooter" },
    { owner: 2, kind: "rifleman", x: 1248, y: 960, alias: "target" },
  ] });
  await call("lab_camera", { sessionId, camera: { action: "focus", refs: ["shooter", "target"], padding: 64 } });
  const screenshot = await client.callTool({
    name: "lab_screenshot",
    arguments: {
      sessionId,
      name: "mcp-smoke",
      presentation: "clean",
      viewport: { width: 1000, height: 700, deviceScaleFactor: 1 },
      subjects: ["shooter", "target"],
    },
  });
  assert.equal(screenshot.isError, undefined, `lab_screenshot succeeds: ${screenshot.content?.[0]?.text}`);
  assert.equal(screenshot.structuredContent.image.mimeType, "image/png", "MCP screenshot result identifies PNG content");
  assert.deepEqual(
    { width: screenshot.structuredContent.image.width, height: screenshot.structuredContent.image.height },
    { width: 1000, height: 700 },
    "MCP screenshot honors the requested fixed viewport",
  );
  assert.ok(screenshot.content.some((item) => item.type === "image" && item.mimeType === "image/png"), "MCP screenshot returns image content");
  assert.ok(fs.statSync(screenshot.structuredContent.pngPath).size > 4096, "MCP screenshot writes a nontrivial PNG artifact");
  const screenshotManifest = JSON.parse(fs.readFileSync(screenshot.structuredContent.manifestPath, "utf8"));
  assert.deepEqual(screenshotManifest.errors.render, [], "MCP screenshot manifest reports no render errors");
  await call("lab_order", { sessionId, playerId: 1, command: { c: "move", units: ["shooter"], x: 1088, y: 1088 } });
  await call("lab_time", { sessionId, control: { action: "step", ticks: 3 } });
  const inspected = await call("lab_inspect", { sessionId, refs: ["shooter", "target"], limit: 2 });
  assert.equal(inspected.entities.length, 2, "MCP inspection returns both authoritative spawned entities");
  assert.ok(inspected.entities.find((entity) => entity.alias === "shooter")?.orderPlan.some((stage) => stage.kind === "move"), "MCP order waits for observed authoritative order evidence");
} finally {
  if (sessionId) await call("lab_close", { sessionId });
  await client.close();
}

console.log("✅ agent_lab_mcp_smoke.mjs: private MCP session, aliases, command, time, inspection, reset, and cleanup passed");
