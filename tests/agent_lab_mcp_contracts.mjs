import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

import {
  AGENT_LAB_MCP_INSTRUCTIONS,
  AGENT_LAB_MCP_LIMITS,
  AgentLabSessionManager,
} from "../scripts/agent-lab/mcp_server.mjs";
import { openAgentLabDriver } from "./fixtures/agent_lab_fake_driver.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
assert.match(AGENT_LAB_MCP_INSTRUCTIONS.slice(0, 512), /lab_open.*lab_catalog.*aliases.*lab_close/s, "server instructions lead with the essential safe workflow");
assert.match(AGENT_LAB_MCP_INSTRUCTIONS.slice(0, 512), /never edit repository files/, "server instructions state the repository-write boundary early");

let now = 0;
const manager = new AgentLabSessionManager({ workspaceRoot: root, driverFactory: openAgentLabDriver, idleMs: 10, now: () => now, log: () => {} });
const idleSession = await manager.open({});
now = 10;
await manager.reapIdle();
assert.equal(manager.sessions.has(idleSession.sessionId), false, "idle sessions close at the documented bound");
await manager.shutdown();

const stderr = [];
const transport = new StdioClientTransport({
  command: process.execPath,
  args: ["scripts/agent-lab/mcp_server.mjs"],
  cwd: root,
  env: { ...process.env, RTS_AGENT_LAB_DRIVER_FACTORY_MODULE: "tests/fixtures/agent_lab_fake_driver.mjs" },
  stderr: "pipe",
});
transport.stderr?.on("data", (chunk) => stderr.push(chunk.toString()));
const client = new Client({ name: "agent-lab-contract-client", version: "1.0.0" });
await client.connect(transport);

const tools = await client.listTools();
const names = tools.tools.map((tool) => tool.name).sort();
assert.deepEqual(names, [
  "lab_camera", "lab_catalog", "lab_close", "lab_inspect", "lab_open", "lab_order",
  "lab_remove", "lab_reset", "lab_spawn", "lab_time", "lab_update",
], "MCP server exposes exactly the Phase 2 tool allowlist");
for (const tool of tools.tools) {
  assert.ok(tool.inputSchema && tool.outputSchema, `${tool.name} publishes both input and structured output schemas`);
}
assert.equal(tools.tools.find((tool) => tool.name === "lab_catalog").annotations?.readOnlyHint, true, "catalog is annotated read-only");
assert.equal(tools.tools.find((tool) => tool.name === "lab_inspect").annotations?.readOnlyHint, true, "inspection is annotated read-only");

async function call(name, args) {
  const result = await client.callTool({ name, arguments: args });
  assert.equal(result.isError, undefined, `${name} succeeds: ${result.content?.[0]?.text}`);
  assert.ok(result.structuredContent, `${name} returns structured content`);
  return result.structuredContent;
}

async function expectRejected(name, args, pattern) {
  const result = await client.callTool({ name, arguments: args });
  assert.equal(result.isError, true, `${name} rejects invalid input`);
  assert.match(result.content?.[0]?.text || "", pattern, `${name} returns an actionable rejection`);
}

const opened = await call("lab_open", { workspaceRoot: root });
const sessionId = opened.sessionId;
assert.equal(opened.capabilities.maxSessions, AGENT_LAB_MCP_LIMITS.maxSessions, "open returns the bounded session limit");
const catalog = await call("lab_catalog", { sessionId, categories: ["units", "buildings", "commands", "abilities"] });
assert.deepEqual(catalog.categories.units, ["rifleman", "tank"], "catalog projects bounded spawnable units");
assert.deepEqual(catalog.categories.abilities, ["charge", "smoke"], "catalog exposes known ability ids for command validation");
await expectRejected("lab_catalog", { sessionId, categories: ["units"], unexpected: true }, /Unrecognized key/);
await expectRejected("lab_time", { sessionId, control: { action: "step", ticks: 101 } }, /Too big/);
await expectRejected("lab_spawn", { sessionId, spawns: [{ owner: 1, kind: "rifleman", x: 960, y: 960, alias: "not valid" }] }, /Alias must start/);
await expectRejected("lab_spawn", { sessionId, spawns: Array.from({ length: 11 }, (_, index) => ({ owner: 1, kind: "rifleman", x: 960 + index, y: 960 })) }, /Too big/);
const spawned = await call("lab_spawn", { sessionId, spawns: [
  { owner: 1, kind: "rifleman", x: 960, y: 960, alias: "shooter" },
  { owner: 2, kind: "rifleman", x: 1248, y: 960, alias: "target" },
] });
assert.equal(spawned.results[0].alias, "shooter", "spawn records the requested session alias");
await call("lab_update", { sessionId, update: { operation: "move", entity: "target", x: 1280, y: 960 } });
await call("lab_order", { sessionId, playerId: 1, command: { c: "attack", units: ["shooter"], target: "target" } });
await call("lab_time", { sessionId, control: { action: "pause" } });
await call("lab_time", { sessionId, control: { action: "step", ticks: 2 } });
const inspected = await call("lab_inspect", { sessionId, refs: ["shooter", "target"], cameraViewport: true, limit: 2 });
assert.deepEqual(inspected.entities.map((entity) => entity.alias).sort(), ["shooter", "target"], "inspect returns both numeric entities and their resolved aliases");
await call("lab_camera", { sessionId, camera: { action: "focus", refs: ["shooter", "target"], padding: 32 } });
await call("lab_spawn", { sessionId, spawns: [{ owner: 2, kind: "rifleman", x: 1312, y: 960, alias: "doomed" }] });
await call("lab_order", { sessionId, playerId: 1, command: { c: "deconstruct", units: ["shooter"], target: "doomed" } });
const stale = await client.callTool({ name: "lab_inspect", arguments: { sessionId, refs: ["doomed"] } });
assert.equal(stale.isError, true, "authoritatively removed aliases are rejected as stale");
assert.match(stale.content?.[0]?.text || "", /staleAlias/, "stale alias error clears the unusable alias without guessing");
await call("lab_remove", { sessionId, refs: ["target"] });
const reset = await call("lab_reset", { sessionId });
assert.deepEqual(reset.clearedAliases, ["shooter"], "reset deliberately clears aliases that cannot be remapped without guessing");

const duplicate = await client.callTool({ name: "lab_spawn", arguments: { sessionId, spawns: [
  { owner: 1, kind: "rifleman", x: 960, y: 960, alias: "same" },
  { owner: 1, kind: "rifleman", x: 992, y: 960, alias: "same" },
] } });
assert.equal(duplicate.isError, true, "duplicate aliases are returned as structured MCP tool errors");
assert.match(duplicate.content?.[0]?.text || "", /duplicateAlias/, "duplicate alias error is actionable");

const second = await call("lab_open", {});
const third = await client.callTool({ name: "lab_open", arguments: {} });
assert.equal(third.isError, true, "concurrent session limit is enforced");
assert.match(third.content?.[0]?.text || "", /sessionLimit/, "session limit error tells the caller how to recover");
const crossSession = await client.callTool({ name: "lab_inspect", arguments: { sessionId: second.sessionId, refs: ["shooter"] } });
assert.equal(crossSession.isError, true, "aliases cannot cross-control another session");
assert.match(crossSession.content?.[0]?.text || "", /unknownAlias/, "cross-session alias errors remain explicit");

await call("lab_close", { sessionId });
await call("lab_close", { sessionId });
await call("lab_close", { sessionId: second.sessionId });
await call("lab_open", {});
const childPid = transport.pid;
await client.close();
await waitForChildExit(childPid);
assert.ok(stderr.some((line) => line.includes("agent-lab-mcp")), "server logs are written to stderr while the stdio client receives valid MCP traffic");

console.log("✅ agent_lab_mcp_contracts.mjs: schemas, aliases, session bounds, stdio MCP, and cleanup passed");

async function waitForChildExit(pid) {
  const deadline = Date.now() + 2_000;
  while (Date.now() < deadline) {
    try {
      process.kill(pid, 0);
    } catch (error) {
      if (error?.code === "ESRCH") return;
      throw error;
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  assert.fail("stdio transport shutdown must tear down the Agent Lab MCP child and its sessions");
}
