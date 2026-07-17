// Transport-neutral, bounded command service for Interact.
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { InteractDriver, InteractDriverError } from "./driver.ts";
import {
  ALIAS_RE, ALL_CATALOG_CATEGORIES, INTERACT_LIMITS,
} from "./command_inputs.ts";
import { commandDefinition } from "./command_registry.ts";
import { SessionCoordinator } from "./session_coordinator.ts";
import { gameSessionCapabilities, scenarioSessionCapabilities } from "./game_session.ts";
import { executeObservationCommand } from "./observation_session.ts";
import { defaultMapForMode } from "./session_defaults.ts";
import type { InteractTailnetPreview } from "./tailnet_preview.ts";
export { INTERACT_LIMITS } from "./command_inputs.ts";
export { INTERACT_COMMANDS } from "./command_registry.ts";
type JsonObject = Record<string, unknown>;
type EntityRef = string | number;
interface AliasEntry { alias: string; id: number }
interface ResolvedEntityRef { input: EntityRef; id: number; alias: string | null }
interface SpawnInput extends JsonObject {
  owner: number;
  kind: string;
  x: number;
  y: number;
  alias?: string;
}
interface ViewportInput { width: number; height: number; deviceScaleFactor?: number }
interface ServiceInput extends JsonObject {
  sessionId?: string;
  workspaceRoot?: string;
  map?: string;
  seed?: string | number;
  scenario?: string;
  id?: string;
  unit?: string;
  count?: number;
  blocker?: string;
  case?: string;
  renderer?: string;
  viewport?: ViewportInput;
  categories?: string[];
  details?: boolean;
  spawns?: SpawnInput[];
  update?: JsonObject; updates?: JsonObject[]; refs?: EntityRef[]; ids?: number[];
  playerId?: number; command?: JsonObject; ignoreCommandLimits?: boolean;
  control?: JsonObject;
  kinds?: string[]; owners?: number[]; cameraViewport?: boolean; limit?: number;
  camera?: JsonObject;
  name?: string;
  presentation?: "clean" | "normal";
  subjects?: EntityRef[];
  kind?: string;
  artifactId?: string;
  path?: string;
  maxDurationMs?: number;
  crop?: { x: number; y: number; width: number; height: number };
  region?: "viewport" | "minimap" | { x: number; y: number; width: number; height: number };
  scale?: number;
  resumeSpeed?: number | null;
  fps?: number;
  frameCount?: number;
  reproduction?: boolean;
  opponent?: string;
  spectate?: string[]; sampleEveryMs?: number; speed?: number;
  ownership?: string;
  units?: number[];
  x?: number;
  y?: number;
  queued?: boolean;
}
interface InteractSession {
  sessionId: string;
  kind: "lab" | "game" | "scenario";
  driver: InteractDriver;
  aliases: Map<string, number>;
  sceneRevision: number;
  sceneIdentity: JsonObject;
}
type DriverFactory = (options: ConstructorParameters<typeof InteractDriver>[0]) => Promise<InteractDriver>;
export class InteractError extends Error {
  details: JsonObject;
  code: string;
  constructor(code: string, message: string, details: JsonObject = {}) {
    super(message);
    this.name = "InteractError";
    this.code = code;
    this.details = details;
  }
}
export class InteractService {
  closed: boolean;
  closePromise: Promise<boolean> | null;
  openAbortController: AbortController | null;
  openPromise: Promise<unknown> | null;
  openingKind: "lab" | "game" | "scenario" | null;
  coordinator: SessionCoordinator;
  sessions: Map<string, InteractSession>;
  log: (...values: unknown[]) => void;
  maxSessions: number;
  artifactPreview: InteractTailnetPreview | null;
  driverFactory: DriverFactory;
  workspaceRoot: string;
  constructor({
    workspaceRoot = process.cwd(),
    driverFactory = (options: ConstructorParameters<typeof InteractDriver>[0]) => InteractDriver.open(options),
    artifactPreview = null,
    log = () => {},
  }: {
    workspaceRoot?: string;
    driverFactory?: DriverFactory;
    artifactPreview?: InteractTailnetPreview | null;
    log?: (...values: unknown[]) => void;
  } = {}) {
    this.workspaceRoot = realWorkspaceRoot(workspaceRoot);
    this.driverFactory = driverFactory;
    this.artifactPreview = artifactPreview;
    this.maxSessions = INTERACT_LIMITS.maxSessions;
    this.log = log;
    this.sessions = new Map();
    this.coordinator = new SessionCoordinator();
    this.openPromise = null;
    this.openingKind = null;
    this.openAbortController = null;
    this.closePromise = null;
    this.closed = false;
  }
  async execute(command: string, rawInput: unknown = {}) {
    const definition = commandDefinition(command);
    if (!definition) {
      throw new InteractError("unknownCommand", `Unknown command ${JSON.stringify(command)}.`);
    }
    // The registry validator has already performed exact, bounded runtime validation.
    const input = definition.validator(rawInput) as ServiceInput;
    const session = definition.scope === "session" && command !== "close"
      ? this.get(input.sessionId!)
      : null;
    return this.coordinator.execute(definition, input.sessionId, async () => {
      let result;
      if (definition.handlerKey === "shutdown") {
        await this.shutdown("explicit");
        result = { shuttingDown: true };
      } else if (definition.handlerKey === "status") {
        result = await this.status(input);
      } else if (["open", "game-open", "scenario-open"].includes(definition.handlerKey)) {
        const kind = definition.handlerKey === "game-open"
          ? "game"
          : definition.handlerKey === "scenario-open" ? "scenario" : "lab";
        result = await this.open(input, kind);
      } else if (definition.handlerKey === "close") {
        result = { sessionId: input.sessionId, closed: await this.close(input.sessionId!) };
      } else if (definition.handlerKey === "capture-cancel") {
        result = { sessionId: input.sessionId, ...session!.driver.cancelFixedCapture() };
      } else if (definition.handlerKey === "record-wait") {
        const recording = await session!.driver.recordWait();
        result = await presentRecordingResult({ sessionId: input.sessionId, ...recording }, this.artifactPreview);
      } else {
        result = await this.executeSession(definition.handlerKey, session!, input);
      }
      if (session && definition.sceneMutation) session.sceneRevision += 1;
      if (session && definition.recordable) {
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
  async open(input: ServiceInput, kind: "lab" | "game" | "scenario" = "lab") {
    if (this.closed) throw new InteractError("serviceClosed", "Interact is shutting down.");
    const workspaceRoot = resolveRequestedWorkspace(input.workspaceRoot, this.workspaceRoot);
    let existing = this.sessions.values().next().value;
    if (existing) return this.describeExistingSession(existing, kind);
    await this.closePromise;
    existing = this.sessions.values().next().value;
    if (existing) return this.describeExistingSession(existing, kind);
    if (this.openPromise) {
      if (this.openingKind !== kind) throw new InteractError("sessionKindMismatch", `A ${this.openingKind} session is already opening.`);
      return this.openPromise;
    }
    const openAbortController = new AbortController();
    this.openAbortController = openAbortController;
    this.openingKind = kind;
    const map = input.map || defaultMapForMode(kind);
    this.openPromise = (async () => {
      const driver = await this.driverFactory({
        workspaceRoot,
        mode: kind,
        map,
        seed: input.seed == null ? "" : String(input.seed),
        scenario: input.scenario || "blank",
        devScenario: { id: input.id || "", unit: input.unit || "", count: input.count || 1,
          blocker: input.blocker || "", case: input.case || "" },
        opponent: input.opponent || "ai_2_1",
        spectate: input.spectate || null,
        renderer: input.renderer || "pixi",
        viewport: input.viewport,
        baseUrl: process.env.RTS_INTERACT_BASE_URL || process.env.RTS_INTERACT_LAB_BASE_URL || "",
        signal: openAbortController.signal,
      });
      if (this.closed) {
        await driver.close().catch(() => {});
        throw new InteractError("serviceClosed", "Interact shut down while the session was opening.");
      }
      const sessionId = `${kind}_${crypto.randomUUID().replaceAll("-", "")}`;
      const session: InteractSession = {
        sessionId, kind, driver, aliases: new Map<string, number>(), sceneRevision: 0,
        sceneIdentity: {
          source: "launch", kind, scenario: kind === "lab" ? input.scenario || "blank" : null,
          map, seed: kind === "lab" ? input.seed ?? null : null,
          opponent: kind === "game" && !input.spectate ? input.opponent || "ai_2_1" : null,
          spectate: kind === "game" ? input.spectate || null : null,
          role: kind === "game" ? (input.spectate ? "spectator" : "player") : kind === "scenario" ? "observer" : null,
          devScenario: kind === "scenario"
            ? { id: input.id, unit: input.unit, count: input.count, blocker: input.blocker || null, case: input.case || null }
            : null,
          renderer: input.renderer || "pixi",
        },
      };
      this.sessions.set(sessionId, session);
      this.coordinator.register(sessionId);
      try {
        return await this.describeSession(session);
      } catch (error) {
        await this.close(sessionId, "openVerificationFailed");
        throw error;
      }
    })();
    try { return await this.openPromise; } finally {
      this.openPromise = null;
      this.openingKind = null;
      if (this.openAbortController === openAbortController) this.openAbortController = null;
    }
  }
  async describeExistingSession(session: InteractSession, requestedKind: "lab" | "game" | "scenario") {
    if (session.kind !== requestedKind) {
      throw new InteractError(
        "sessionKindMismatch",
        `A ${session.kind} session is already active. Close it before opening a ${requestedKind} session.`,
        { sessionId: session.sessionId, activeKind: session.kind, requestedKind },
      );
    }
    return this.describeSession(session);
  }
  async describeSession(session: InteractSession) {
    const status = await session.driver.status();
    const catalog = session.kind === "lab" ? await session.driver.catalog() : null;
    return {
      sessionId: session.sessionId,
      kind: session.kind,
      workspace: session.driver.workspace,
      tick: Number.isInteger(status.snapshotTick) ? status.snapshotTick : null,
      players: Array.isArray(catalog?.players) ? catalog.players : [],
      status,
      capabilities: session.kind === "lab"
        ? { aliases: true, selection: true, catalogCategories: [...ALL_CATALOG_CATEGORIES], maxSessions: this.maxSessions }
        : session.kind === "scenario"
          ? scenarioSessionCapabilities(this.maxSessions)
          : gameSessionCapabilities(session.sceneIdentity.role, this.maxSessions),
    };
  }
  get(sessionId: string | null | undefined) {
    if (!sessionId) throw new InteractError("unknownSession", "Unknown or closed sessionId. Run open first.");
    const session = this.sessions.get(sessionId);
    if (!session) throw new InteractError("unknownSession", "Unknown or closed sessionId. Run open first.");
    return session;
  }
  async close(sessionId: string, reason = "explicit") {
    const session = this.sessions.get(sessionId);
    if (!session) return false;
    this.sessions.delete(sessionId);
    if (session.driver.fixedCaptureStatus?.().active) session.driver.cancelFixedCapture?.();
    const closing = (async () => {
      const recorderSettlement = session.driver.settleRecording?.("sessionClose", {
        aliases: [...session.aliases].map(([alias, id]) => ({ alias, id })),
      });
      await recorderSettlement?.catch((error: unknown) => this.log("recordingSettlementFailed", {
        sessionId, reason, error: conciseError(error),
      }));
      await this.coordinator.drain(sessionId);
      await session.driver.close().catch((error: unknown) => this.log("sessionCloseFailed", {
        sessionId, reason, error: conciseError(error),
      }));
      this.coordinator.release(sessionId);
      return true;
    })();
    this.closePromise = closing;
    try { return await closing; } finally { if (this.closePromise === closing) this.closePromise = null; }
  }

  async shutdown(reason = "shutdown") {
    if (this.closed) return;
    this.closed = true;
    this.openAbortController?.abort();
    await this.openPromise?.catch(() => {});
    await this.closePromise?.catch(() => {});
    await Promise.all([...this.sessions.keys()].map((sessionId) => this.close(sessionId, reason)));
  }

  canRefreshCheckout() {
    return !this.closed && this.openPromise == null && this.closePromise == null && this.sessions.size === 0;
  }

  async status({ sessionId }: Pick<ServiceInput, "sessionId"> = {}) {
    if (sessionId) {
      const session = this.get(sessionId);
      const fixedCapture = session.driver.fixedCaptureStatus?.() || { active: false };
      if (fixedCapture.active) {
        return {
          sessionId,
          kind: session.kind,
          status: session.driver.fixedCapture?.startStatus || { ready: true },
          aliases: [...session.aliases].map(([alias, id]) => ({ alias, id })),
          recorder: presentRecorderStatus(session.driver.recordingStatus?.() || { active: false }, this.artifactPreview),
          fixedCapture,
        };
      }
      return {
        sessionId,
        kind: session.kind,
        status: await session.driver.status(),
        aliases: [...session.aliases].map(([alias, id]) => ({ alias, id })),
        recorder: presentRecorderStatus(session.driver.recordingStatus?.() || { active: false }, this.artifactPreview),
        fixedCapture,
      };
    }
    return {
      workspaceRoot: this.workspaceRoot,
      opening: this.openPromise != null,
      openingKind: this.openingKind,
      closing: this.closePromise != null,
      sessions: [...this.sessions.values()].map((session) => ({
        sessionId: session.sessionId,
        kind: session.kind,
        aliases: session.aliases.size,
      })),
      maxSessions: this.maxSessions,
    };
  }
  async executeSession(command: string, session: InteractSession, input: ServiceInput) {
    const sessionId = session.sessionId;
    if (command.startsWith("game-") && session.kind !== "game") {
      throw new InteractError("sessionKindMismatch", "This command requires a game session.");
    }
    if (command.startsWith("scenario-") && session.kind !== "scenario") {
      throw new InteractError("sessionKindMismatch", "This command requires a dev scenario session.");
    }
    const observation = await executeObservationCommand(command, session, input, this.artifactPreview);
    if (observation.handled) {
      if (observation.kind === "recording") {
        return { ...observation.result, recorder: presentRecorderStatus(observation.result.recorder, this.artifactPreview) };
      }
      if (observation.kind === "capture") {
        return presentFixedCaptureResult({ sessionId, ...observation.result }, this.artifactPreview);
      }
      return observation.result;
    }
    if (command === "game-move") return { sessionId, result: await session.driver.move({
      units: input.units || [], x: input.x, y: input.y, queued: input.queued === true,
    }) };
    if (command === "game-give-up") return { sessionId, result: await session.driver.giveUp() };
    if (session.kind !== "lab" && !["record-stop"].includes(command)) {
      throw new InteractError("sessionKindMismatch", "This command requires a Lab session.");
    }
    if (command === "reset") {
      const before = await aliasSnapshots(session);
      const result = await session.driver.reset();
      return { sessionId, result, ...await reconcileAliasesAfterReset(session, before) };
    }
    if (command === "catalog") {
      return { sessionId, ...projectCatalog(await session.driver.catalog(), input.categories) };
    }
    if (command === "spawn") return spawn(session, input.spawns, input.details === true);
    if (command === "update") return update(session, input.updates || (input.update ? [input.update] : []));
    if (command === "remove") {
      const resolved = await resolveEntityReferences(session, input.refs || []);
      const result = await session.driver.remove(resolved.map((entry) => entry.id));
      const removed = resolved.map((entry) => ({ id: entry.id, alias: aliasForEntity(session.aliases, entry.id) }));
      for (const entry of resolved) clearAliasesForEntity(session.aliases, entry.id);
      return { sessionId, removed, result };
    }
    if (command === "order") return order(session, input);
    if (command === "time") return { sessionId, result: await session.driver.time(input.control || {}) };
    if (command === "inspect") return inspect(session, input);
    if (command === "select") return select(session, input);
    if (command === "camera") return camera(session, input.camera || {});
    if (command === "screenshot") return screenshot(session, input, this.artifactPreview);
    if (command === "export") return exportArtifact(this.workspaceRoot, session, input);
    if (command === "import") return importArtifact(this.workspaceRoot, session, input);
    if (command === "artifact-inspect") return inspectArtifact(this.workspaceRoot, session, input);
    if (command === "record-start") {
      const result = await session.driver.recordStart({ ...input, sessionId });
      return { sessionId, recorder: presentRecorderStatus(result, this.artifactPreview) };
    }
    if (command === "record-stop") {
      const result = await session.driver.recordStop({
        aliases: [...session.aliases].map(([alias, id]) => ({ alias, id })),
      });
      return presentRecordingResult({ sessionId, ...result }, this.artifactPreview);
    }
    if (command === "capture-fixed") {
      const result = await session.driver.captureFixed({
        ...input, sessionId, sceneIdentity: session.sceneIdentity, sceneRevision: session.sceneRevision,
        aliases: [...session.aliases].map(([alias, id]) => ({ alias, id })),
      });
      return presentFixedCaptureResult({ sessionId, ...result }, this.artifactPreview);
    }
    throw new InteractError("unknownCommand", `Unknown session command ${command}.`);
  }
}

export function validateCommandInput(command: string, input: unknown) {
  const definition = commandDefinition(command);
  if (!definition) {
    throw new InteractError("unknownCommand", `Unknown command ${JSON.stringify(command)}.`);
  }
  return definition.validator(input);
}

async function spawn(session: InteractSession, spawns: SpawnInput[] = [], includeDetails = false) {
  validateSpawnAliases(session, spawns);
  const catalog = await session.driver.catalog();
  const playerIds = new Set(objectArray(catalog.players).map((player) => Number(player.id)));
  const spawnableKinds = new Set(flattenFactions(catalog.factions, "units").concat(flattenFactions(catalog.factions, "buildings")));
  for (const spec of spawns) {
    if (!playerIds.has(spec.owner)) throw new InteractError("unknownPlayer", `Player ${spec.owner} is not available.`);
    if (!spawnableKinds.has(spec.kind)) throw new InteractError("invalidKind", `${spec.kind} is not spawnable.`);
  }
  const response = await session.driver.spawn(spawns.map(({ alias: _alias, ...spec }) => spec));
  const entities = objectArray(response.entities);
  const result = asJsonObject(response.result);
  const outcome = asJsonObject(result.outcome);
  const outcomes = objectArray(outcome.items);
  const staged: Array<{ spec: SpawnInput; id: number; entity: JsonObject | null }> = [];
  for (let index = 0; index < spawns.length; index += 1) {
    const spec = spawns[index];
    const entity = entities[index] || null;
    const candidateId = entity?.id ?? asJsonObject(outcomes[index]?.outcome).entityId;
    if (!Number.isInteger(candidateId) || Number(candidateId) <= 0) throw new InteractError("missingEntityId", "Spawn did not return an entity id.");
    staged.push({ spec, id: Number(candidateId), entity });
  }
  for (const { spec, id } of staged) if (spec.alias) session.aliases.set(spec.alias, id);
  const summary = boundedResponseSummary(staged.map(({ spec, id }, index) => ({
    index,
    alias: spec.alias || null,
    id,
  })));
  const compact = {
    sessionId: session.sessionId,
    spawned: summary,
    snapshotTick: findSnapshotTick(response),
  };
  if (!includeDetails) return compact;
  const results = staged.map(({ spec, id, entity }) => ({
    alias: spec.alias || null,
    id,
    entity: decorateEntity(entity, session.aliases),
  }));
  return { ...compact, results, result };
}

async function update(session: InteractSession, values: JsonObject[] = []) {
  const catalog = await session.driver.catalog();
  const entityValues = values.filter((value) => ["move", "owner"].includes(String(value.operation)));
  const resolvedEntities = entityValues.length
    ? await resolveEntityReferences(session, entityValues.map((value) => value.entity as EntityRef))
    : [];
  let resolvedIndex = 0;
  const operations: JsonObject[] = [];
  for (const value of values) {
    assertKnownPlayer(catalog, value.playerId ?? value.owner);
    let operation = value;
    if (value.operation === "move") {
      const entity = resolvedEntities[resolvedIndex++];
      operation = { operation: "move", entityId: entity.id, x: value.x, y: value.y };
    } else if (value.operation === "owner") {
      const entity = resolvedEntities[resolvedIndex++];
      operation = { operation: "reassign", entityId: entity.id, owner: value.owner };
    } else if (value.operation === "research" && !flattenFactions(catalog.factions, "upgrades").includes(String(value.upgrade))) {
      throw new InteractError("invalidUpgrade", `${value.upgrade} is not an available lab upgrade.`);
    }
    operations.push(operation);
  }
  return { sessionId: session.sessionId, result: await session.driver.update(operations) };
}

async function order(session: InteractSession, { playerId, command, ignoreCommandLimits = false }: ServiceInput) {
  if (typeof playerId !== "number" || !command) throw new InteractError("invalidInput", "order requires playerId and command.");
  const catalog = await session.driver.catalog();
  assertKnownPlayer(catalog, playerId);
  validateCommandCatalog(command, catalog);
  const { command: resolvedCommand, resolved } = await resolveCommand(session, command);
  const result = await session.driver.order({ playerId, command: resolvedCommand, ignoreCommandLimits });
  return { sessionId: session.sessionId, command: resolvedCommand, resolved, result };
}

async function inspect(session: InteractSession, { refs, kinds, owners, cameraViewport, limit }: ServiceInput) {
  const resolved = refs ? await resolveEntityReferences(session, refs) : [];
  const response = await session.driver.inspect({
    ids: resolved.map((entry) => entry.id), kinds, owners,
    cameraViewport: cameraViewport === true, limit: limit || 25,
  });
  return {
    sessionId: session.sessionId,
    entities: objectArray(response.entities).map((entity) => decorateEntity(entity, session.aliases)),
    players: response.players || [], room: response.room || null, camera: response.camera || null,
    cameraViewport: response.cameraViewport || null,
    cameraWorldBounds: response.cameraWorldBounds || null, selection: Array.isArray(response.selection) ? response.selection : [],
    truncated: response.truncated === true,
    totalMatching: Number.isInteger(response.totalMatching) ? response.totalMatching : 0,
  };
}

async function select(session: InteractSession, { refs = [] }: ServiceInput) {
  const resolved = await resolveEntityReferences(session, refs); const response = await session.driver.select(resolved.map((entry) => entry.id));
  return { sessionId: session.sessionId, selection: Array.isArray(response.selection) ? response.selection : [],
    entities: objectArray(response.entities).map((entity) => decorateEntity(entity, session.aliases)),
  };
}

async function camera(session: InteractSession, value: JsonObject = {}) {
  let command: JsonObject = value;
  if (value.action === "focus") {
    const resolved = await resolveEntityReferences(session, value.refs as EntityRef[]);
    command = { action: "focus", entityIds: resolved.map((entry) => entry.id), padding: value.padding };
  }
  const response = await session.driver.camera(command);
  return {
    sessionId: session.sessionId,
    camera: response.camera || response,
    cameraViewport: response.cameraViewport || null,
    cameraWorldBounds: response.cameraWorldBounds || null,
  };
}

async function screenshot(session: InteractSession, { name = "scene", presentation = "clean", viewport, subjects }: ServiceInput, artifactPreview: InteractTailnetPreview | null) {
  const resolved = subjects ? await resolveEntityReferences(session, subjects) : [];
  const inspected = resolved.length
    ? await session.driver.inspect({ ids: resolved.map((entry) => entry.id), limit: resolved.length })
    : { entities: [] };
  const entitiesById = new Map(objectArray(inspected.entities).map((entity) => [Number(entity.id), entity]));
  const subjectSummaries = resolved.map((entry) => decorateEntity(entitiesById.get(entry.id), session.aliases));
  const capture = await session.driver.screenshot({
    sessionId: session.sessionId, name, presentation, viewport,
    subjectIds: resolved.map((entry) => entry.id), subjectSummaries,
    request: { command: "screenshot", sessionId: session.sessionId, name, presentation, viewport, subjects: resolved },
  });
  const result = {
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
  return presentScreenshotResult(result, artifactPreview);
}

const TAILNET_DELIVERY_INSTRUCTION = "Share this Tailnet URL with the user to preview the Interact artifact. Do not share a local file path.";

async function presentScreenshotResult(result: JsonObject & { pngPath: string; manifestPath: string }, artifactPreview: InteractTailnetPreview | null) {
  if (!artifactPreview) return result;
  const { pngPath, manifestPath, ...visible } = result;
  return {
    ...visible,
    preview: await publishPreview(artifactPreview, pngPath, "image/png"),
    manifest: { available: true, localPathWithheld: true },
  };
}

async function presentRecordingResult(result: JsonObject & { videoPath: string; framePaths: string[]; contactSheetPath: string; manifestPath: string }, artifactPreview: InteractTailnetPreview | null) {
  if (!artifactPreview) return result;
  const { videoPath, framePaths, contactSheetPath, manifestPath, ...visible } = result;
  const [preview, poster] = await Promise.all([
    publishPreview(artifactPreview, videoPath, "video/mp4"),
    publishPreview(artifactPreview, contactSheetPath, "image/png"),
  ]);
  return {
    ...visible,
    preview: { ...preview, poster },
    frames: { count: Array.isArray(framePaths) ? framePaths.length : 0, localPathsWithheld: true },
    manifest: { available: true, localPathWithheld: true },
  };
}

async function presentFixedCaptureResult(result: JsonObject & { videoPath: string; contactSheetPath: string; manifestPath: string; frameSummary: JsonObject }, artifactPreview: InteractTailnetPreview | null) {
  if (!artifactPreview) return result;
  const { videoPath, contactSheetPath, manifestPath, frameSummary, ...visible } = result;
  const [preview, poster] = await Promise.all([
    publishPreview(artifactPreview, videoPath, "video/mp4"),
    publishPreview(artifactPreview, contactSheetPath, "image/png"),
  ]);
  const { representativeFramePaths, ...summary } = frameSummary || {};
  return {
    ...visible,
    preview: { ...preview, poster },
    frameSummary: {
      ...summary,
      representativeFrames: Array.isArray(representativeFramePaths) ? representativeFramePaths.length : 0,
      localPathsWithheld: true,
    },
    manifest: { available: true, localPathWithheld: true },
  };
}

function presentRecorderStatus(value: JsonObject, artifactPreview: InteractTailnetPreview | null) {
  if (!artifactPreview || !value || typeof value !== "object") return value;
  const { videoPath, last, ...status } = value;
  if (!last || typeof last !== "object") return videoPath == null ? status : { ...status, localPathWithheld: true };
  const { videoPath: lastVideoPath, framePaths, contactSheetPath, manifestPath, ...lastStatus } = last as JsonObject;
  return {
    ...status,
    ...(videoPath == null ? {} : { localPathWithheld: true }),
    last: {
      ...lastStatus,
      ...(lastVideoPath == null && !Array.isArray(framePaths) && contactSheetPath == null && manifestPath == null
        ? {}
        : { localPathsWithheld: true }),
    },
  };
}

async function publishPreview(artifactPreview: InteractTailnetPreview, filePath: string, mimeType: string) {
  try {
    const preview = await artifactPreview.publish({ filePath, mimeType });
    return { available: true, ...preview, instruction: TAILNET_DELIVERY_INSTRUCTION };
  } catch (error) {
    return {
      available: false,
      code: String(errorCode(error) || "tailnetPreviewUnavailable").slice(0, 80),
      message: conciseError(error),
      instruction: "Do not share a local file path. Restore Tailnet preview availability, then capture again.",
    };
  }
}

async function exportArtifact(workspaceRoot: string, session: InteractSession, { kind, name = "", reproduction = false }: ServiceInput) {
  if (kind !== "setup" && kind !== "replay") throw new InteractError("invalidArtifactKind", "export requires setup or replay kind.");
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
  if (bytes.length > INTERACT_LIMITS.maxArtifactBytes) {
    throw new InteractError("artifactTooLarge", "Artifact exceeds the 8 MiB local file bound.");
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
  fs.writeFileSync(artifactPath, new Uint8Array(bytes), { mode: 0o600 });
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

async function importArtifact(workspaceRoot: string, session: InteractSession, selector: ServiceInput) {
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
  const compact = {
    sessionId: session.sessionId,
    artifactId: selected.artifactId,
    kind: selected.kind,
    path: selected.path,
    imported: true,
    tick: (await session.driver.status()).snapshotTick ?? null,
    aliases: {
      restored: boundedResponseSummary(reconciliation.restored),
      stale: boundedResponseSummary(reconciliation.stale),
    },
    validation: { ok: true, authority: selected.kind === "setup" ? "checkpoint import" : "replay room rebuild" },
  };
  return selector.details === true
    ? { ...compact, aliases: reconciliation, result: importResult }
    : compact;
}

async function inspectArtifact(workspaceRoot: string, session: InteractSession, selector: ServiceInput) {
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

function artifactDirectory(workspaceRoot: string) {
  return path.join(workspaceRoot, "target", "interact", "lab", "artifacts");
}

function readArtifact(workspaceRoot: string, selector: ServiceInput) {
  const kind = selector.kind || null;
  const artifactId = selector.artifactId || null;
  const requestedPath = selector.path || null;
  const directory = artifactDirectory(workspaceRoot);
  let artifactPath: string | undefined;
  if (artifactId) {
    if (!/^artifact_[a-f0-9]{32}$/.test(artifactId)) throw new InteractError("invalidArtifactId", "artifactId is invalid.");
    const candidates = kind
      ? [path.join(directory, `${artifactId}.${kind}.json`)]
      : ["setup", "replay"].map((candidateKind) => path.join(directory, `${artifactId}.${candidateKind}.json`));
    artifactPath = candidates.find((candidate) => fs.existsSync(candidate));
    if (!artifactPath) throw new InteractError("artifactNotFound", "Artifact id does not exist in this worktree.");
  } else {
    if (!requestedPath) throw new InteractError("invalidArtifactPath", "Artifact import requires artifactId or path.");
    if (/^[A-Za-z][A-Za-z0-9+.-]*:/.test(requestedPath)) throw new InteractError("unsafeArtifactPath", "Artifact URLs are not accepted.");
    artifactPath = path.resolve(workspaceRoot, requestedPath);
  }
  const rootPrefix = `${fs.realpathSync(path.join(workspaceRoot, "target", "interact", "lab"))}${path.sep}`;
  let realPath;
  try { realPath = fs.realpathSync(artifactPath); } catch { throw new InteractError("artifactNotFound", "Artifact path does not exist."); }
  if (!realPath.startsWith(rootPrefix)) throw new InteractError("unsafeArtifactPath", "Artifact path must stay beneath target/interact/lab/.");
  const match = path.basename(realPath).match(/^(artifact_[a-f0-9]{32})\.(setup|replay)\.json$/);
  if (!match) throw new InteractError("invalidArtifactPath", "Artifact filename is not an Interact artifact.");
  const resolvedKind = match[2];
  if (kind && kind !== resolvedKind) throw new InteractError("artifactKindMismatch", "Requested artifact kind does not match the file.");
  const bytes = readBoundedFile(realPath, INTERACT_LIMITS.maxArtifactBytes, "artifactTooLarge", "Artifact exceeds 8 MiB.");
  let artifact: unknown;
  try { artifact = JSON.parse(bytes.toString("utf8")) as unknown; } catch { throw new InteractError("invalidArtifact", "Artifact is not valid JSON."); }
  validateArtifactShape(resolvedKind, artifact);
  const sidecarPath = path.join(path.dirname(realPath), `${match[1]}.aliases.json`);
  const aliases = readAliasSidecar(sidecarPath, match[1], resolvedKind);
  return { artifactId: match[1], kind: resolvedKind, path: realPath, sidecarPath, bytes, artifact, aliases };
}

function validateArtifactShape(kind: string, artifact: unknown): asserts artifact is JsonObject {
  if (!isJsonObject(artifact)) throw new InteractError("invalidArtifact", "Artifact must be an object.");
  if (kind === "setup") {
    if (artifact.schemaVersion !== 1 || artifact.kind !== "labCheckpointScenario" || typeof artifact.checkpointPayload !== "string") {
      throw new InteractError("incompatibleArtifact", "Only LabCheckpointScenarioV1 setup artifacts are accepted.");
    }
  } else if (artifact.schema !== "rts.labReplay" || artifact.schemaVersion !== 1 || artifact.kind !== "labReplay" || !Array.isArray(artifact.operations)) {
    throw new InteractError("incompatibleArtifact", "Only LabReplayArtifactV1 replay artifacts are accepted.");
  }
}

function readAliasSidecar(sidecarPath: string, artifactId: string, kind: string): AliasEntry[] {
  if (!fs.existsSync(sidecarPath)) return [];
  let sidecar: unknown;
  try {
    const bytes = readBoundedFile(
      sidecarPath,
      INTERACT_LIMITS.maxAliasSidecarBytes,
      "invalidAliasSidecar",
      "Alias sidecar exceeds 64 KiB.",
    );
    sidecar = JSON.parse(bytes.toString("utf8")) as unknown;
  } catch (error) {
    if (error instanceof InteractError) throw error;
    throw new InteractError("invalidAliasSidecar", "Alias sidecar is invalid JSON.");
  }
  if (!isJsonObject(sidecar) || sidecar.schemaVersion !== 1 || sidecar.artifactId !== artifactId || sidecar.kind !== kind || !Array.isArray(sidecar.aliases) || sidecar.aliases.length > INTERACT_LIMITS.maxAliases) {
    throw new InteractError("invalidAliasSidecar", "Alias sidecar identity or bounds are invalid.");
  }
  const seen = new Set<string>();
  return sidecar.aliases.map((entry: unknown) => {
    if (!isJsonObject(entry) || typeof entry.alias !== "string" || !ALIAS_RE.test(entry.alias) || !Number.isInteger(entry.id) || Number(entry.id) <= 0 || seen.has(entry.alias)) throw new InteractError("invalidAliasSidecar", "Alias sidecar contains an invalid or duplicate entry.");
    seen.add(entry.alias);
    return { alias: entry.alias, id: Number(entry.id) };
  });
}

function readBoundedFile(filePath: string, maxBytes: number, code: string, message: string) {
  let size;
  try { size = fs.statSync(filePath).size; } catch { throw new InteractError("artifactNotFound", "Artifact path does not exist."); }
  if (size > maxBytes) throw new InteractError(code, message);
  const bytes = fs.readFileSync(filePath);
  if (bytes.length > maxBytes) throw new InteractError(code, message);
  return bytes;
}

async function reconcileImportedAliases(session: InteractSession, aliases: AliasEntry[], entityIdMap: unknown) {
  const remaps = new Map(objectArray(entityIdMap).map((entry) => [Number(entry.oldId), Number(entry.newId)]));
  session.aliases.clear();
  const candidates = aliases.map((entry) => ({ ...entry, id: remaps.get(entry.id) || entry.id }));
  const inspected = candidates.length ? await session.driver.inspect({ ids: candidates.map((entry) => entry.id), limit: candidates.length }) : { entities: [] };
  const existing = new Set(objectArray(inspected.entities).map((entity) => Number(entity.id)));
  const restored: Array<AliasEntry & { oldId?: number }> = [];
  const stale: AliasEntry[] = [];
  for (const entry of candidates) {
    if (existing.has(entry.id)) { session.aliases.set(entry.alias, entry.id); restored.push({ alias: entry.alias, oldId: aliases.find((source) => source.alias === entry.alias)?.id, id: entry.id }); }
    else stale.push({ alias: entry.alias, id: entry.id });
  }
  return { restored, stale };
}

async function validateImportedAliases(session: InteractSession, aliases: AliasEntry[]) {
  session.aliases.clear();
  const inspected = aliases.length ? await session.driver.inspect({ ids: aliases.map((entry) => entry.id), limit: aliases.length }) : { entities: [] };
  const ids = new Set(objectArray(inspected.entities).map((entity) => Number(entity.id)));
  const restored: AliasEntry[] = [], stale: AliasEntry[] = [];
  for (const entry of aliases) {
    if (ids.has(entry.id)) { session.aliases.set(entry.alias, entry.id); restored.push(entry); } else stale.push(entry);
  }
  return { restored, stale };
}

async function validateAliasesAgainstArtifact(_session: InteractSession, kind: string, artifact: JsonObject, aliases: AliasEntry[]) {
  const setup = kind === "setup" ? artifact : asJsonObject(artifact.initialSetup);
  const metadata = asJsonObject(setup.metadata);
  const idMap = objectArray(metadata.sourceEntityIdMap);
  const known = new Set([...artifactEntityIds(setup), ...idMap.flatMap((entry) => [Number(entry.oldId), Number(entry.newId)])]);
  return { entries: aliases, stale: aliases.filter((entry) => !known.has(entry.id)), status: "sidecarValidated" };
}

function artifactSummary(kind: string, artifact: JsonObject, aliases: AliasEntry[]) {
  const setup = kind === "setup" ? artifact : asJsonObject(artifact.initialSetup);
  const timeline = asJsonObject(artifact.timeline);
  const map = asJsonObject(setup.map);
  let entityCount = null;
  try {
    entityCount = artifactEntityIds(setup).length;
  } catch {}
  return {
    authoring: kind === "setup" ? { name: artifact.name || "" } : artifact.authoring || {},
    map: Object.keys(map).length ? { name: map.name, contentHash: map.contentHash, materializedHash: map.materializedHash } : null,
    tick: kind === "setup" ? asJsonObject(setup.metadata).exportedTick ?? null : timeline.initialTick ?? null,
    durationTicks: kind === "replay" ? timeline.durationTicks ?? null : 0,
    entityCount,
    operationCount: kind === "replay" && Array.isArray(artifact.operations) ? artifact.operations.length : 0,
    serverBuildSha: kind === "replay" ? artifact.serverBuildSha || null : null,
    aliasCount: aliases.length,
  };
}

function artifactEntityIds(setup: JsonObject): number[] {
  try {
    const parsed = JSON.parse(typeof setup.checkpointPayload === "string" ? setup.checkpointPayload : "") as unknown;
    const entities = asJsonObject(parsed).entities;
    const rows = Array.isArray(entities) ? entities : asJsonObject(entities).entities;
    return objectArray(rows).map((entity) => Number(entity.id)).filter((id) => Number.isInteger(id) && id > 0);
  } catch { return []; }
}

function reproductionSummary(kind: string | undefined, artifactId: string, aliases: AliasEntry[]) {
  const refs = aliases.slice(0, 12).map((entry) => entry.alias).join(", ");
  const input = JSON.stringify({ sessionId: "<current-session-id>", kind, artifactId });
  return `node scripts/interact/cli.mjs lab import '${input}'; aliases: ${refs || "none"}`;
}

function boundedResponseSummary<T>(values: T[]) {
  const source = Array.isArray(values) ? values : [];
  return {
    count: source.length,
    details: source.slice(0, INTERACT_LIMITS.maxResponseDetails),
    truncated: source.length > INTERACT_LIMITS.maxResponseDetails,
  };
}

function recordingOperation(command: string, input: JsonObject, result: unknown) {
  return {
    command,
    acceptedAt: new Date().toISOString(),
    input: JSON.parse(JSON.stringify(input, (key, value) => key === "sessionId" ? undefined : value)),
    authoritativeTick: findSnapshotTick(result),
  };
}

function findSnapshotTick(value: unknown, depth = 0): number | null {
  if (!isJsonObject(value) || depth > 5) return null;
  if (Number.isInteger(value.snapshotTick)) return Number(value.snapshotTick);
  for (const child of Object.values(value)) {
    const tick = findSnapshotTick(child, depth + 1);
    if (tick != null) return tick;
  }
  return null;
}

function projectCatalog(catalog: JsonObject, requested?: string[]) {
  const categories = [...new Set(requested?.length ? requested : ALL_CATALOG_CATEGORIES)];
  const all = {
    maps: Array.isArray(catalog.maps) ? catalog.maps : [], players: Array.isArray(catalog.players) ? catalog.players : [],
    factions: objectArray(catalog.factions).map((faction) => ({
      id: String(faction.id || ""), label: String(faction.label || ""), units: uniqueStrings(faction.units),
      buildings: uniqueStrings(faction.buildings), upgrades: uniqueStrings(faction.upgrades),
    })),
    units: uniqueStrings(flattenFactions(catalog.factions, "units")), buildings: uniqueStrings(flattenFactions(catalog.factions, "buildings")),
    upgrades: uniqueStrings(flattenFactions(catalog.factions, "upgrades")), commands: uniqueStrings(catalog.supportedCommandKinds), abilities: uniqueStrings(catalog.abilities),
  };
  return { categories: Object.fromEntries(categories.map((category) => [category, all[category as keyof typeof all]])), truncated: false };
}

function flattenFactions(factions: unknown, field: string): string[] { return objectArray(factions).flatMap((faction) => Array.isArray(faction[field]) ? faction[field].filter((value): value is string => typeof value === "string") : []); }
function uniqueStrings(values: unknown): string[] { return [...new Set((Array.isArray(values) ? values : []).filter((value): value is string => typeof value === "string"))].sort(); }

function validateSpawnAliases(session: InteractSession, spawns: SpawnInput[]) {
  const aliases = new Set<string>();
  for (const { alias: value } of spawns) {
    if (!value) continue;
    if (session.aliases.has(value) || aliases.has(value)) throw new InteractError("duplicateAlias", `Alias ${JSON.stringify(value)} is already in use.`);
    aliases.add(value);
  }
  if (session.aliases.size + aliases.size > INTERACT_LIMITS.maxAliases) throw new InteractError("aliasLimit", `At most ${INTERACT_LIMITS.maxAliases} aliases are allowed.`);
}

function assertKnownPlayer(catalog: JsonObject, playerId: unknown) {
  if (playerId == null) return;
  if (!objectArray(catalog.players).some((player) => Number(player.id) === Number(playerId))) throw new InteractError("unknownPlayer", `Player ${playerId} is not available.`);
}

function validateCommandCatalog(command: JsonObject, catalog: JsonObject) {
  if (typeof command.c !== "string") throw new InteractError("invalidCommand", "Command kind is required.");
  if (!new Set(uniqueStrings(catalog.supportedCommandKinds)).has(command.c)) throw new InteractError("unsupportedCommand", `${command.c} is not supported.`);
  const validateFrom = (field: string, values: Iterable<unknown>, code: string, label: string) => {
    if (command[field] != null && !new Set(values).has(command[field])) throw new InteractError(code, `${command[field]} is not an available ${label}.`);
  };
  if (command.c === "build") validateFrom("building", flattenFactions(catalog.factions, "buildings"), "invalidKind", "building");
  validateFrom("unit", flattenFactions(catalog.factions, "units"), "invalidKind", "unit");
  validateFrom("upgrade", flattenFactions(catalog.factions, "upgrades"), "invalidUpgrade", "upgrade");
  validateFrom("ability", uniqueStrings(catalog.abilities), "invalidAbility", "ability");
}

async function resolveCommand(session: InteractSession, command: JsonObject) {
  const wire: JsonObject = { ...command }; const resolved: JsonObject = {};
  for (const field of ["units", "buildings"]) {
    if (!Array.isArray(command[field])) continue;
    const entries = await resolveEntityReferences(session, command[field] as EntityRef[]);
    wire[field] = entries.map((entry) => entry.id);
    resolved[field] = entries;
  }
  for (const field of ["target", "node", "building"]) {
    if (command[field] == null || (field === "building" && command.c === "build")) continue;
    const entry = await resolveEntityReference(session, command[field] as EntityRef); wire[field] = entry.id; resolved[field] = entry;
  }
  return { command: wire, resolved };
}

async function resolveEntityReferences(session: InteractSession, references: EntityRef[]): Promise<ResolvedEntityRef[]> {
  const entries: ResolvedEntityRef[] = []; const requestedIds: number[] = [];
  for (const reference of references) {
    if (typeof reference === "number") { entries.push({ input: reference, id: reference, alias: null }); requestedIds.push(reference); continue; }
    const id = session.aliases.get(reference);
    if (!id) throw new InteractError("unknownAlias", `Unknown alias ${JSON.stringify(reference)}.`);
    entries.push({ input: reference, id, alias: reference }); requestedIds.push(id);
  }
  if (new Set(requestedIds).size !== requestedIds.length) throw new InteractError("duplicateReference", "A command may not resolve the same entity twice.");
  const found = new Set();
  for (let offset = 0; offset < requestedIds.length; offset += INTERACT_LIMITS.maxInspectRefs) {
    const ids = requestedIds.slice(offset, offset + INTERACT_LIMITS.maxInspectRefs);
    const existing = await session.driver.inspect({ ids, limit: ids.length });
    for (const entity of objectArray(existing.entities)) found.add(Number(entity.id));
  }
  for (const entry of entries) {
    if (found.has(entry.id)) continue;
    if (entry.alias) session.aliases.delete(entry.alias);
    throw new InteractError(entry.alias ? "staleAlias" : "unknownEntity", entry.alias ? `Alias ${JSON.stringify(entry.alias)} is stale and was cleared.` : `Entity ${entry.id} is unavailable.`);
  }
  return entries;
}

async function resolveEntityReference(session: InteractSession, reference: EntityRef) { return (await resolveEntityReferences(session, [reference]))[0]; }
async function aliasSnapshots(session: InteractSession) {
  if (!session.aliases.size) return [];
  const entries = [...session.aliases].map(([alias, id]) => ({ alias, id }));
  const inspected = await session.driver.inspect({ ids: entries.map((entry) => entry.id), limit: entries.length });
  const byId = new Map(objectArray(inspected.entities).map((entity) => [Number(entity.id), entity]));
  return entries.map((entry) => ({ ...entry, entity: byId.get(entry.id) || null }));
}
async function reconcileAliasesAfterReset(session: InteractSession, before: Array<AliasEntry & { entity: JsonObject | null }>) {
  session.aliases.clear(); const after = await session.driver.inspect({ limit: INTERACT_LIMITS.maxInspectResults });
  const claimed = new Set(); const aliases = []; const clearedAliases = [];
  for (const entry of before) {
    const matches = objectArray(after.entities).filter((entity) => exactAliasMatch(entry.entity, entity) && !claimed.has(entity.id));
    if (matches.length === 1) { const id = Number(matches[0].id); claimed.add(id); session.aliases.set(entry.alias, id); aliases.push({ alias: entry.alias, id }); }
    else clearedAliases.push(entry.alias);
  }
  return { aliases, clearedAliases };
}
function exactAliasMatch(before: JsonObject | null, after: JsonObject) { return !!before && before.kind === after.kind && before.owner === after.owner && Number(before.x) === Number(after.x) && Number(before.y) === Number(after.y); }
function decorateEntity(entity: unknown, aliases: Map<string, number>) { return isJsonObject(entity) ? { ...entity, alias: aliasForEntity(aliases, entity.id) } : entity || null; }
function aliasForEntity(aliases: Map<string, number>, id: unknown) { for (const [alias, entityId] of aliases) if (entityId === id) return alias; return null; }
function clearAliasesForEntity(aliases: Map<string, number>, id: number) { for (const [alias, entityId] of aliases) if (entityId === id) aliases.delete(alias); }

function realWorkspaceRoot(value: string) { try { return fs.realpathSync(value); } catch { throw new InteractError("invalidWorkspace", `Workspace does not exist: ${String(value)}`); } }
function resolveRequestedWorkspace(requested: string | undefined, allowed: string) { const candidate = realWorkspaceRoot(requested || allowed); if (candidate !== allowed) throw new InteractError("workspaceNotAllowed", "open may use only the worktree that launched this daemon."); return candidate; }

export function normalizeError(error: unknown) {
  if (error instanceof InteractError) return error;
  if (error instanceof InteractDriverError) return new InteractError(error.code || "driverError", conciseError(error), error.details || {});
  return new InteractError(errorCode(error) || "commandFailed", conciseError(error));
}
export function conciseError(error: unknown) { return String(error instanceof Error ? error.message : "Interact command failed.").split("\nServer log tail:")[0].slice(0, 1000); }

export async function loadDriverFactory(workspaceRoot = process.cwd()) {
  const modulePath = process.env.RTS_INTERACT_DRIVER_FACTORY_MODULE;
  if (!modulePath) return undefined;
  const module = await import(pathToFileURL(path.resolve(workspaceRoot, modulePath)).href);
  if (typeof module.openInteractDriver !== "function") throw new InteractError("invalidDriverFactory", "Driver factory module must export openInteractDriver(options).");
  return module.openInteractDriver as DriverFactory;
}

function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asJsonObject(value: unknown): JsonObject {
  return isJsonObject(value) ? value : {};
}

function objectArray(value: unknown): JsonObject[] {
  return Array.isArray(value) ? value.filter(isJsonObject) : [];
}

function errorCode(error: unknown): string | null {
  return isJsonObject(error) && typeof error.code === "string" ? error.code : null;
}
