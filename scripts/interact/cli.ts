#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import {
  INTERACT_NAMESPACES, namespaceCommandKey, requestTimeoutMs,
} from "./command_registry.ts";
import { commandHelp, helpCatalog } from "./command_help.ts";
import {
  IPC_VERSION, checkoutCommit, configuredIdleMs, prepareRuntime, processAlive,
  readStartupError, readState, reclaimStaleStartupLock, removeOwnedStartupLock,
  runtimePaths, sleep,
} from "./runtime.ts";
import type { RuntimePaths, RuntimeRecord } from "./runtime.ts";

const STARTUP_TIMEOUT_MS = 15_000;
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const USAGE = "node scripts/interact/cli.mjs <lab|game|scenario> <command> [JSON-object]";
const NAMESPACE_SUMMARIES = Object.freeze({
  lab: "Arrange and inspect authoritative Lab scenes.",
  game: "Observe and minimally control one isolated human-vs-AI match.",
  scenario: "Observe and capture one authored server-backed dev scenario.",
});

interface DaemonIdentity extends RuntimeRecord {
  protocolVersion: number;
  daemonId: string;
  capability: string;
  workspaceRoot: string;
  socket: string;
  pid: number;
}

interface RequestPayload extends RuntimeRecord {
  command: string;
  input: RuntimeRecord;
}

interface IpcResponse {
  ok: boolean;
  result: RuntimeRecord;
  error?: { code: string; message: string; details?: RuntimeRecord };
  probe?: RuntimeRecord;
}

interface CodedError extends Error {
  code: string;
  details?: RuntimeRecord;
}

export async function runCli(argv = process.argv.slice(2), { cwd = process.cwd(), env = process.env } = {}) {
  if (argv.length === 1 && ["--help", "-h", "help"].includes(argv[0])) {
    return {
      ok: true,
      result: {
        usage: USAGE,
        namespaces: Object.entries(NAMESPACE_SUMMARIES).map(([name, summary]) => ({ name, summary })),
        documentation: "docs/interact-cli.md",
      },
    };
  }
  const namespaceHelp = argv[0] === "help";
  const namespace = namespaceHelp ? argv[1] : argv[0];
  if (!namespace || !(namespace in INTERACT_NAMESPACES)) {
    if (namespaceHelp && namespace) {
      throw cliError("unknownNamespace", `Unknown Interact namespace ${JSON.stringify(namespace)}. Available namespaces: lab, game, scenario.`);
    }
    throw cliError("unknownNamespace", `Interact requires a namespace. Usage: ${USAGE}`);
  }
  const namespaceArgv = namespaceHelp ? argv.slice(2) : argv.slice(1);
  const publicCommands = INTERACT_NAMESPACES[namespace];
  const namespaceUsage = `node scripts/interact/cli.mjs ${namespace} <command> [JSON-object]`;
  if (namespaceArgv.length === 0 || (namespaceArgv.length === 1 && ["--help", "-h", "help"].includes(namespaceArgv[0]))) {
    return {
      ok: true,
      result: {
        namespace,
        usage: namespaceUsage,
        commands: [...publicCommands],
        catalog: helpCatalog(namespace),
        documentation: "docs/interact-cli.md",
      },
    };
  }
  let helpCommand: string | null = null;
  if (namespaceHelp && namespaceArgv.length === 1) {
    helpCommand = namespaceArgv[0];
  } else if (namespaceArgv.length === 2 && namespaceArgv[0] === "help") {
    helpCommand = namespaceArgv[1];
  } else if (namespaceArgv.length === 2 && ["--help", "-h"].includes(namespaceArgv[1])) {
    helpCommand = namespaceArgv[0];
  }
  if (helpCommand != null) {
    if (!publicCommands.includes(helpCommand)) {
      throw cliError("unknownCommand", `Unknown command ${JSON.stringify(helpCommand)}.`);
    }
    return {
      ok: true,
      result: {
        namespace,
        command: helpCommand,
        ...commandHelp(helpCommand, namespace),
        documentation: "docs/interact-cli.md",
      },
    };
  }
  if (namespaceHelp) throw cliError("usage", `Usage: ${namespaceUsage}`);
  if (namespaceArgv.length < 1 || namespaceArgv.length > 2) throw cliError("usage", `Usage: ${namespaceUsage}`);
  const [publicCommand, rawInput = "{}"] = namespaceArgv;
  const command = namespaceCommandKey(namespace, publicCommand);
  if (!command) throw cliError("unknownCommand", `Unknown command ${JSON.stringify(publicCommand)}.`);
  let input: unknown;
  try { input = JSON.parse(rawInput); } catch { throw cliError("invalidJson", "Input must be one valid JSON object argument."); }
  if (!isRecord(input)) throw cliError("invalidJson", "Input must be a JSON object.");
  const workspaceRoot = gitRoot(cwd);
  const currentCheckoutCommit = checkoutCommit(workspaceRoot);
  const paths = runtimePaths(workspaceRoot);
  let response: IpcResponse | null = null;
  let requestError: unknown = null;
  try {
    response = await requestForCheckout(paths, { command, input }, currentCheckoutCommit, env);
  } catch (error) {
    requestError = error;
  }
  if (!response) {
    if (command === "shutdown") {
      await confirmStopped(paths, requestError);
      return { ok: true, result: { shuttingDown: false, alreadyStopped: true } };
    }
    await ensureDaemon(paths, env);
    response = await requestForCheckout(paths, { command, input }, currentCheckoutCommit, env);
  }
  return response;
}

async function requestForCheckout(paths: RuntimePaths, payload: RequestPayload, currentCheckoutCommit: string, env: NodeJS.ProcessEnv): Promise<IpcResponse> {
  const state = readState(paths);
  if (!validIdentity(paths, state)) {
    throw cliError("daemonIdentity", "No compatible Interact daemon is ready.");
  }
  const daemonCommit = typeof state.checkoutCommit === "string" && /^[a-f0-9]{40}$/.test(state.checkoutCommit)
    ? state.checkoutCommit
    : null;
  const checkoutMatches = daemonCommit === currentCheckoutCommit;
  if (payload.command === "status") {
    const response = await requestCurrent(paths, payload);
    if (response?.ok) {
      response.result.daemonCheckout = {
        daemonCommit,
        checkoutCommit: currentCheckoutCommit,
        matches: checkoutMatches,
      };
    }
    return response;
  }
  if (payload.command === "shutdown" || checkoutMatches) {
    return requestCurrent(paths, payload);
  }
  if (daemonCommit == null) {
    return checkoutMismatchEnvelope(null, currentCheckoutCommit);
  }
  const refresh = await requestCurrent(paths, {
    command: "shutdown",
    input: {},
    refreshCheckout: currentCheckoutCommit,
  });
  if (!refresh?.ok) return refresh;
  await waitForDaemonExit(paths, state.pid);
  await ensureDaemon(paths, env);
  return requestCurrent(paths, payload);
}

function checkoutMismatchEnvelope(daemonCommit: string | null, currentCheckoutCommit: string): IpcResponse {
  return {
    ok: false,
    result: {},
    error: {
      code: "daemonCheckoutMismatch",
      message: "The Interact daemon has no checkout metadata and may predate safe idle refresh. It was preserved; inspect status, then explicitly shut down when it is safe to discard.",
      details: {
        daemonCommit,
        checkoutCommit: currentCheckoutCommit,
        recoveryCommand: "node scripts/interact/cli.mjs lab shutdown",
      },
    },
  };
}

async function waitForDaemonExit(paths: RuntimePaths, pid: unknown) {
  const deadline = Date.now() + 5_000;
  while (Date.now() < deadline) {
    if (!processAlive(pid) && !fs.existsSync(paths.directory)) return;
    await sleep(25);
  }
  throw cliError("daemonRefreshTimeout", "The idle mismatched daemon did not finish shutting down within 5 seconds.");
}

async function ensureDaemon(paths: RuntimePaths, env: NodeJS.ProcessEnv) {
  try {
    configuredIdleMs(env);
  } catch (error) {
    throw cliError("invalidConfiguration", errorMessage(error));
  }
  prepareRuntime(paths);
  const deadline = Date.now() + STARTUP_TIMEOUT_MS;
  while (Date.now() < deadline) {
    if (await daemonReady(paths)) return;
    const existing = readState(paths);
    if (processAlive(existing?.pid)) {
      throw cliError("daemonUnreachable", "A live Interact daemon owns this worktree runtime but did not answer a compatible request.");
    }
    let lock: number;
    let child: ReturnType<typeof spawn> | null = null;
    let childFailure: CodedError | null = null;
    let spawned = false;
    const startupNonce = crypto.randomBytes(16).toString("hex");
    try {
      lock = fs.openSync(paths.lock, "wx", 0o600);
      fs.writeFileSync(lock, `${JSON.stringify({ nonce: startupNonce, role: "cli", pid: process.pid, createdAt: Date.now() })}\n`);
    } catch (error) {
      if (!hasErrorCode(error, "EEXIST")) throw error;
      reclaimStaleStartupLock(paths);
      await sleep(25);
      continue;
    }
    try {
      if (await daemonReady(paths)) return;
      const probe = await probeSocket(paths);
      if (probe.live) {
        throw cliError("daemonStateUnavailable", "A live Interact daemon owns the socket, but its authenticated state is missing or incompatible.");
      }
      const owner = readState(paths);
      if (processAlive(owner?.pid)) {
        throw cliError("daemonUnreachable", "A live Interact daemon owner is recorded but its socket is unavailable.");
      }
      // The startup lock is ours and an active probe proved that no process is listening.
      fs.rmSync(paths.socket, { force: true });
      fs.rmSync(paths.state, { force: true });
      fs.rmSync(paths.startupError, { force: true });
      child = spawn(process.execPath, [path.join(scriptDir, "daemon.ts"), paths.workspaceRoot, startupNonce], {
        cwd: paths.workspaceRoot,
        env,
        detached: true,
        stdio: "ignore",
      });
      child.once("error", (error) => { childFailure = asCodedError(error); });
      child.once("exit", (code, signal) => {
        childFailure ||= cliError(
          "daemonStartup",
          `Interact daemon exited before readiness (${signal ? `signal ${signal}` : `code ${code}`}).`,
        );
      });
      child.unref();
      spawned = true;
    } finally {
      fs.closeSync(lock);
      if (!spawned) removeOwnedStartupLock(paths, startupNonce, process.pid, "cli");
    }
    while (Date.now() < deadline) {
      if (await daemonReady(paths)) return;
      const startupError = readStartupError(paths);
      if (startupError?.nonce === startupNonce || childFailure) {
        removeOwnedStartupLock(paths, startupNonce, process.pid, "cli");
        if (child?.pid) removeOwnedStartupLock(paths, startupNonce, child.pid, "daemon");
        fs.rmSync(paths.startupError, { force: true });
        try { fs.rmdirSync(paths.directory); } catch (error) { if (!hasErrorCode(error, "ENOENT") && !hasErrorCode(error, "ENOTEMPTY")) throw error; }
        throw cliError(
          typeof startupError?.code === "string" ? startupError.code : errorCode(childFailure) || "daemonStartup",
          typeof startupError?.message === "string" ? startupError.message : errorMessage(childFailure || "Interact daemon exited before readiness."),
        );
      }
      const state = readState(paths);
      if (state && !processAlive(state.pid)) break;
      await sleep(25);
    }
  }
  throw cliError("daemonStartup", "Interact daemon did not become ready within 15 seconds.");
}

function requestCurrent(paths: RuntimePaths, payload: RequestPayload, timeoutMs: number | null = null): Promise<IpcResponse> {
  const state = readState(paths);
  if (!validIdentity(paths, state)) return Promise.reject(cliError("daemonIdentity", "No compatible Interact daemon is ready."));
  return request(paths.socket, {
    protocolVersion: IPC_VERSION,
    daemonId: state.daemonId,
    capability: state.capability,
    ...payload,
  }, timeoutMs ?? requestTimeoutMs(payload.command));
}

function request(socketPath: string, payload: RuntimeRecord, timeoutMs: number): Promise<IpcResponse> {
  return new Promise<IpcResponse>((resolve, reject) => {
    const socket = net.createConnection(socketPath);
    socket.setEncoding("utf8");
    socket.setTimeout(timeoutMs, () => socket.destroy(cliError("requestTimeout", `Daemon request exceeded ${timeoutMs}ms.`)));
    let body = "";
    socket.once("connect", () => socket.write(`${JSON.stringify(payload)}\n`));
    socket.on("data", (chunk) => { body += chunk; });
    socket.once("error", reject);
    socket.once("end", () => {
      try { resolve(parseIpcResponse(body)); } catch { reject(cliError("invalidDaemonResponse", "Daemon returned invalid JSON.")); }
    });
  });
}

async function daemonReady(paths: RuntimePaths) {
  const state = readState(paths);
  if (!validIdentity(paths, state)) return false;
  try {
    const response = await requestCurrent(paths, { command: "status", input: {} }, 1_000);
    return response?.ok === true;
  } catch {
    return false;
  }
}

async function probeSocket(paths: RuntimePaths) {
  if (!fs.existsSync(paths.socket)) return { live: false, reason: "missing" };
  try {
    const response = await request(paths.socket, { protocolVersion: IPC_VERSION, probe: "interact" }, 500);
    if (response?.ok === true && response?.probe?.protocolVersion === IPC_VERSION) {
      return { live: true, compatible: true, ...response.probe };
    }
    return { live: true, compatible: false, reason: "unexpectedResponse" };
  } catch (error) {
    const code = errorCode(error);
    if (["ECONNREFUSED", "ENOENT", "ENOTSOCK", "EINVAL"].includes(code)) return { live: false, reason: code };
    return { live: true, compatible: false, reason: code || "probeFailed" };
  }
}

async function confirmStopped(paths: RuntimePaths, requestError: unknown): Promise<void> {
  if (!fs.existsSync(paths.directory)) return;
  const state = readState(paths);
  if (processAlive(state?.pid)) throw requestError || cliError("daemonUnreachable", "A live Interact daemon is not accepting shutdown.");
  const initialProbe = await probeSocket(paths);
  if (initialProbe.live) throw requestError || cliError("daemonOccupied", "A live process still owns the Interact socket.");
  prepareRuntime(paths);
  let lock;
  let startupNonce = "";
  let cleaned = false;
  try {
    lock = fs.openSync(paths.lock, "wx", 0o600);
    startupNonce = crypto.randomBytes(16).toString("hex");
    fs.writeFileSync(lock, `${JSON.stringify({ nonce: startupNonce, role: "cli", pid: process.pid, createdAt: Date.now() })}\n`);
  } catch (error) {
    if (hasErrorCode(error, "EEXIST") && reclaimStaleStartupLock(paths)) {
      return confirmStopped(paths, requestError);
    }
    if (hasErrorCode(error, "EEXIST")) throw requestError || cliError("daemonStarting", "Interact startup is still in progress.");
    throw error;
  }
  try {
    const owner = readState(paths);
    const probe = await probeSocket(paths);
    if (processAlive(owner?.pid) || probe.live) {
      throw requestError || cliError("daemonOccupied", "A live process still owns the Interact runtime.");
    }
    fs.rmSync(paths.socket, { force: true });
    fs.rmSync(paths.state, { force: true });
    cleaned = true;
  } finally {
    fs.closeSync(lock);
    removeOwnedStartupLock(paths, startupNonce, process.pid, "cli");
  }
  if (cleaned) {
    try { fs.rmdirSync(paths.directory); } catch (error) { if (!hasErrorCode(error, "ENOENT") && !hasErrorCode(error, "ENOTEMPTY")) throw error; }
  }
}

function validIdentity(paths: RuntimePaths, state: RuntimeRecord | null): state is DaemonIdentity {
  return state?.protocolVersion === IPC_VERSION &&
    typeof state.daemonId === "string" && state.daemonId.length >= 16 &&
    typeof state.capability === "string" && /^[a-f0-9]{64}$/.test(state.capability) &&
    state.workspaceRoot === paths.workspaceRoot && state.socket === paths.socket && processAlive(state.pid);
}

function gitRoot(cwd: string) {
  const result = spawnSync("git", ["rev-parse", "--show-toplevel"], { cwd, encoding: "utf8" });
  if (result.status !== 0) throw cliError("invalidWorkspace", "Run Interact from a Git worktree.");
  return fs.realpathSync(result.stdout.trim());
}

function cliError(code: string, message: string): CodedError { return Object.assign(new Error(message), { code }); }

export async function main() {
  try {
    const response = await runCli();
    const stream = response.ok ? process.stdout : process.stderr;
    stream.write(`${JSON.stringify(response)}\n`);
    if (!response.ok) process.exitCode = 1;
  } catch (error) {
    const normalized = asCodedError(error);
    process.stderr.write(`${JSON.stringify({ ok: false, error: { code: normalized.code || "cliFailed", message: normalized.message.slice(0, 1000), ...(normalized.details ? { details: normalized.details } : {}) } })}\n`);
    process.exitCode = 1;
  }
}

function parseIpcResponse(body: string): IpcResponse {
  const value: unknown = JSON.parse(body);
  if (!value || typeof value !== "object" || Array.isArray(value) || typeof (value as RuntimeRecord).ok !== "boolean") {
    throw new TypeError("Invalid IPC response.");
  }
  const response = value as RuntimeRecord;
  return {
    ...response,
    ok: response.ok as boolean,
    result: isRecord(response.result) ? response.result : {},
    ...(isRecord(response.probe) ? { probe: response.probe } : {}),
  } as IpcResponse;
}

function isRecord(value: unknown): value is RuntimeRecord {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function errorCode(error: unknown): string {
  return isRecord(error) && typeof error.code === "string" ? error.code : "";
}

function hasErrorCode(error: unknown, code: string): boolean { return errorCode(error) === code; }
function errorMessage(error: unknown): string { return error instanceof Error ? error.message : String(error); }
function asCodedError(error: unknown): CodedError {
  if (error instanceof Error) return Object.assign(error, { code: errorCode(error) || "cliFailed" }) as CodedError;
  return cliError("cliFailed", String(error));
}
