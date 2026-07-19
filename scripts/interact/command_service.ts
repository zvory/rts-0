// Transport-neutral, bounded command service for Interact.
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { InteractDriver, InteractDriverError } from "./driver.ts";
import { ALL_CATALOG_CATEGORIES, INTERACT_LIMITS } from "./command_inputs.ts";
import { commandDefinition } from "./command_registry.ts";
import { SessionCoordinator } from "./session_coordinator.ts";
import { gameSessionCapabilities, scenarioSessionCapabilities } from "./game_session.ts";
import { defaultMapForMode } from "./session_defaults.ts";
import { executeLabCommand } from "./namespaces/lab/commands.ts";
import { executeGameCommand } from "./namespaces/game/commands.ts";
import { executeDevScenarioCommand } from "./namespaces/dev_scenario/commands.ts";
import {
  presentRecorderStatus, stopRecording, waitForRecording,
} from "./capabilities/media.ts";
import type { InteractTailnetPreview } from "./tailnet_preview.ts";
import { InteractError } from "./service_contract.ts";
import type { InteractSession, JsonObject, ServiceInput } from "./service_contract.ts";
export { InteractError } from "./service_contract.ts";
export { INTERACT_LIMITS } from "./command_inputs.ts";
export { INTERACT_COMMANDS } from "./command_registry.ts";
type DriverFactory = (options: ConstructorParameters<typeof InteractDriver>[0]) => Promise<InteractDriver>;
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
        result = await waitForRecording(session!, this.artifactPreview);
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
    if (command.startsWith("game-") && session.kind !== "game") {
      throw new InteractError("sessionKindMismatch", "This command requires a game session.");
    }
    if (command.startsWith("scenario-") && session.kind !== "scenario") {
      throw new InteractError("sessionKindMismatch", "This command requires a dev scenario session.");
    }
    if (command === "record-stop") return stopRecording(session, this.artifactPreview);
    if (command.startsWith("game-")) return executeGameCommand(command, session, input, this.artifactPreview);
    if (command.startsWith("scenario-")) return executeDevScenarioCommand(command, session, input, this.artifactPreview);
    if (session.kind !== "lab") {
      throw new InteractError("sessionKindMismatch", "This command requires a Lab session.");
    }
    return executeLabCommand(command, session, input, this.artifactPreview, this.workspaceRoot);
  }
}

export function validateCommandInput(command: string, input: unknown) {
  const definition = commandDefinition(command);
  if (!definition) {
    throw new InteractError("unknownCommand", `Unknown command ${JSON.stringify(command)}.`);
  }
  return definition.validator(input);
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

function errorCode(error: unknown): string | null {
  return isJsonObject(error) && typeof error.code === "string" ? error.code : null;
}
