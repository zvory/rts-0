// Transport-neutral, bounded command service for Lab Interact.

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { LabInteractDriver, LabInteractDriverError } from "./driver.mjs";
import { RECORDING_LIMITS } from "./recording.mjs";
import { FIXED_CAPTURE_LIMITS } from "./fixed_capture.mjs";

export const LAB_INTERACT_LIMITS = Object.freeze({
  maxSessions: 1,
  maxSpawnBatch: 10,
  maxAliases: 100,
  maxCommandUnits: 100,
  maxInspectRefs: 100,
  maxInspectResults: 100,
  maxFocusRefs: 20,
  maxScreenshotSubjects: 20,
  maxArtifactBytes: 8 * 1024 * 1024,
  maxAliasSidecarBytes: 64 * 1024,
  maxRecordingOperations: RECORDING_LIMITS.maxOperations,
  defaultRecordingDurationMs: RECORDING_LIMITS.defaultDurationMs,
  maxRecordingDurationMs: RECORDING_LIMITS.maxDurationMs,
  maxFixedCaptureFrames: FIXED_CAPTURE_LIMITS.maxFrames,
});

export const LAB_INTERACT_COMMANDS = Object.freeze([
  "open", "close", "reset", "catalog", "spawn", "update", "remove", "order",
  "time", "inspect", "camera", "screenshot", "status", "shutdown",
  "export", "import", "artifact-inspect",
  "record-start", "record-stop",
  "capture-fixed",
  "capture-cancel",
]);

const ALL_CATALOG_CATEGORIES = Object.freeze([
  "maps", "players", "factions", "units", "buildings", "upgrades", "commands", "abilities",
]);
const ALIAS_RE = /^[A-Za-z][A-Za-z0-9_-]{0,31}$/;
const TOKEN_RE = /^[A-Za-z0-9_]{1,64}$/;
const SESSION_RE = /^lab_[a-f0-9]{32}$/;
const U32_MAX = 0xffff_ffff;
const COMMAND_FIELDS = Object.freeze({
  move: ["c", "units", "x", "y", "queued"],
  attackMove: ["c", "units", "x", "y", "queued"],
  attack: ["c", "units", "target", "queued"],
  deconstruct: ["c", "units", "target", "queued"],
  setupAntiTankGuns: ["c", "units", "x", "y", "queued"],
  tearDownAntiTankGuns: ["c", "units"],
  charge: ["c", "units"],
  useAbility: ["c", "ability", "units", "x", "y", "queued"],
  recastAbility: ["c", "ability", "units", "targetObjectId", "queued"],
  setAutocast: ["c", "ability", "units", "enabled"],
  gather: ["c", "units", "node", "queued"],
  build: ["c", "units", "building", "tileX", "tileY", "queued"],
  train: ["c", "building", "unit"],
  research: ["c", "building", "upgrade"],
  cancel: ["c", "building"],
  stop: ["c", "units"],
  holdPosition: ["c", "units"],
  setRally: ["c", "building", "x", "y", "kind", "queued"],
});

export class LabInteractError extends Error {
  constructor(code, message, details = {}) {
    super(message);
    this.name = "LabInteractError";
    this.code = code;
    this.details = details;
  }
}

export class LabInteractService {
  constructor({
    workspaceRoot = process.cwd(),
    driverFactory = (options) => LabInteractDriver.open(options),
    log = () => {},
  } = {}) {
    this.workspaceRoot = realWorkspaceRoot(workspaceRoot);
    this.driverFactory = driverFactory;
    this.maxSessions = LAB_INTERACT_LIMITS.maxSessions;
    this.log = log;
    this.sessions = new Map();
    this.openPromise = null;
    this.closePromise = null;
    this.closed = false;
  }

  async execute(command, rawInput = {}) {
    if (!LAB_INTERACT_COMMANDS.includes(command)) {
      throw new LabInteractError("unknownCommand", `Unknown command ${JSON.stringify(command)}.`);
    }
    const input = validateInput(command, rawInput);
    if (command === "shutdown") return { shuttingDown: true };
    if (command === "status") return this.status(input);
    if (command === "open") return this.open(input);
    if (command === "close") return { sessionId: input.sessionId, closed: await this.close(input.sessionId) };
    if (command === "capture-cancel") {
      const session = this.get(input.sessionId);
      return { sessionId: input.sessionId, ...session.driver.cancelFixedCapture() };
    }
    return this.use(input.sessionId, async (session) => {
      const result = await this.executeSession(command, session, input);
      if (["reset", "spawn", "update", "remove", "order", "time"].includes(command)) session.sceneRevision += 1;
      if (!command.startsWith("record-")) {
        const recorder = session.driver.recordingStatus?.();
        if (recorder?.active) {
          const operation = recordingOperation(command, input, result);
          session.driver.recordAcceptedOperation?.(
            operation,
            [...session.aliases].map(([alias, id]) => ({ alias, id })),
          );
        }
      }
      return result;
    });
  }

  async open(input) {
    if (this.closed) throw new LabInteractError("serviceClosed", "Lab Interact is shutting down.");
    const workspaceRoot = resolveRequestedWorkspace(input.workspaceRoot, this.workspaceRoot);
    let existing = this.sessions.values().next().value;
    if (existing) return this.use(existing.sessionId, (session) => this.describeSession(session));
    await this.closePromise;
    existing = this.sessions.values().next().value;
    if (existing) return this.use(existing.sessionId, (session) => this.describeSession(session));
    if (this.openPromise) return this.openPromise;
    this.openPromise = (async () => {
      const driver = await this.driverFactory({
        workspaceRoot,
        map: input.map || "Default",
        seed: input.seed == null ? "" : String(input.seed),
        scenario: input.scenario || "blank",
        viewport: input.viewport,
        baseUrl: process.env.RTS_LAB_INTERACT_BASE_URL || "",
      });
      if (this.closed) {
        await driver.close().catch(() => {});
        throw new LabInteractError("serviceClosed", "Lab Interact shut down while the session was opening.");
      }
      const sessionId = `lab_${crypto.randomUUID().replaceAll("-", "")}`;
      const session = {
        sessionId, driver, aliases: new Map(), operationTail: Promise.resolve(), sceneRevision: 0,
        sceneIdentity: { source: "launch", scenario: input.scenario || "blank", map: input.map || "Default", seed: input.seed ?? null },
      };
      this.sessions.set(sessionId, session);
      try {
        return await this.describeSession(session);
      } catch (error) {
        await this.close(sessionId, "openVerificationFailed");
        throw error;
      }
    })();
    try { return await this.openPromise; } finally { this.openPromise = null; }
  }

  async describeSession(session) {
    const [status, catalog] = await Promise.all([session.driver.status(), session.driver.catalog()]);
    return {
      sessionId: session.sessionId,
      workspace: session.driver.workspace,
      tick: Number.isInteger(status.snapshotTick) ? status.snapshotTick : null,
      players: Array.isArray(catalog.players) ? catalog.players : [],
      status,
      capabilities: {
        aliases: true,
        catalogCategories: [...ALL_CATALOG_CATEGORIES],
        maxSessions: this.maxSessions,
      },
    };
  }

  get(sessionId) {
    const session = this.sessions.get(sessionId);
    if (!session) throw new LabInteractError("unknownSession", "Unknown or closed sessionId. Run open first.");
    return session;
  }

  use(sessionId, operation) {
    const session = this.get(sessionId);
    const run = session.operationTail.then(() => operation(session), () => operation(session));
    session.operationTail = run.catch(() => {});
    return run;
  }

  async close(sessionId, reason = "explicit") {
    const session = this.sessions.get(sessionId);
    if (!session) return false;
    this.sessions.delete(sessionId);
    if (session.driver.fixedCaptureStatus?.().active) session.driver.cancelFixedCapture?.();
    const closing = (async () => {
      await session.operationTail;
      await session.driver.close().catch((error) => this.log("sessionCloseFailed", {
        sessionId, reason, error: conciseError(error),
      }));
      return true;
    })();
    this.closePromise = closing;
    try { return await closing; } finally { if (this.closePromise === closing) this.closePromise = null; }
  }

  async shutdown(reason = "shutdown") {
    if (this.closed) return;
    this.closed = true;
    await this.openPromise?.catch(() => {});
    await this.closePromise?.catch(() => {});
    await Promise.all([...this.sessions.keys()].map((sessionId) => this.close(sessionId, reason)));
  }

  async status({ sessionId } = {}) {
    if (sessionId) {
      const session = this.get(sessionId);
      const fixedCapture = session.driver.fixedCaptureStatus?.() || { active: false };
      if (fixedCapture.active) {
        return {
          sessionId,
          status: session.driver.fixedCapture?.startStatus || { ready: true },
          aliases: [...session.aliases].map(([alias, id]) => ({ alias, id })),
          recorder: session.driver.recordingStatus?.() || { active: false },
          fixedCapture,
        };
      }
      return this.use(sessionId, async (session) => ({
        sessionId,
        status: await session.driver.status(),
        aliases: [...session.aliases].map(([alias, id]) => ({ alias, id })),
        recorder: session.driver.recordingStatus?.() || { active: false },
        fixedCapture,
      }));
    }
    return {
      workspaceRoot: this.workspaceRoot,
      opening: this.openPromise != null,
      closing: this.closePromise != null,
      sessions: [...this.sessions.values()].map((session) => ({
        sessionId: session.sessionId,
        aliases: session.aliases.size,
      })),
      maxSessions: this.maxSessions,
    };
  }

  async executeSession(command, session, input) {
    const sessionId = session.sessionId;
    if (command === "reset") {
      const before = await aliasSnapshots(session);
      const result = await session.driver.reset();
      return { sessionId, result, ...await reconcileAliasesAfterReset(session, before) };
    }
    if (command === "catalog") {
      return { sessionId, ...projectCatalog(await session.driver.catalog(), input.categories) };
    }
    if (command === "spawn") return spawn(session, input.spawns);
    if (command === "update") return update(session, input.update);
    if (command === "remove") {
      const resolved = await resolveEntityReferences(session, input.refs);
      const result = await session.driver.remove(resolved.map((entry) => entry.id));
      const removed = resolved.map((entry) => ({ id: entry.id, alias: aliasForEntity(session.aliases, entry.id) }));
      for (const entry of resolved) clearAliasesForEntity(session.aliases, entry.id);
      return { sessionId, removed, result };
    }
    if (command === "order") return order(session, input);
    if (command === "time") return { sessionId, result: await session.driver.time(input.control) };
    if (command === "inspect") return inspect(session, input);
    if (command === "camera") return camera(session, input.camera);
    if (command === "screenshot") return screenshot(session, input);
    if (command === "export") return exportArtifact(this.workspaceRoot, session, input);
    if (command === "import") return importArtifact(this.workspaceRoot, session, input);
    if (command === "artifact-inspect") return inspectArtifact(this.workspaceRoot, session, input);
    if (command === "record-start") {
      const result = await session.driver.recordStart({ ...input, sessionId });
      return { sessionId, recorder: result };
    }
    if (command === "record-stop") {
      const result = await session.driver.recordStop({
        aliases: [...session.aliases].map(([alias, id]) => ({ alias, id })),
      });
      return { sessionId, ...result };
    }
    if (command === "capture-fixed") {
      const result = await session.driver.captureFixed({
        ...input, sessionId, sceneIdentity: session.sceneIdentity, sceneRevision: session.sceneRevision,
        aliases: [...session.aliases].map(([alias, id]) => ({ alias, id })),
      });
      return { sessionId, ...result };
    }
    throw new LabInteractError("unknownCommand", `Unknown session command ${command}.`);
  }
}

export function validateCommandInput(command, input) {
  if (!LAB_INTERACT_COMMANDS.includes(command)) {
    throw new LabInteractError("unknownCommand", `Unknown command ${JSON.stringify(command)}.`);
  }
  return validateInput(command, input);
}

async function spawn(session, spawns) {
  validateSpawnAliases(session, spawns);
  const catalog = await session.driver.catalog();
  const playerIds = new Set((catalog.players || []).map((player) => Number(player.id)));
  const spawnableKinds = new Set(flattenFactions(catalog.factions, "units").concat(flattenFactions(catalog.factions, "buildings")));
  for (const spec of spawns) {
    if (!playerIds.has(spec.owner)) throw new LabInteractError("unknownPlayer", `Player ${spec.owner} is not available.`);
    if (!spawnableKinds.has(spec.kind)) throw new LabInteractError("invalidKind", `${spec.kind} is not spawnable.`);
  }
  const results = [];
  for (const spec of spawns) {
    const response = await session.driver.spawn(spec);
    const id = response?.entity?.id ?? response?.result?.outcome?.entityId;
    if (!Number.isInteger(id) || id <= 0) throw new LabInteractError("missingEntityId", "Spawn did not return an entity id.");
    if (spec.alias) session.aliases.set(spec.alias, id);
    results.push({ alias: spec.alias || null, id, entity: decorateEntity(response.entity, session.aliases), result: response.result });
  }
  return { sessionId: session.sessionId, results };
}

async function update(session, value) {
  const catalog = await session.driver.catalog();
  assertKnownPlayer(catalog, value.playerId ?? value.owner);
  let operation = value;
  if (value.operation === "move") {
    const entity = await resolveEntityReference(session, value.entity);
    operation = { operation: "move", entityId: entity.id, x: value.x, y: value.y };
  } else if (value.operation === "owner") {
    const entity = await resolveEntityReference(session, value.entity);
    operation = { operation: "reassign", entityId: entity.id, owner: value.owner };
  } else if (value.operation === "research" && !flattenFactions(catalog.factions, "upgrades").includes(value.upgrade)) {
    throw new LabInteractError("invalidUpgrade", `${value.upgrade} is not an available lab upgrade.`);
  }
  return { sessionId: session.sessionId, result: await session.driver.update(operation) };
}

async function order(session, { playerId, command, ignoreCommandLimits = false }) {
  const catalog = await session.driver.catalog();
  assertKnownPlayer(catalog, playerId);
  validateCommandCatalog(command, catalog);
  const { command: resolvedCommand, resolved } = await resolveCommand(session, command);
  const result = await session.driver.order({ playerId, command: resolvedCommand, ignoreCommandLimits });
  return { sessionId: session.sessionId, command: resolvedCommand, resolved, result };
}

async function inspect(session, { refs, kinds, owners, cameraViewport, limit }) {
  const resolved = refs ? await resolveEntityReferences(session, refs) : [];
  const response = await session.driver.inspect({
    ids: resolved.map((entry) => entry.id), kinds, owners,
    cameraViewport: cameraViewport === true, limit: limit || 25,
  });
  return {
    sessionId: session.sessionId,
    entities: (response.entities || []).map((entity) => decorateEntity(entity, session.aliases)),
    players: response.players || [], room: response.room || null, camera: response.camera || null,
    truncated: response.truncated === true,
    totalMatching: Number.isInteger(response.totalMatching) ? response.totalMatching : 0,
  };
}

async function camera(session, value) {
  let command = value;
  if (value.action === "focus") {
    const resolved = await resolveEntityReferences(session, value.refs);
    command = { action: "focus", entityIds: resolved.map((entry) => entry.id), padding: value.padding };
  }
  const response = await session.driver.camera(command);
  return { sessionId: session.sessionId, camera: response.camera || response };
}

async function screenshot(session, { name = "scene", presentation = "clean", viewport, subjects }) {
  const resolved = subjects ? await resolveEntityReferences(session, subjects) : [];
  const inspected = resolved.length
    ? await session.driver.inspect({ ids: resolved.map((entry) => entry.id), limit: resolved.length })
    : { entities: [] };
  const entitiesById = new Map((inspected.entities || []).map((entity) => [entity.id, entity]));
  const subjectSummaries = resolved.map((entry) => decorateEntity(entitiesById.get(entry.id), session.aliases));
  const capture = await session.driver.screenshot({
    sessionId: session.sessionId, name, presentation, viewport,
    subjectIds: resolved.map((entry) => entry.id), subjectSummaries,
    request: { command: "screenshot", sessionId: session.sessionId, name, presentation, viewport, subjects: resolved },
  });
  return {
    sessionId: session.sessionId,
    pngPath: capture.pngPath,
    manifestPath: capture.manifestPath,
    presentation: capture.presentation,
    image: {
      mimeType: capture.image.mimeType,
      bytes: capture.image.bytes,
      width: capture.image.width,
      height: capture.image.height,
    },
    readiness: capture.readiness,
  };
}

function validateInput(command, value) {
  record(value, "input");
  const session = () => sessionId(value.sessionId);
  if (command === "shutdown") return exact(value, [], "shutdown");
  if (command === "status") {
    exact(value, ["sessionId"], "status");
    if (value.sessionId != null) session();
    return value;
  }
  if (command === "open") {
    exact(value, ["workspaceRoot", "map", "seed", "scenario", "viewport"], "open");
    if (value.workspaceRoot != null && (typeof value.workspaceRoot !== "string" || !value.workspaceRoot)) invalid("open.workspaceRoot", "must be a non-empty string");
    if (value.map != null) token(value.map, "open.map", 48);
    if (value.scenario != null) token(value.scenario, "open.scenario", 48);
    if (value.seed != null && !((typeof value.seed === "string" && value.seed.length <= 64) || isInteger(value.seed, 0, U32_MAX))) invalid("open.seed", "must be a bounded string or unsigned integer");
    if (value.viewport != null) viewport(value.viewport, 4096, "open.viewport");
    return value;
  }
  session();
  if (command === "close" || command === "reset") return exact(value, ["sessionId"], command);
  if (command === "catalog") {
    exact(value, ["sessionId", "categories"], command);
    if (value.categories != null) array(value.categories, "catalog.categories", 0, ALL_CATALOG_CATEGORIES.length, (entry) => {
      if (!ALL_CATALOG_CATEGORIES.includes(entry)) invalid("catalog.categories", "contains an unknown category");
    });
  } else if (command === "spawn") {
    exact(value, ["sessionId", "spawns"], command);
    array(value.spawns, "spawn.spawns", 1, LAB_INTERACT_LIMITS.maxSpawnBatch, (spec, index) => {
      record(spec, `spawn.spawns[${index}]`); exact(spec, ["owner", "kind", "x", "y", "completed", "alias"], "spawn spec");
      u32(spec.owner, "spawn.owner"); token(spec.kind, "spawn.kind"); finite(spec.x, "spawn.x"); finite(spec.y, "spawn.y");
      optionalBoolean(spec.completed, "spawn.completed"); if (spec.alias != null) alias(spec.alias);
    });
  } else if (command === "update") {
    exact(value, ["sessionId", "update"], command); validateUpdate(value.update);
  } else if (command === "remove") {
    exact(value, ["sessionId", "refs"], command); refs(value.refs, "remove.refs", 1, LAB_INTERACT_LIMITS.maxInspectRefs);
  } else if (command === "order") {
    exact(value, ["sessionId", "playerId", "command", "ignoreCommandLimits"], command);
    u32(value.playerId, "order.playerId"); optionalBoolean(value.ignoreCommandLimits, "order.ignoreCommandLimits"); validateCommand(value.command);
  } else if (command === "time") {
    exact(value, ["sessionId", "control"], command); validateTime(value.control);
  } else if (command === "inspect") {
    exact(value, ["sessionId", "refs", "kinds", "owners", "cameraViewport", "limit"], command);
    if (value.refs != null) refs(value.refs, "inspect.refs", 0, LAB_INTERACT_LIMITS.maxInspectRefs);
    if (value.kinds != null) array(value.kinds, "inspect.kinds", 0, 32, (entry) => token(entry, "inspect.kind"));
    if (value.owners != null) array(value.owners, "inspect.owners", 0, 16, (entry) => u32(entry, "inspect.owner"));
    optionalBoolean(value.cameraViewport, "inspect.cameraViewport");
    if (value.limit != null) integer(value.limit, "inspect.limit", 1, LAB_INTERACT_LIMITS.maxInspectResults);
  } else if (command === "camera") {
    exact(value, ["sessionId", "camera"], command); validateCamera(value.camera);
  } else if (command === "screenshot") {
    exact(value, ["sessionId", "name", "presentation", "viewport", "subjects"], command);
    if (value.name != null && !/^[A-Za-z0-9_-]{1,48}$/.test(value.name)) invalid("screenshot.name", "must be a safe artifact token");
    if (value.presentation != null && !["clean", "normal"].includes(value.presentation)) invalid("screenshot.presentation", "must be clean or normal");
    if (value.viewport != null) viewport(value.viewport, 2048, "screenshot.viewport");
    if (value.subjects != null) refs(value.subjects, "screenshot.subjects", 0, LAB_INTERACT_LIMITS.maxScreenshotSubjects);
  } else if (command === "export") {
    exact(value, ["sessionId", "kind", "name", "reproduction"], command);
    artifactKind(value.kind, "export.kind");
    const maxNameBytes = value.kind === "setup" ? 80 : 120;
    if (value.name != null && (typeof value.name !== "string" || Buffer.byteLength(value.name) > maxNameBytes)) invalid("export.name", `must be at most ${maxNameBytes} UTF-8 bytes`);
    optionalBoolean(value.reproduction, "export.reproduction");
  } else if (command === "import") {
    exact(value, ["sessionId", "kind", "artifactId", "path"], command);
    artifactKind(value.kind, "import.kind");
    artifactSelector(value, "import");
  } else if (command === "artifact-inspect") {
    exact(value, ["sessionId", "kind", "artifactId", "path"], command);
    if (value.kind != null) artifactKind(value.kind, "artifact-inspect.kind");
    artifactSelector(value, "artifact-inspect");
  } else if (command === "record-start") {
    exact(value, ["sessionId", "name", "maxDurationMs", "viewport", "crop", "scale"], command);
    if (value.name != null && !/^[A-Za-z0-9_-]{1,48}$/.test(value.name)) invalid("record-start.name", "must be a safe artifact token");
    if (value.maxDurationMs != null) integer(value.maxDurationMs, "record-start.maxDurationMs", 1_000, LAB_INTERACT_LIMITS.maxRecordingDurationMs);
    if (value.viewport != null) viewport(value.viewport, 2048, "record-start.viewport");
    if (value.crop != null) recordingCrop(value.crop);
    if (value.scale != null) boundedNumber(value.scale, "record-start.scale", 0.25, 1);
  } else if (command === "record-stop") {
    exact(value, ["sessionId"], command);
  } else if (command === "capture-fixed") {
    exact(value, ["sessionId", "name", "fps", "frameCount", "viewport"], command);
    if (value.name != null && !/^[A-Za-z0-9_-]{1,48}$/.test(value.name)) invalid("capture-fixed.name", "must be a safe artifact token");
    if (value.fps != null) integer(value.fps, "capture-fixed.fps", FIXED_CAPTURE_LIMITS.minFps, FIXED_CAPTURE_LIMITS.maxFps);
    if (value.frameCount != null) integer(value.frameCount, "capture-fixed.frameCount", 1, FIXED_CAPTURE_LIMITS.maxFrames);
    if (value.viewport != null) viewport(value.viewport, 2048, "capture-fixed.viewport");
  } else if (command === "capture-cancel") {
    exact(value, ["sessionId"], command);
  }
  return value;
}

async function exportArtifact(workspaceRoot, session, { kind, name = "", reproduction = false }) {
  const artifactId = `artifact_${crypto.randomUUID().replaceAll("-", "")}`;
  const directory = artifactDirectory(workspaceRoot);
  fs.mkdirSync(directory, { recursive: true });
  let artifact;
  if (kind === "setup") {
    artifact = (await session.driver.exportSetup(name)).scenario;
  } else {
    artifact = JSON.parse((await session.driver.exportReplay(name)).bytes.toString("utf8"));
  }
  const bytes = Buffer.from(`${JSON.stringify(artifact, null, 2)}\n`);
  if (bytes.length > LAB_INTERACT_LIMITS.maxArtifactBytes) {
    throw new LabInteractError("artifactTooLarge", "Artifact exceeds the 8 MiB local file bound.");
  }
  const artifactPath = path.join(directory, `${artifactId}.${kind}.json`);
  const sidecarPath = path.join(directory, `${artifactId}.aliases.json`);
  const aliases = [...session.aliases].map(([alias, id]) => ({ alias, id }));
  const sidecar = {
    schemaVersion: 1,
    artifactId,
    kind,
    artifactFile: path.basename(artifactPath),
    aliases,
    reproduction: reproduction ? reproductionSummary(kind, artifactId, aliases) : null,
  };
  fs.writeFileSync(artifactPath, bytes, { mode: 0o600 });
  fs.writeFileSync(sidecarPath, `${JSON.stringify(sidecar, null, 2)}\n`, { mode: 0o600 });
  return {
    sessionId: session.sessionId,
    artifactId,
    kind,
    path: artifactPath,
    sidecarPath,
    bytes: bytes.length,
    ...artifactSummary(kind, artifact, aliases),
    reproduction: sidecar.reproduction,
  };
}

async function importArtifact(workspaceRoot, session, selector) {
  const selected = readArtifact(workspaceRoot, selector);
  let importResult;
  let reconciliation;
  if (selected.kind === "setup") {
    importResult = await session.driver.importSetup(selected.artifact);
    reconciliation = await reconcileImportedAliases(session, selected.aliases, importResult.entityIdMap || []);
  } else {
    importResult = await session.driver.importReplay(selected.bytes);
    reconciliation = await validateImportedAliases(session, selected.aliases);
  }
  session.sceneIdentity = { source: "artifact", artifactId: selected.artifactId, kind: selected.kind, path: selected.path };
  session.sceneRevision = 0;
  return {
    sessionId: session.sessionId,
    artifactId: selected.artifactId,
    kind: selected.kind,
    path: selected.path,
    imported: true,
    tick: (await session.driver.status()).snapshotTick ?? null,
    aliases: reconciliation,
    validation: { ok: true, authority: selected.kind === "setup" ? "checkpoint import" : "replay room rebuild" },
    result: importResult,
  };
}

async function inspectArtifact(workspaceRoot, session, selector) {
  const selected = readArtifact(workspaceRoot, selector);
  const aliasValidation = await validateAliasesAgainstArtifact(session, selected.kind, selected.artifact, selected.aliases);
  return {
    sessionId: session.sessionId,
    artifactId: selected.artifactId,
    kind: selected.kind,
    path: selected.path,
    sidecarPath: selected.sidecarPath,
    bytes: selected.bytes.length,
    ...artifactSummary(selected.kind, selected.artifact, selected.aliases),
    aliases: aliasValidation,
    validation: {
      ok: true,
      schema: selected.kind === "setup" ? "LabCheckpointScenarioV1" : "LabReplayArtifactV1",
      scope: "boundedLocalShape",
      authoritative: false,
      authoritativeValidationOnImport: true,
    },
  };
}

function artifactDirectory(workspaceRoot) {
  return path.join(workspaceRoot, "target", "lab-interact", "artifacts");
}

function readArtifact(workspaceRoot, { kind = null, artifactId = null, path: requestedPath = null }) {
  const directory = artifactDirectory(workspaceRoot);
  let artifactPath;
  if (artifactId) {
    if (!/^artifact_[a-f0-9]{32}$/.test(artifactId)) throw new LabInteractError("invalidArtifactId", "artifactId is invalid.");
    const candidates = kind
      ? [path.join(directory, `${artifactId}.${kind}.json`)]
      : ["setup", "replay"].map((candidateKind) => path.join(directory, `${artifactId}.${candidateKind}.json`));
    artifactPath = candidates.find((candidate) => fs.existsSync(candidate));
    if (!artifactPath) throw new LabInteractError("artifactNotFound", "Artifact id does not exist in this worktree.");
  } else {
    if (/^[A-Za-z][A-Za-z0-9+.-]*:/.test(requestedPath)) throw new LabInteractError("unsafeArtifactPath", "Artifact URLs are not accepted.");
    artifactPath = path.resolve(workspaceRoot, requestedPath);
  }
  const rootPrefix = `${fs.realpathSync(path.join(workspaceRoot, "target", "lab-interact"))}${path.sep}`;
  let realPath;
  try { realPath = fs.realpathSync(artifactPath); } catch { throw new LabInteractError("artifactNotFound", "Artifact path does not exist."); }
  if (!realPath.startsWith(rootPrefix)) throw new LabInteractError("unsafeArtifactPath", "Artifact path must stay beneath target/lab-interact/.");
  const match = path.basename(realPath).match(/^(artifact_[a-f0-9]{32})\.(setup|replay)\.json$/);
  if (!match) throw new LabInteractError("invalidArtifactPath", "Artifact filename is not a Lab Interact artifact.");
  const resolvedKind = match[2];
  if (kind && kind !== resolvedKind) throw new LabInteractError("artifactKindMismatch", "Requested artifact kind does not match the file.");
  const bytes = readBoundedFile(realPath, LAB_INTERACT_LIMITS.maxArtifactBytes, "artifactTooLarge", "Artifact exceeds 8 MiB.");
  let artifact;
  try { artifact = JSON.parse(bytes); } catch { throw new LabInteractError("invalidArtifact", "Artifact is not valid JSON."); }
  validateArtifactShape(resolvedKind, artifact);
  const sidecarPath = path.join(path.dirname(realPath), `${match[1]}.aliases.json`);
  const aliases = readAliasSidecar(sidecarPath, match[1], resolvedKind);
  return { artifactId: match[1], kind: resolvedKind, path: realPath, sidecarPath, bytes, artifact, aliases };
}

function validateArtifactShape(kind, artifact) {
  if (!artifact || typeof artifact !== "object" || Array.isArray(artifact)) throw new LabInteractError("invalidArtifact", "Artifact must be an object.");
  if (kind === "setup") {
    if (artifact.schemaVersion !== 1 || artifact.kind !== "labCheckpointScenario" || typeof artifact.checkpointPayload !== "string") {
      throw new LabInteractError("incompatibleArtifact", "Only LabCheckpointScenarioV1 setup artifacts are accepted.");
    }
  } else if (artifact.schema !== "rts.labReplay" || artifact.schemaVersion !== 1 || artifact.kind !== "labReplay" || !Array.isArray(artifact.operations)) {
    throw new LabInteractError("incompatibleArtifact", "Only LabReplayArtifactV1 replay artifacts are accepted.");
  }
}

function readAliasSidecar(sidecarPath, artifactId, kind) {
  if (!fs.existsSync(sidecarPath)) return [];
  let sidecar;
  try {
    const bytes = readBoundedFile(
      sidecarPath,
      LAB_INTERACT_LIMITS.maxAliasSidecarBytes,
      "invalidAliasSidecar",
      "Alias sidecar exceeds 64 KiB.",
    );
    sidecar = JSON.parse(bytes);
  } catch (error) {
    if (error instanceof LabInteractError) throw error;
    throw new LabInteractError("invalidAliasSidecar", "Alias sidecar is invalid JSON.");
  }
  if (sidecar?.schemaVersion !== 1 || sidecar.artifactId !== artifactId || sidecar.kind !== kind || !Array.isArray(sidecar.aliases) || sidecar.aliases.length > LAB_INTERACT_LIMITS.maxAliases) {
    throw new LabInteractError("invalidAliasSidecar", "Alias sidecar identity or bounds are invalid.");
  }
  const seen = new Set();
  return sidecar.aliases.map((entry) => {
    if (!entry || !ALIAS_RE.test(entry.alias) || !Number.isInteger(entry.id) || entry.id <= 0 || seen.has(entry.alias)) throw new LabInteractError("invalidAliasSidecar", "Alias sidecar contains an invalid or duplicate entry.");
    seen.add(entry.alias);
    return { alias: entry.alias, id: entry.id };
  });
}

function readBoundedFile(filePath, maxBytes, code, message) {
  let size;
  try { size = fs.statSync(filePath).size; } catch { throw new LabInteractError("artifactNotFound", "Artifact path does not exist."); }
  if (size > maxBytes) throw new LabInteractError(code, message);
  const bytes = fs.readFileSync(filePath);
  if (bytes.length > maxBytes) throw new LabInteractError(code, message);
  return bytes;
}

async function reconcileImportedAliases(session, aliases, entityIdMap) {
  const remaps = new Map(entityIdMap.map((entry) => [Number(entry.oldId), Number(entry.newId)]));
  session.aliases.clear();
  const candidates = aliases.map((entry) => ({ ...entry, id: remaps.get(entry.id) || entry.id }));
  const inspected = candidates.length ? await session.driver.inspect({ ids: candidates.map((entry) => entry.id), limit: candidates.length }) : { entities: [] };
  const existing = new Set((inspected.entities || []).map((entity) => entity.id));
  const restored = [];
  const stale = [];
  for (const entry of candidates) {
    if (existing.has(entry.id)) { session.aliases.set(entry.alias, entry.id); restored.push({ alias: entry.alias, oldId: aliases.find((source) => source.alias === entry.alias)?.id, id: entry.id }); }
    else stale.push({ alias: entry.alias, id: entry.id });
  }
  return { restored, stale };
}

async function validateImportedAliases(session, aliases) {
  session.aliases.clear();
  const inspected = aliases.length ? await session.driver.inspect({ ids: aliases.map((entry) => entry.id), limit: aliases.length }) : { entities: [] };
  const ids = new Set((inspected.entities || []).map((entity) => entity.id));
  const restored = [], stale = [];
  for (const entry of aliases) {
    if (ids.has(entry.id)) { session.aliases.set(entry.alias, entry.id); restored.push(entry); } else stale.push(entry);
  }
  return { restored, stale };
}

async function validateAliasesAgainstArtifact(_session, kind, artifact, aliases) {
  const setup = kind === "setup" ? artifact : artifact.initialSetup;
  const idMap = setup?.metadata?.sourceEntityIdMap || [];
  const known = new Set([...artifactEntityIds(setup), ...idMap.flatMap((entry) => [entry.oldId, entry.newId])]);
  return { entries: aliases, stale: aliases.filter((entry) => !known.has(entry.id)), status: "sidecarValidated" };
}

function artifactSummary(kind, artifact, aliases) {
  const setup = kind === "setup" ? artifact : artifact.initialSetup;
  let entityCount = null;
  try {
    entityCount = artifactEntityIds(setup).length;
  } catch {}
  return {
    authoring: kind === "setup" ? { name: artifact.name || "" } : artifact.authoring || {},
    map: setup?.map ? { name: setup.map.name, contentHash: setup.map.contentHash, materializedHash: setup.map.materializedHash } : null,
    tick: kind === "setup" ? setup?.metadata?.exportedTick ?? null : artifact.timeline?.initialTick ?? null,
    durationTicks: kind === "replay" ? artifact.timeline?.durationTicks ?? null : 0,
    entityCount,
    operationCount: kind === "replay" ? artifact.operations.length : 0,
    serverBuildSha: kind === "replay" ? artifact.serverBuildSha || null : null,
    aliasCount: aliases.length,
  };
}

function artifactEntityIds(setup) {
  try {
    const entities = JSON.parse(setup?.checkpointPayload || "")?.entities;
    const rows = Array.isArray(entities) ? entities : entities?.entities;
    return Array.isArray(rows) ? rows.map((entity) => entity?.id).filter((id) => Number.isInteger(id) && id > 0) : [];
  } catch { return []; }
}

function reproductionSummary(kind, artifactId, aliases) {
  const refs = aliases.slice(0, 12).map((entry) => entry.alias).join(", ");
  const input = JSON.stringify({ sessionId: "<current-session-id>", kind, artifactId });
  return `node scripts/lab-interact/cli.mjs import '${input}'; aliases: ${refs || "none"}`;
}

function artifactKind(value, label) { if (!["setup", "replay"].includes(value)) invalid(label, "must be setup or replay"); }
function artifactSelector(value, label) {
  const count = Number(value.artifactId != null) + Number(value.path != null);
  if (count !== 1) invalid(label, "must provide exactly one of artifactId or path");
  if (value.artifactId != null && (typeof value.artifactId !== "string" || !/^artifact_[a-f0-9]{32}$/.test(value.artifactId))) invalid(`${label}.artifactId`, "is invalid");
  if (value.path != null && (typeof value.path !== "string" || !value.path || value.path.length > 1024)) invalid(`${label}.path`, "must be a bounded path string");
}

function validateUpdate(value) {
  record(value, "update");
  const operation = value.operation;
  const allowed = {
    move: ["operation", "entity", "x", "y"], owner: ["operation", "entity", "owner"],
    resources: ["operation", "playerId", "steel", "oil"], research: ["operation", "playerId", "upgrade", "completed"],
    godMode: ["operation", "playerId", "enabled"],
  }[operation];
  if (!allowed) invalid("update.operation", "is unsupported");
  exact(value, allowed, "update");
  if (["move", "owner"].includes(operation)) entityRef(value.entity, "update.entity");
  if (operation === "move") { finite(value.x, "update.x"); finite(value.y, "update.y"); }
  if (operation === "owner") u32(value.owner, "update.owner");
  if (["resources", "research", "godMode"].includes(operation)) u32(value.playerId, "update.playerId");
  if (operation === "resources") { integer(value.steel, "update.steel", 0, U32_MAX); integer(value.oil, "update.oil", 0, U32_MAX); }
  if (operation === "research") { token(value.upgrade, "update.upgrade"); optionalBoolean(value.completed, "update.completed"); }
  if (operation === "godMode") optionalBoolean(value.enabled, "update.enabled");
}

function validateTime(value) {
  record(value, "time.control");
  const allowed = { pause: ["action"], resume: ["action", "speed"], speed: ["action", "speed"], step: ["action", "ticks"], seek: ["action", "tick"] }[value.action];
  if (!allowed) invalid("time.action", "is unsupported"); exact(value, allowed, "time.control");
  if (value.action === "resume" && value.speed != null) boundedNumber(value.speed, "time.speed", 0.01, 16);
  if (value.action === "speed") boundedNumber(value.speed, "time.speed", 0, 16);
  if (value.action === "step" && value.ticks != null) integer(value.ticks, "time.ticks", 1, 100);
  if (value.action === "seek") integer(value.tick, "time.tick", 0, 1_000_000);
}

function validateCamera(value) {
  record(value, "camera.camera");
  if (value.action === "focus") {
    exact(value, ["action", "refs", "padding"], "camera"); refs(value.refs, "camera.refs", 1, LAB_INTERACT_LIMITS.maxFocusRefs);
    if (value.padding != null) boundedNumber(value.padding, "camera.padding", 0, 1024);
  } else if (value.action === "set") {
    exact(value, ["action", "centerX", "centerY", "zoom"], "camera");
    if ((value.centerX == null) !== (value.centerY == null)) invalid("camera", "centerX and centerY must be provided together");
    if (value.centerX == null && value.zoom == null) invalid("camera", "requires zoom or a center");
    if (value.centerX != null) { finite(value.centerX, "camera.centerX"); finite(value.centerY, "camera.centerY"); }
    if (value.zoom != null) boundedNumber(value.zoom, "camera.zoom", Number.MIN_VALUE, 16);
  } else invalid("camera.action", "is unsupported");
}

function recordingCrop(value) {
  record(value, "record-start.crop");
  exact(value, ["x", "y", "width", "height"], "record-start.crop");
  boundedNumber(value.x, "record-start.crop.x", 0, 2048);
  boundedNumber(value.y, "record-start.crop.y", 0, 2048);
  boundedNumber(value.width, "record-start.crop.width", 2, 2048);
  boundedNumber(value.height, "record-start.crop.height", 2, 2048);
}

function recordingOperation(command, input, result) {
  return {
    command,
    acceptedAt: new Date().toISOString(),
    input: JSON.parse(JSON.stringify(input, (key, value) => key === "sessionId" ? undefined : value)),
    authoritativeTick: findSnapshotTick(result),
  };
}

function findSnapshotTick(value, depth = 0) {
  if (!value || typeof value !== "object" || depth > 5) return null;
  if (Number.isInteger(value.snapshotTick)) return value.snapshotTick;
  for (const child of Object.values(value)) {
    const tick = findSnapshotTick(child, depth + 1);
    if (tick != null) return tick;
  }
  return null;
}

function validateCommand(value) {
  record(value, "order.command");
  const allowed = COMMAND_FIELDS[value.c]; if (!allowed) invalid("order.command.c", "is unsupported"); exact(value, allowed, "order.command");
  if (allowed.includes("units")) refs(value.units, "order.command.units", 1, LAB_INTERACT_LIMITS.maxCommandUnits);
  for (const field of ["x", "y"]) {
    if (allowed.includes(field) && (value.c !== "useAbility" || value[field] != null)) finite(value[field], `order.command.${field}`);
  }
  for (const field of ["target", "node", "building"]) if (allowed.includes(field) && !(field === "building" && value.c === "build")) entityRef(value[field], `order.command.${field}`);
  for (const field of ["ability", "unit", "upgrade"]) if (allowed.includes(field)) token(value[field], `order.command.${field}`);
  if (value.c === "build") { token(value.building, "order.command.building"); integer(value.tileX, "order.command.tileX", 0, U32_MAX); integer(value.tileY, "order.command.tileY", 0, U32_MAX); }
  if (value.targetObjectId != null) u32(value.targetObjectId, "order.command.targetObjectId");
  if (value.kind != null && !["move", "attackMove", "attack", "gather", "build"].includes(value.kind)) invalid("order.command.kind", "is unsupported");
  optionalBoolean(value.queued, "order.command.queued");
  if (allowed.includes("enabled")) optionalBoolean(value.enabled, "order.command.enabled", false);
}

function projectCatalog(catalog, requested) {
  const categories = [...new Set(requested?.length ? requested : ALL_CATALOG_CATEGORIES)];
  const all = {
    maps: Array.isArray(catalog.maps) ? catalog.maps : [], players: Array.isArray(catalog.players) ? catalog.players : [],
    factions: Array.isArray(catalog.factions) ? catalog.factions.map((faction) => ({
      id: String(faction?.id || ""), label: String(faction?.label || ""), units: uniqueStrings(faction?.units),
      buildings: uniqueStrings(faction?.buildings), upgrades: uniqueStrings(faction?.upgrades),
    })) : [],
    units: uniqueStrings(flattenFactions(catalog.factions, "units")), buildings: uniqueStrings(flattenFactions(catalog.factions, "buildings")),
    upgrades: uniqueStrings(flattenFactions(catalog.factions, "upgrades")), commands: uniqueStrings(catalog.supportedCommandKinds), abilities: uniqueStrings(catalog.abilities),
  };
  return { categories: Object.fromEntries(categories.map((category) => [category, all[category]])), truncated: false };
}

function flattenFactions(factions, field) { return (Array.isArray(factions) ? factions : []).flatMap((faction) => Array.isArray(faction?.[field]) ? faction[field] : []); }
function uniqueStrings(values) { return [...new Set((Array.isArray(values) ? values : []).filter((value) => typeof value === "string"))].sort(); }

function validateSpawnAliases(session, spawns) {
  const aliases = new Set();
  for (const { alias: value } of spawns) {
    if (!value) continue;
    if (session.aliases.has(value) || aliases.has(value)) throw new LabInteractError("duplicateAlias", `Alias ${JSON.stringify(value)} is already in use.`);
    aliases.add(value);
  }
  if (session.aliases.size + aliases.size > LAB_INTERACT_LIMITS.maxAliases) throw new LabInteractError("aliasLimit", `At most ${LAB_INTERACT_LIMITS.maxAliases} aliases are allowed.`);
}

function assertKnownPlayer(catalog, playerId) {
  if (playerId == null) return;
  if (!(catalog.players || []).some((player) => Number(player?.id) === Number(playerId))) throw new LabInteractError("unknownPlayer", `Player ${playerId} is not available.`);
}

function validateCommandCatalog(command, catalog) {
  if (!new Set(uniqueStrings(catalog.supportedCommandKinds)).has(command.c)) throw new LabInteractError("unsupportedCommand", `${command.c} is not supported.`);
  const validateFrom = (field, values, code, label) => {
    if (command[field] != null && !new Set(values).has(command[field])) throw new LabInteractError(code, `${command[field]} is not an available ${label}.`);
  };
  if (command.c === "build") validateFrom("building", flattenFactions(catalog.factions, "buildings"), "invalidKind", "building");
  validateFrom("unit", flattenFactions(catalog.factions, "units"), "invalidKind", "unit");
  validateFrom("upgrade", flattenFactions(catalog.factions, "upgrades"), "invalidUpgrade", "upgrade");
  validateFrom("ability", catalog.abilities, "invalidAbility", "ability");
}

async function resolveCommand(session, command) {
  const wire = { ...command }; const resolved = {};
  if (Array.isArray(command.units)) { const units = await resolveEntityReferences(session, command.units); wire.units = units.map((entry) => entry.id); resolved.units = units; }
  for (const field of ["target", "node", "building"]) {
    if (command[field] == null || (field === "building" && command.c === "build")) continue;
    const entry = await resolveEntityReference(session, command[field]); wire[field] = entry.id; resolved[field] = entry;
  }
  return { command: wire, resolved };
}

async function resolveEntityReferences(session, references) {
  const entries = []; const requestedIds = [];
  for (const reference of references) {
    if (typeof reference === "number") { entries.push({ input: reference, id: reference, alias: null }); requestedIds.push(reference); continue; }
    const id = session.aliases.get(reference);
    if (!id) throw new LabInteractError("unknownAlias", `Unknown alias ${JSON.stringify(reference)}.`);
    entries.push({ input: reference, id, alias: reference }); requestedIds.push(id);
  }
  if (new Set(requestedIds).size !== requestedIds.length) throw new LabInteractError("duplicateReference", "A command may not resolve the same entity twice.");
  const existing = await session.driver.inspect({ ids: [...new Set(requestedIds)], limit: requestedIds.length });
  const found = new Set((existing.entities || []).map((entity) => entity.id));
  for (const entry of entries) {
    if (found.has(entry.id)) continue;
    if (entry.alias) session.aliases.delete(entry.alias);
    throw new LabInteractError(entry.alias ? "staleAlias" : "unknownEntity", entry.alias ? `Alias ${JSON.stringify(entry.alias)} is stale and was cleared.` : `Entity ${entry.id} is unavailable.`);
  }
  return entries;
}

async function resolveEntityReference(session, reference) { return (await resolveEntityReferences(session, [reference]))[0]; }
async function aliasSnapshots(session) {
  if (!session.aliases.size) return [];
  const entries = [...session.aliases].map(([alias, id]) => ({ alias, id }));
  const inspected = await session.driver.inspect({ ids: entries.map((entry) => entry.id), limit: entries.length });
  const byId = new Map((inspected.entities || []).map((entity) => [entity.id, entity]));
  return entries.map((entry) => ({ ...entry, entity: byId.get(entry.id) || null }));
}
async function reconcileAliasesAfterReset(session, before) {
  session.aliases.clear(); const after = await session.driver.inspect({ limit: LAB_INTERACT_LIMITS.maxInspectResults });
  const claimed = new Set(); const aliases = []; const clearedAliases = [];
  for (const entry of before) {
    const matches = (after.entities || []).filter((entity) => exactAliasMatch(entry.entity, entity) && !claimed.has(entity.id));
    if (matches.length === 1) { const id = matches[0].id; claimed.add(id); session.aliases.set(entry.alias, id); aliases.push({ alias: entry.alias, id }); }
    else clearedAliases.push(entry.alias);
  }
  return { aliases, clearedAliases };
}
function exactAliasMatch(before, after) { return !!before && !!after && before.kind === after.kind && before.owner === after.owner && Number(before.x) === Number(after.x) && Number(before.y) === Number(after.y); }
function decorateEntity(entity, aliases) { return entity && typeof entity === "object" ? { ...entity, alias: aliasForEntity(aliases, entity.id) } : entity || null; }
function aliasForEntity(aliases, id) { for (const [alias, entityId] of aliases) if (entityId === id) return alias; return null; }
function clearAliasesForEntity(aliases, id) { for (const [alias, entityId] of aliases) if (entityId === id) aliases.delete(alias); }

function exact(value, allowed, label) { const extras = Object.keys(value).filter((key) => !allowed.includes(key)); if (extras.length) invalid(label, `contains unexpected field ${JSON.stringify(extras[0])}`); return value; }
function record(value, label) { if (!value || typeof value !== "object" || Array.isArray(value)) invalid(label, "must be a JSON object"); }
function array(value, label, minimum, maximum, validate) { if (!Array.isArray(value) || value.length < minimum || value.length > maximum) invalid(label, `must contain ${minimum}-${maximum} items`); value.forEach(validate); }
function refs(value, label, minimum, maximum) { array(value, label, minimum, maximum, (entry) => entityRef(entry, label)); }
function entityRef(value, label) { if (typeof value === "string") alias(value); else u32(value, label); }
function alias(value) { if (typeof value !== "string" || !ALIAS_RE.test(value)) invalid("alias", "must start with a letter and contain only letters, digits, _ or -"); }
function sessionId(value) { if (typeof value !== "string" || !SESSION_RE.test(value)) invalid("sessionId", "must be a Lab Interact session id"); }
function token(value, label, maximum = 64) { if (typeof value !== "string" || !TOKEN_RE.test(value) || value.length > maximum) invalid(label, "must be a safe protocol token"); }
function finite(value, label) { if (!Number.isFinite(value)) invalid(label, "must be a finite number"); }
function boundedNumber(value, label, minimum, maximum) { finite(value, label); if (value < minimum || value > maximum) invalid(label, `must be from ${minimum} to ${maximum}`); }
function isInteger(value, minimum, maximum) { return Number.isInteger(value) && value >= minimum && value <= maximum; }
function integer(value, label, minimum, maximum) { if (!isInteger(value, minimum, maximum)) invalid(label, `must be an integer from ${minimum} to ${maximum}`); return value; }
function u32(value, label) { return integer(value, label, 1, U32_MAX); }
function optionalBoolean(value, label, optional = true) { if (value == null && optional) return; if (typeof value !== "boolean") invalid(label, "must be a boolean"); }
function viewport(value, maximum, label) { record(value, label); exact(value, ["width", "height", "deviceScaleFactor"], label); integer(value.width, `${label}.width`, 320, maximum); integer(value.height, `${label}.height`, 240, maximum); if (value.deviceScaleFactor != null) boundedNumber(value.deviceScaleFactor, `${label}.deviceScaleFactor`, Number.MIN_VALUE, 4); }
function invalid(label, message) { throw new LabInteractError("invalidInput", `${label} ${message}.`); }
function realWorkspaceRoot(value) { try { return fs.realpathSync(value); } catch { throw new LabInteractError("invalidWorkspace", `Workspace does not exist: ${String(value)}`); } }
function resolveRequestedWorkspace(requested, allowed) { const candidate = realWorkspaceRoot(requested || allowed); if (candidate !== allowed) throw new LabInteractError("workspaceNotAllowed", "open may use only the worktree that launched this daemon."); return candidate; }

export function normalizeError(error) {
  if (error instanceof LabInteractError) return error;
  if (error instanceof LabInteractDriverError) return new LabInteractError(error.code || "driverError", conciseError(error));
  return new LabInteractError(error?.code || "commandFailed", conciseError(error));
}
export function conciseError(error) { return String(error?.message || "Lab Interact command failed.").split("\nServer log tail:")[0].slice(0, 1000); }

export async function loadDriverFactory(workspaceRoot = process.cwd()) {
  const modulePath = process.env.RTS_LAB_INTERACT_DRIVER_FACTORY_MODULE;
  if (!modulePath) return undefined;
  const module = await import(pathToFileURL(path.resolve(workspaceRoot, modulePath)).href);
  if (typeof module.openLabInteractDriver !== "function") throw new LabInteractError("invalidDriverFactory", "Driver factory module must export openLabInteractDriver(options).");
  return module.openLabInteractDriver;
}
