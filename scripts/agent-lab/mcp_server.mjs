// Project-local MCP adapter for the bounded Agent Lab browser driver.
// The adapter owns schemas, aliases, session lifetime, and stdio transport; game authority stays
// in the existing lab room and normal client bridge.

import crypto from "node:crypto";
import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { AgentLabDriver, AgentLabDriverError, ensureTestNodeModules } from "./driver.mjs";

const agentLabRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "../..");
const agentLabTestsDir = path.join(agentLabRoot, "tests");
// Reuse the repository's lockfile-keyed test dependency cache rather than using npx or a separate
// install path at MCP launch time. This runs before resolving the SDK peer imports.
ensureTestNodeModules(agentLabTestsDir, "@modelcontextprotocol/sdk");
const requireFromTests = createRequire(path.join(agentLabTestsDir, "package.json"));
const { McpServer } = requireFromTests("@modelcontextprotocol/sdk/server/mcp.js");
const { StdioServerTransport } = requireFromTests("@modelcontextprotocol/sdk/server/stdio.js");
const { z } = requireFromTests("zod/v4");

export const AGENT_LAB_MCP_NAME = "bewegungskrieg-agent-lab";
export const AGENT_LAB_MCP_VERSION = "1.0.0";
export const AGENT_LAB_MCP_LIMITS = Object.freeze({
  maxSessions: 2,
  idleMs: 5 * 60_000,
  maxSpawnBatch: 10,
  maxAliases: 100,
  maxCommandUnits: 100,
  maxInspectRefs: 100,
  maxInspectResults: 100,
  maxFocusRefs: 20,
});

export const AGENT_LAB_MCP_INSTRUCTIONS = [
  "Open a private lab with lab_open, inspect lab_catalog, use short session-local aliases, keep scenes small, confirm authoritative lab_inspect results after mutations, and lab_close when finished. These tools affect only an ephemeral local lab session; they never edit repository files, send arbitrary protocol data, or control deployed rooms.",
  "Pass the returned sessionId to every later tool. Numeric ids are accepted where entity references are accepted, but aliases must be unique and are never guessed. Use lab_reset to restore the setup baseline; it preserves an alias only when an exact unique authoritative match remains after a reset.",
].join(" ");

const ALIAS_RE = /^[A-Za-z][A-Za-z0-9_-]{0,31}$/;
const TOKEN_RE = /^[A-Za-z0-9_]{1,64}$/;
const SESSION_RE = /^lab_[a-f0-9]{32}$/;
const U32_MAX = 0xffff_ffff;
const ALL_CATALOG_CATEGORIES = Object.freeze([
  "maps",
  "players",
  "factions",
  "units",
  "buildings",
  "upgrades",
  "commands",
  "abilities",
]);

const sessionIdSchema = z.string().regex(SESSION_RE, "sessionId must be an Agent Lab session id.");
const aliasSchema = z.string().regex(ALIAS_RE, "Alias must start with a letter and contain only letters, digits, _ or -.");
const u32Schema = z.number().int().min(1).max(U32_MAX);
const entityReferenceSchema = z.union([u32Schema, aliasSchema]);
const finiteNumberSchema = z.number().finite();
const tokenSchema = z.string().regex(TOKEN_RE, "Value must be a safe protocol token.");
const queuedSchema = z.boolean().optional();
const entityRefsSchema = z.array(entityReferenceSchema).min(1).max(AGENT_LAB_MCP_LIMITS.maxCommandUnits);
const unknownRecordSchema = z.record(z.string(), z.unknown());

const labOpenInputSchema = z.object({
  workspaceRoot: z.string().min(1).optional(),
  map: z.string().regex(TOKEN_RE).max(48).optional(),
  seed: z.union([z.string().max(64), z.number().int().min(0).max(U32_MAX)]).optional(),
  scenario: z.string().regex(TOKEN_RE).max(48).optional(),
  viewport: z.object({
    width: z.number().int().min(320).max(4096),
    height: z.number().int().min(240).max(4096),
    deviceScaleFactor: z.number().finite().positive().max(4).optional(),
  }).strict().optional(),
}).strict();

const labCloseInputSchema = z.object({ sessionId: sessionIdSchema }).strict();
const labResetInputSchema = z.object({ sessionId: sessionIdSchema }).strict();
const labCatalogInputSchema = z.object({
  sessionId: sessionIdSchema,
  categories: z.array(z.enum(ALL_CATALOG_CATEGORIES)).max(ALL_CATALOG_CATEGORIES.length).optional(),
}).strict();

const spawnSpecSchema = z.object({
  owner: u32Schema,
  kind: tokenSchema,
  x: finiteNumberSchema,
  y: finiteNumberSchema,
  completed: z.boolean().optional(),
  alias: aliasSchema.optional(),
}).strict();
const labSpawnInputSchema = z.object({
  sessionId: sessionIdSchema,
  spawns: z.array(spawnSpecSchema).min(1).max(AGENT_LAB_MCP_LIMITS.maxSpawnBatch),
}).strict();

const labUpdateInputSchema = z.object({
  sessionId: sessionIdSchema,
  update: z.discriminatedUnion("operation", [
    z.object({ operation: z.literal("move"), entity: entityReferenceSchema, x: finiteNumberSchema, y: finiteNumberSchema }).strict(),
    z.object({ operation: z.literal("owner"), entity: entityReferenceSchema, owner: u32Schema }).strict(),
    z.object({ operation: z.literal("resources"), playerId: u32Schema, steel: z.number().int().min(0).max(U32_MAX), oil: z.number().int().min(0).max(U32_MAX) }).strict(),
    z.object({ operation: z.literal("research"), playerId: u32Schema, upgrade: tokenSchema, completed: z.boolean().optional() }).strict(),
    z.object({ operation: z.literal("godMode"), playerId: u32Schema, enabled: z.boolean().optional() }).strict(),
  ]),
}).strict();

const labRemoveInputSchema = z.object({
  sessionId: sessionIdSchema,
  refs: z.array(entityReferenceSchema).min(1).max(AGENT_LAB_MCP_LIMITS.maxInspectRefs),
}).strict();

const commandSchema = z.discriminatedUnion("c", [
  z.object({ c: z.literal("move"), units: entityRefsSchema, x: finiteNumberSchema, y: finiteNumberSchema, queued: queuedSchema }).strict(),
  z.object({ c: z.literal("attackMove"), units: entityRefsSchema, x: finiteNumberSchema, y: finiteNumberSchema, queued: queuedSchema }).strict(),
  z.object({ c: z.literal("attack"), units: entityRefsSchema, target: entityReferenceSchema, queued: queuedSchema }).strict(),
  z.object({ c: z.literal("deconstruct"), units: entityRefsSchema, target: entityReferenceSchema, queued: queuedSchema }).strict(),
  z.object({ c: z.literal("setupAntiTankGuns"), units: entityRefsSchema, x: finiteNumberSchema, y: finiteNumberSchema, queued: queuedSchema }).strict(),
  z.object({ c: z.literal("tearDownAntiTankGuns"), units: entityRefsSchema }).strict(),
  z.object({ c: z.literal("charge"), units: entityRefsSchema }).strict(),
  z.object({ c: z.literal("useAbility"), ability: tokenSchema, units: entityRefsSchema, x: finiteNumberSchema.optional(), y: finiteNumberSchema.optional(), queued: queuedSchema }).strict(),
  z.object({ c: z.literal("recastAbility"), ability: tokenSchema, units: entityRefsSchema, targetObjectId: u32Schema.optional(), queued: queuedSchema }).strict(),
  z.object({ c: z.literal("setAutocast"), ability: tokenSchema, units: entityRefsSchema, enabled: z.boolean() }).strict(),
  z.object({ c: z.literal("gather"), units: entityRefsSchema, node: entityReferenceSchema, queued: queuedSchema }).strict(),
  z.object({ c: z.literal("build"), units: entityRefsSchema, building: tokenSchema, tileX: z.number().int().min(0).max(U32_MAX), tileY: z.number().int().min(0).max(U32_MAX), queued: queuedSchema }).strict(),
  z.object({ c: z.literal("train"), building: entityReferenceSchema, unit: tokenSchema }).strict(),
  z.object({ c: z.literal("research"), building: entityReferenceSchema, upgrade: tokenSchema }).strict(),
  z.object({ c: z.literal("cancel"), building: entityReferenceSchema }).strict(),
  z.object({ c: z.literal("stop"), units: entityRefsSchema }).strict(),
  z.object({ c: z.literal("holdPosition"), units: entityRefsSchema }).strict(),
  z.object({ c: z.literal("setRally"), building: entityReferenceSchema, x: finiteNumberSchema, y: finiteNumberSchema, kind: z.enum(["move", "attackMove", "attack", "gather", "build"]).optional(), queued: queuedSchema }).strict(),
]);
const labOrderInputSchema = z.object({
  sessionId: sessionIdSchema,
  playerId: u32Schema,
  command: commandSchema,
  ignoreCommandLimits: z.boolean().optional(),
}).strict();

const labTimeInputSchema = z.object({
  sessionId: sessionIdSchema,
  control: z.discriminatedUnion("action", [
    z.object({ action: z.literal("pause") }).strict(),
    z.object({ action: z.literal("resume"), speed: z.number().finite().min(0.01).max(16).optional() }).strict(),
    z.object({ action: z.literal("speed"), speed: z.number().finite().min(0).max(16) }).strict(),
    z.object({ action: z.literal("step"), ticks: z.number().int().min(1).max(100).optional() }).strict(),
    z.object({ action: z.literal("seek"), tick: z.number().int().min(0).max(1_000_000) }).strict(),
  ]),
}).strict();

const labInspectInputSchema = z.object({
  sessionId: sessionIdSchema,
  refs: z.array(entityReferenceSchema).max(AGENT_LAB_MCP_LIMITS.maxInspectRefs).optional(),
  kinds: z.array(tokenSchema).max(32).optional(),
  owners: z.array(u32Schema).max(16).optional(),
  cameraViewport: z.boolean().optional(),
  limit: z.number().int().min(1).max(AGENT_LAB_MCP_LIMITS.maxInspectResults).optional(),
}).strict();

const labCameraInputSchema = z.object({
  sessionId: sessionIdSchema,
  camera: z.discriminatedUnion("action", [
    z.object({ action: z.literal("set"), centerX: finiteNumberSchema.optional(), centerY: finiteNumberSchema.optional(), zoom: z.number().finite().positive().max(16).optional() })
      .strict()
      .refine((value) => value.zoom != null || (value.centerX != null && value.centerY != null), "Set camera requires zoom or both centerX and centerY.")
      .refine((value) => (value.centerX == null) === (value.centerY == null), "centerX and centerY must be provided together."),
    z.object({ action: z.literal("focus"), refs: z.array(entityReferenceSchema).min(1).max(AGENT_LAB_MCP_LIMITS.maxFocusRefs), padding: z.number().finite().min(0).max(1024).optional() }).strict(),
  ]),
}).strict();

const sessionOutputSchema = z.object({
  sessionId: sessionIdSchema,
  workspace: z.object({ root: z.string(), branch: z.string(), head: z.string() }).strict(),
  tick: z.number().int().nullable(),
  players: z.array(unknownRecordSchema),
  status: unknownRecordSchema,
  capabilities: z.object({
    aliases: z.boolean(),
    catalogCategories: z.array(z.string()),
    maxSessions: z.number().int(),
    idleMs: z.number().int(),
  }).strict(),
}).strict();
const closeOutputSchema = z.object({ sessionId: sessionIdSchema, closed: z.boolean() }).strict();
const resetOutputSchema = z.object({
  sessionId: sessionIdSchema,
  result: z.unknown(),
  aliases: z.array(z.object({ alias: z.string(), id: u32Schema }).strict()),
  clearedAliases: z.array(z.string()),
}).strict();
const catalogOutputSchema = z.object({
  sessionId: sessionIdSchema,
  categories: z.record(z.string(), z.array(z.unknown())),
  truncated: z.boolean(),
}).strict();
const mutationOutputSchema = z.object({ sessionId: sessionIdSchema, result: z.unknown() }).strict();
const spawnOutputSchema = z.object({
  sessionId: sessionIdSchema,
  results: z.array(z.object({ alias: z.string().nullable(), id: u32Schema, entity: z.unknown(), result: z.unknown() }).strict()),
}).strict();
const removeOutputSchema = z.object({
  sessionId: sessionIdSchema,
  removed: z.array(z.object({ id: u32Schema, alias: z.string().nullable() }).strict()),
  result: z.unknown(),
}).strict();
const orderOutputSchema = z.object({
  sessionId: sessionIdSchema,
  command: unknownRecordSchema,
  resolved: unknownRecordSchema,
  result: z.unknown(),
}).strict();
const inspectOutputSchema = z.object({
  sessionId: sessionIdSchema,
  entities: z.array(unknownRecordSchema),
  players: z.array(unknownRecordSchema),
  room: z.unknown(),
  camera: z.unknown(),
  truncated: z.boolean(),
  totalMatching: z.number().int(),
}).strict();
const cameraOutputSchema = z.object({ sessionId: sessionIdSchema, camera: z.unknown() }).strict();

export class AgentLabMcpError extends Error {
  constructor(code, message, details = {}) {
    super(message);
    this.name = "AgentLabMcpError";
    this.code = code;
    this.details = details;
  }
}

export class AgentLabSessionManager {
  constructor({
    workspaceRoot = process.cwd(),
    driverFactory = (options) => AgentLabDriver.open(options),
    maxSessions = AGENT_LAB_MCP_LIMITS.maxSessions,
    idleMs = AGENT_LAB_MCP_LIMITS.idleMs,
    now = () => Date.now(),
    log = defaultLog,
  } = {}) {
    this.workspaceRoot = realWorkspaceRoot(workspaceRoot);
    this.driverFactory = driverFactory;
    this.maxSessions = boundedPositiveInteger(maxSessions, "maxSessions", 8);
    this.idleMs = boundedPositiveInteger(idleMs, "idleMs", 3_600_000);
    this.now = now;
    this.log = log;
    this.sessions = new Map();
    this.opening = 0;
    this.closed = false;
    this.reaper = setInterval(() => {
      void this.reapIdle().catch((error) => this.log("idleReapFailed", error));
    }, Math.min(this.idleMs, 30_000));
    this.reaper.unref?.();
  }

  async open(input) {
    if (this.closed) throw new AgentLabMcpError("serverClosed", "The Agent Lab MCP server is shutting down.");
    if (this.sessions.size + this.opening >= this.maxSessions) {
      throw new AgentLabMcpError("sessionLimit", `Agent Lab allows at most ${this.maxSessions} concurrent sessions. Close an existing session first.`);
    }
    const workspaceRoot = resolveRequestedWorkspace(input.workspaceRoot, this.workspaceRoot);
    this.opening += 1;
    try {
      const driver = await this.driverFactory({
        workspaceRoot,
        map: input.map || "Default",
        seed: input.seed == null ? "" : String(input.seed),
        scenario: input.scenario || "blank",
        viewport: input.viewport,
      });
      if (this.closed) {
        await driver.close().catch(() => {});
        throw new AgentLabMcpError("serverClosed", "The Agent Lab MCP server shut down while the session was opening.");
      }
      const sessionId = `lab_${crypto.randomUUID().replaceAll("-", "")}`;
      const session = {
        sessionId,
        driver,
        aliases: new Map(),
        lastUsedAt: this.now(),
        // The browser driver serializes individual bridge calls, but an MCP operation can make
        // several of them around alias validation and reconciliation. Keep each session actor-like
        // so concurrent tool requests cannot observe and then overwrite the same alias state.
        operationTail: Promise.resolve(),
      };
      this.sessions.set(sessionId, session);
      this.log("sessionOpened", { sessionId, workspaceRoot });
      return session;
    } finally {
      this.opening -= 1;
    }
  }

  get(sessionId) {
    const session = this.sessions.get(sessionId);
    if (!session) throw new AgentLabMcpError("unknownSession", "Unknown or closed sessionId. Call lab_open to create a new private lab session.", { sessionId });
    session.lastUsedAt = this.now();
    return session;
  }

  use(sessionId, operation) {
    const session = this.get(sessionId);
    const run = session.operationTail.then(
      () => operation(session),
      () => operation(session),
    );
    // A failed request must not poison the queue for the next request.
    session.operationTail = run.catch(() => {});
    return run;
  }

  async close(sessionId, reason = "explicit") {
    const session = this.sessions.get(sessionId);
    if (!session) return false;
    this.sessions.delete(sessionId);
    // Requests already admitted to this session finish before its browser and server are torn
    // down. Later requests see an unknown session immediately.
    await session.operationTail;
    await session.driver.close().catch((error) => this.log("sessionCloseFailed", { sessionId, reason, error: conciseError(error) }));
    this.log("sessionClosed", { sessionId, reason });
    return true;
  }

  async closeAll(reason = "shutdown") {
    clearInterval(this.reaper);
    const ids = [...this.sessions.keys()];
    await Promise.all(ids.map((sessionId) => this.close(sessionId, reason)));
  }

  async reapIdle() {
    const cutoff = this.now() - this.idleMs;
    const idle = [...this.sessions.values()].filter((session) => session.lastUsedAt <= cutoff);
    await Promise.all(idle.map((session) => this.close(session.sessionId, "idleTimeout")));
  }

  async shutdown(reason = "shutdown") {
    if (this.closed) return;
    this.closed = true;
    await this.closeAll(reason);
  }
}

export function createAgentLabMcpServer(options = {}) {
  const manager = options.manager || new AgentLabSessionManager(options);
  const server = new McpServer(
    { name: AGENT_LAB_MCP_NAME, version: AGENT_LAB_MCP_VERSION },
    { instructions: AGENT_LAB_MCP_INSTRUCTIONS, capabilities: { logging: {} } },
  );

  registerTool(server, "lab_open", {
    title: "Open Agent Lab",
    description: "Start a bounded private local lab session for this trusted worktree. This launches local child processes but never edits repository files.",
    inputSchema: labOpenInputSchema,
    outputSchema: sessionOutputSchema,
    annotations: mutationAnnotations(false),
  }, async (input) => {
    const session = await manager.open(input);
    try {
      const [status, catalog] = await Promise.all([session.driver.status(), session.driver.catalog()]);
      return {
        sessionId: session.sessionId,
        workspace: session.driver.workspace,
        tick: integerOrNull(status.snapshotTick),
        players: Array.isArray(catalog.players) ? catalog.players : [],
        status,
        capabilities: {
          aliases: true,
          catalogCategories: [...ALL_CATALOG_CATEGORIES],
          maxSessions: manager.maxSessions,
          idleMs: manager.idleMs,
        },
      };
    } catch (error) {
      await manager.close(session.sessionId, "openVerificationFailed");
      throw error;
    }
  });

  registerTool(server, "lab_close", {
    title: "Close Agent Lab",
    description: "Idempotently close one private Agent Lab session and its owned local processes. It never changes repository files.",
    inputSchema: labCloseInputSchema,
    outputSchema: closeOutputSchema,
    annotations: mutationAnnotations(true, true),
  }, async ({ sessionId }) => ({ sessionId, closed: await manager.close(sessionId) }));

  registerTool(server, "lab_reset", {
    title: "Reset Agent Lab",
    description: "Restore a private lab to its baseline. Aliases are preserved only for exact unique authoritative matches after reset; other aliases are cleared.",
    inputSchema: labResetInputSchema,
    outputSchema: resetOutputSchema,
    annotations: mutationAnnotations(true),
  }, async ({ sessionId }) => manager.use(sessionId, async (session) => {
    const before = await aliasSnapshots(session);
    const result = await session.driver.reset();
    const reconciliation = await reconcileAliasesAfterReset(session, before);
    return { sessionId, result, ...reconciliation };
  }));

  registerTool(server, "lab_catalog", {
    title: "Inspect Agent Lab Catalog",
    description: "Read the bounded maps, factions, units, buildings, upgrades, players, commands, or abilities available in one private lab session.",
    inputSchema: labCatalogInputSchema,
    outputSchema: catalogOutputSchema,
    annotations: readOnlyAnnotations(),
  }, async ({ sessionId, categories }) => manager.use(sessionId, async (session) => {
    return { sessionId, ...projectCatalog(await session.driver.catalog(), categories) };
  }));

  registerTool(server, "lab_spawn", {
    title: "Spawn Agent Lab Entities",
    description: "Spawn one small batch of entities in a private lab session. Optional aliases are session-local and do not enter game state or repository files.",
    inputSchema: labSpawnInputSchema,
    outputSchema: spawnOutputSchema,
    annotations: mutationAnnotations(false),
  }, async ({ sessionId, spawns }) => manager.use(sessionId, async (session) => {
    validateSpawnAliases(session, spawns);
    const catalog = await session.driver.catalog();
    const playerIds = new Set((catalog.players || []).map((player) => Number(player.id)));
    const spawnableKinds = new Set(flattenFactions(catalog.factions, "units").concat(flattenFactions(catalog.factions, "buildings")));
    for (const spec of spawns) {
      if (!playerIds.has(spec.owner)) throw new AgentLabMcpError("unknownPlayer", `Player ${spec.owner} is not available in this lab session.`);
      if (!spawnableKinds.has(spec.kind)) throw new AgentLabMcpError("invalidKind", `${spec.kind} is not a spawnable unit or building in this lab session.`);
    }
    const results = [];
    for (const spec of spawns) {
      const response = await session.driver.spawn(spec);
      const id = response?.entity?.id ?? response?.result?.outcome?.entityId;
      if (!Number.isInteger(id) || id <= 0) throw new AgentLabMcpError("missingEntityId", "The authoritative spawn result did not include an entity id.");
      if (spec.alias) session.aliases.set(spec.alias, id);
      results.push({ alias: spec.alias || null, id, entity: decorateEntity(response.entity, session.aliases), result: response.result });
    }
    return { sessionId, results };
  }));

  registerTool(server, "lab_update", {
    title: "Update Agent Lab Setup",
    description: "Apply one closed, ephemeral setup update (move, owner, resources, research, or god mode) to a private lab. This never accepts arbitrary state patches or edits repository files.",
    inputSchema: labUpdateInputSchema,
    outputSchema: mutationOutputSchema,
    annotations: mutationAnnotations(false),
  }, async ({ sessionId, update }) => manager.use(sessionId, async (session) => {
    const catalog = await session.driver.catalog();
    await assertKnownPlayer(catalog, update.playerId ?? update.owner);
    let operation;
    if (update.operation === "move") {
      const entity = await resolveEntityReference(session, update.entity);
      operation = { operation: "move", entityId: entity.id, x: update.x, y: update.y };
    } else if (update.operation === "owner") {
      const entity = await resolveEntityReference(session, update.entity);
      await assertKnownPlayer(catalog, update.owner);
      operation = { operation: "reassign", entityId: entity.id, owner: update.owner };
    } else if (update.operation === "research") {
      if (!flattenFactions(catalog.factions, "upgrades").includes(update.upgrade)) {
        throw new AgentLabMcpError("invalidUpgrade", `${update.upgrade} is not an available lab upgrade.`);
      }
      operation = update;
    } else {
      operation = update;
    }
    return { sessionId, result: await session.driver.update(operation) };
  }));

  registerTool(server, "lab_remove", {
    title: "Remove Agent Lab Entities",
    description: "Remove a bounded list of aliases or numeric entity ids from a private lab and clear their aliases. This is ephemeral and never edits repository files.",
    inputSchema: labRemoveInputSchema,
    outputSchema: removeOutputSchema,
    annotations: mutationAnnotations(true),
  }, async ({ sessionId, refs }) => manager.use(sessionId, async (session) => {
    const resolved = await resolveEntityReferences(session, refs);
    const result = await session.driver.remove(resolved.map((entry) => entry.id));
    const removed = resolved.map((entry) => ({ id: entry.id, alias: aliasForEntity(session.aliases, entry.id) }));
    for (const entry of resolved) clearAliasesForEntity(session.aliases, entry.id);
    return { sessionId, removed, result };
  }));

  registerTool(server, "lab_order", {
    title: "Issue Agent Lab Order",
    description: "Validate and issue one normal mirrored gameplay command in a private lab via issueCommandAs. Entity aliases resolve only within this session; arbitrary command JSON is rejected.",
    inputSchema: labOrderInputSchema,
    outputSchema: orderOutputSchema,
    annotations: mutationAnnotations(false),
  }, async ({ sessionId, playerId, command, ignoreCommandLimits = false }) => manager.use(sessionId, async (session) => {
    const catalog = await session.driver.catalog();
    await assertKnownPlayer(catalog, playerId);
    validateCommandCatalog(command, catalog);
    const { command: resolvedCommand, resolved } = await resolveCommand(session, command);
    const result = await session.driver.order({ playerId, command: resolvedCommand, ignoreCommandLimits });
    return { sessionId, command: resolvedCommand, resolved, result };
  }));

  registerTool(server, "lab_time", {
    title: "Control Agent Lab Time",
    description: "Pause, resume, set bounded speed, step, or seek authoritative time in one private lab session. This changes only that ephemeral session.",
    inputSchema: labTimeInputSchema,
    outputSchema: mutationOutputSchema,
    annotations: mutationAnnotations(false),
  }, async ({ sessionId, control }) => manager.use(sessionId, async (session) => {
    return { sessionId, result: await session.driver.time(control) };
  }));

  registerTool(server, "lab_inspect", {
    title: "Inspect Agent Lab State",
    description: "Read concise authoritative entity, player, room, and camera summaries from one private lab. Filters and result limits are bounded; no snapshots or checkpoint payloads are returned.",
    inputSchema: labInspectInputSchema,
    outputSchema: inspectOutputSchema,
    annotations: readOnlyAnnotations(),
  }, async ({ sessionId, refs, kinds, owners, cameraViewport, limit }) => manager.use(sessionId, async (session) => {
    const resolved = refs ? await resolveEntityReferences(session, refs) : [];
    const response = await session.driver.inspect({
      ids: resolved.map((entry) => entry.id),
      kinds,
      owners,
      cameraViewport: cameraViewport === true,
      limit: limit || 25,
    });
    return {
      sessionId,
      entities: (response.entities || []).map((entity) => decorateEntity(entity, session.aliases)),
      players: response.players || [],
      room: response.room || null,
      camera: response.camera || null,
      truncated: response.truncated === true,
      totalMatching: Number.isInteger(response.totalMatching) ? response.totalMatching : 0,
    };
  }));

  registerTool(server, "lab_camera", {
    title: "Set Agent Lab Camera",
    description: "Set a private lab camera center/zoom or focus a bounded alias/id list with padding. This affects only the headless lab presentation, not game authority or repository files.",
    inputSchema: labCameraInputSchema,
    outputSchema: cameraOutputSchema,
    annotations: mutationAnnotations(false),
  }, async ({ sessionId, camera }) => manager.use(sessionId, async (session) => {
    let command;
    if (camera.action === "focus") {
      const resolved = await resolveEntityReferences(session, camera.refs);
      command = { action: "focus", entityIds: resolved.map((entry) => entry.id), padding: camera.padding };
    } else {
      command = camera;
    }
    const response = await session.driver.camera(command);
    return { sessionId, camera: response.camera || response };
  }));

  return { server, manager };
}

function registerTool(server, name, config, handler) {
  server.registerTool(name, config, async (input) => {
    try {
      const structuredContent = await handler(input);
      return {
        content: [{ type: "text", text: boundedText(structuredContent) }],
        structuredContent,
      };
    } catch (error) {
      const normalized = normalizeError(error);
      return {
        isError: true,
        content: [{ type: "text", text: `[${normalized.code}] ${normalized.message}` }],
      };
    }
  });
}

function readOnlyAnnotations() {
  return { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: false };
}

function mutationAnnotations(destructiveHint, idempotentHint = false) {
  return { readOnlyHint: false, destructiveHint, idempotentHint, openWorldHint: false };
}

function projectCatalog(catalog, requested) {
  const categories = [...new Set(requested?.length ? requested : ALL_CATALOG_CATEGORIES)];
  const all = {
    maps: Array.isArray(catalog.maps) ? catalog.maps : [],
    players: Array.isArray(catalog.players) ? catalog.players : [],
    factions: Array.isArray(catalog.factions) ? catalog.factions.map(projectFaction) : [],
    units: uniqueStrings(flattenFactions(catalog.factions, "units")),
    buildings: uniqueStrings(flattenFactions(catalog.factions, "buildings")),
    upgrades: uniqueStrings(flattenFactions(catalog.factions, "upgrades")),
    commands: uniqueStrings(catalog.supportedCommandKinds),
    abilities: uniqueStrings(catalog.abilities),
  };
  return { categories: Object.fromEntries(categories.map((category) => [category, all[category]])), truncated: false };
}

function projectFaction(faction) {
  return {
    id: String(faction?.id || ""),
    label: String(faction?.label || ""),
    units: uniqueStrings(faction?.units),
    buildings: uniqueStrings(faction?.buildings),
    upgrades: uniqueStrings(faction?.upgrades),
  };
}

function flattenFactions(factions, field) {
  return (Array.isArray(factions) ? factions : []).flatMap((faction) => Array.isArray(faction?.[field]) ? faction[field] : []);
}

function uniqueStrings(values) {
  return [...new Set((Array.isArray(values) ? values : []).filter((value) => typeof value === "string"))].sort();
}

function validateSpawnAliases(session, spawns) {
  const aliases = new Set();
  for (const { alias } of spawns) {
    if (!alias) continue;
    if (session.aliases.has(alias) || aliases.has(alias)) {
      throw new AgentLabMcpError("duplicateAlias", `Alias ${JSON.stringify(alias)} is already in use for this session. Choose a new alias.`);
    }
    aliases.add(alias);
  }
  if (session.aliases.size + aliases.size > AGENT_LAB_MCP_LIMITS.maxAliases) {
    throw new AgentLabMcpError("aliasLimit", `A session may hold at most ${AGENT_LAB_MCP_LIMITS.maxAliases} aliases.`);
  }
}

async function assertKnownPlayer(catalog, playerId) {
  if (playerId == null) return;
  const found = (catalog.players || []).some((player) => Number(player?.id) === Number(playerId));
  if (!found) throw new AgentLabMcpError("unknownPlayer", `Player ${playerId} is not available in this lab session.`);
}

function validateCommandCatalog(command, catalog) {
  const commands = new Set(uniqueStrings(catalog.supportedCommandKinds));
  if (!commands.has(command.c)) throw new AgentLabMcpError("unsupportedCommand", `${command.c} is not supported by this lab session.`);
  const validateFrom = (field, values, code, label) => {
    if (command[field] != null && !new Set(values).has(command[field])) {
      throw new AgentLabMcpError(code, `${command[field]} is not an available ${label} in this lab session.`);
    }
  };
  validateFrom("building", flattenFactions(catalog.factions, "buildings"), "invalidKind", "building");
  validateFrom("unit", flattenFactions(catalog.factions, "units"), "invalidKind", "unit");
  validateFrom("upgrade", flattenFactions(catalog.factions, "upgrades"), "invalidUpgrade", "upgrade");
  validateFrom("ability", catalog.abilities, "invalidAbility", "ability");
}

async function resolveCommand(session, command) {
  const wire = { ...command };
  const resolved = {};
  if (Array.isArray(command.units)) {
    const units = await resolveEntityReferences(session, command.units);
    wire.units = units.map((entry) => entry.id);
    resolved.units = units;
  }
  for (const field of ["target", "node", "building"]) {
    if (command[field] == null || (field === "building" && command.c === "build")) continue;
    const entry = await resolveEntityReference(session, command[field]);
    wire[field] = entry.id;
    resolved[field] = entry;
  }
  return { command: wire, resolved };
}

async function resolveEntityReferences(session, references) {
  const entries = [];
  const requestedIds = [];
  for (const reference of references) {
    if (typeof reference === "number") {
      entries.push({ input: reference, id: reference, alias: null });
      requestedIds.push(reference);
      continue;
    }
    const id = session.aliases.get(reference);
    if (!id) throw new AgentLabMcpError("unknownAlias", `Unknown alias ${JSON.stringify(reference)}. Use lab_inspect or lab_spawn to find valid aliases.`);
    entries.push({ input: reference, id, alias: reference });
    requestedIds.push(id);
  }
  const duplicates = new Set();
  if (new Set(requestedIds).size !== requestedIds.length) {
    for (const entry of entries) {
      if (duplicates.has(entry.id)) throw new AgentLabMcpError("duplicateReference", "A command may not resolve the same entity more than once.");
      duplicates.add(entry.id);
    }
  }
  const existing = await session.driver.inspect({ ids: [...new Set(requestedIds)], limit: requestedIds.length });
  const found = new Set((existing.entities || []).map((entity) => entity.id));
  for (const entry of entries) {
    if (found.has(entry.id)) continue;
    if (entry.alias) session.aliases.delete(entry.alias);
    throw new AgentLabMcpError(entry.alias ? "staleAlias" : "unknownEntity", entry.alias
      ? `Alias ${JSON.stringify(entry.alias)} no longer resolves to a current entity. It was cleared.`
      : `Entity ${entry.id} is not in the current authoritative snapshot.`);
  }
  return entries;
}

async function resolveEntityReference(session, reference) {
  return (await resolveEntityReferences(session, [reference]))[0];
}

async function aliasSnapshots(session) {
  if (session.aliases.size === 0) return [];
  const entries = [...session.aliases.entries()].map(([alias, id]) => ({ alias, id }));
  const inspected = await session.driver.inspect({ ids: entries.map((entry) => entry.id), limit: entries.length });
  const byId = new Map((inspected.entities || []).map((entity) => [entity.id, entity]));
  return entries.map((entry) => ({ ...entry, entity: byId.get(entry.id) || null }));
}

async function reconcileAliasesAfterReset(session, before) {
  session.aliases.clear();
  const after = await session.driver.inspect({ limit: AGENT_LAB_MCP_LIMITS.maxInspectResults });
  const claimed = new Set();
  const aliases = [];
  const clearedAliases = [];
  for (const entry of before) {
    const matches = (after.entities || []).filter((entity) => exactAliasMatch(entry.entity, entity) && !claimed.has(entity.id));
    if (matches.length === 1) {
      const id = matches[0].id;
      claimed.add(id);
      session.aliases.set(entry.alias, id);
      aliases.push({ alias: entry.alias, id });
    } else {
      clearedAliases.push(entry.alias);
    }
  }
  return { aliases, clearedAliases };
}

function exactAliasMatch(before, after) {
  return !!before && !!after &&
    before.kind === after.kind && before.owner === after.owner &&
    Number(before.x) === Number(after.x) && Number(before.y) === Number(after.y);
}

function decorateEntity(entity, aliases) {
  if (!entity || typeof entity !== "object") return entity || null;
  return { ...entity, alias: aliasForEntity(aliases, entity.id) };
}

function aliasForEntity(aliases, id) {
  for (const [alias, entityId] of aliases) if (entityId === id) return alias;
  return null;
}

function clearAliasesForEntity(aliases, id) {
  for (const [alias, entityId] of aliases) if (entityId === id) aliases.delete(alias);
}

function resolveRequestedWorkspace(requested, allowed) {
  const candidate = realWorkspaceRoot(requested || allowed);
  if (candidate !== allowed) {
    throw new AgentLabMcpError("workspaceNotAllowed", "lab_open may use only the worktree that launched this project-scoped MCP server.");
  }
  return candidate;
}

function realWorkspaceRoot(value) {
  try {
    return fs.realpathSync(value);
  } catch {
    throw new AgentLabMcpError("invalidWorkspace", `Workspace does not exist: ${String(value)}`);
  }
}

function boundedPositiveInteger(value, label, maximum) {
  const number = Number(value);
  if (!Number.isInteger(number) || number < 1 || number > maximum) throw new AgentLabMcpError("invalidConfig", `${label} must be an integer from 1 to ${maximum}.`);
  return number;
}

function normalizeError(error) {
  if (error instanceof AgentLabMcpError) return error;
  if (error instanceof AgentLabDriverError) return new AgentLabMcpError(error.code || "driverError", conciseError(error));
  return new AgentLabMcpError(error?.code || "toolFailed", conciseError(error));
}

function conciseError(error) {
  const message = String(error?.message || "Agent Lab operation failed.");
  return message.split("\nServer log tail:")[0].slice(0, 1000);
}

function boundedText(value) {
  const text = JSON.stringify(value);
  return text.length <= 12_000 ? text : `${text.slice(0, 11_950)}… (full bounded data is in structuredContent)`;
}

function integerOrNull(value) {
  return Number.isInteger(value) ? value : null;
}

function defaultLog(event, data) {
  const safe = typeof data === "object" && data != null ? data : { detail: String(data || "") };
  process.stderr.write(`${JSON.stringify({ source: "agent-lab-mcp", event, ...safe })}\n`);
}

async function loadDriverFactoryForMain() {
  const modulePath = process.env.RTS_AGENT_LAB_DRIVER_FACTORY_MODULE;
  if (!modulePath) return undefined;
  const resolved = path.resolve(process.cwd(), modulePath);
  const module = await import(pathToFileURL(resolved).href);
  if (typeof module.openAgentLabDriver !== "function") {
    throw new Error("RTS_AGENT_LAB_DRIVER_FACTORY_MODULE must export openAgentLabDriver(options).");
  }
  return module.openAgentLabDriver;
}

export async function main() {
  const driverFactory = await loadDriverFactoryForMain();
  const { server, manager } = createAgentLabMcpServer({ driverFactory });
  let shuttingDown = false;
  const shutdown = async (reason) => {
    if (shuttingDown) return;
    shuttingDown = true;
    await manager.shutdown(reason);
    await server.close().catch(() => {});
  };
  server.server.onclose = () => { void manager.shutdown("transportClosed"); };
  for (const signal of ["SIGINT", "SIGTERM"]) {
    process.once(signal, () => { void shutdown(signal); });
  }
  await server.connect(new StdioServerTransport());
  defaultLog("started", { workspaceRoot: manager.workspaceRoot, maxSessions: manager.maxSessions, idleMs: manager.idleMs });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`${JSON.stringify({ source: "agent-lab-mcp", event: "startupFailed", error: conciseError(error) })}\n`);
    process.exitCode = 1;
  });
}
