#!/usr/bin/env node

import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { LAB_INTERACT_COMMANDS } from "./command_service.mjs";
import {
  IPC_VERSION, REQUEST_TIMEOUT_MS, prepareRuntime, processAlive, readState, runtimePaths, sleep,
} from "./runtime.mjs";

const STARTUP_TIMEOUT_MS = 15_000;
const scriptDir = path.dirname(fileURLToPath(import.meta.url));

export async function runCli(argv = process.argv.slice(2), { cwd = process.cwd(), env = process.env } = {}) {
  if (argv.length < 1 || argv.length > 2) throw cliError("usage", "Usage: node scripts/lab-interact/cli.mjs <command> [JSON-object]");
  const [command, rawInput = "{}"] = argv;
  if (!LAB_INTERACT_COMMANDS.includes(command)) throw cliError("unknownCommand", `Unknown command ${JSON.stringify(command)}.`);
  let input;
  try { input = JSON.parse(rawInput); } catch { throw cliError("invalidJson", "Input must be one valid JSON object argument."); }
  if (!input || typeof input !== "object" || Array.isArray(input)) throw cliError("invalidJson", "Input must be a JSON object.");
  const workspaceRoot = gitRoot(cwd);
  const paths = runtimePaths(workspaceRoot);
  let response = null;
  let requestError = null;
  try { response = await requestCurrent(paths, { command, input }); } catch (error) { requestError = error; }
  if (!response) {
    if (command === "shutdown") {
      await confirmStopped(paths, requestError);
      return { ok: true, result: { shuttingDown: false, alreadyStopped: true } };
    }
    await ensureDaemon(paths, env);
    response = await requestCurrent(paths, { command, input });
  }
  return response;
}

async function ensureDaemon(paths, env) {
  prepareRuntime(paths);
  const deadline = Date.now() + STARTUP_TIMEOUT_MS;
  while (Date.now() < deadline) {
    if (await daemonReady(paths)) return;
    const existing = readState(paths);
    if (processAlive(existing?.pid)) {
      throw cliError("daemonUnreachable", "A live Lab Interact daemon owns this worktree runtime but did not answer a compatible request.");
    }
    let lock;
    let spawned = false;
    try {
      lock = fs.openSync(paths.lock, "wx", 0o600);
      fs.writeFileSync(lock, `${JSON.stringify({ pid: process.pid, createdAt: Date.now() })}\n`);
    } catch (error) {
      if (error?.code !== "EEXIST") throw error;
      if (staleStartup(paths)) fs.rmSync(paths.lock, { force: true });
      await sleep(25);
      continue;
    }
    try {
      if (await daemonReady(paths)) return;
      const probe = await probeSocket(paths);
      if (probe.live) {
        throw cliError("daemonStateUnavailable", "A live Lab Interact daemon owns the socket, but its authenticated state is missing or incompatible.");
      }
      const owner = readState(paths);
      if (processAlive(owner?.pid)) {
        throw cliError("daemonUnreachable", "A live Lab Interact daemon owner is recorded but its socket is unavailable.");
      }
      // The startup lock is ours and an active probe proved that no process is listening.
      fs.rmSync(paths.socket, { force: true });
      fs.rmSync(paths.state, { force: true });
      const child = spawn(process.execPath, [path.join(scriptDir, "daemon.mjs"), paths.workspaceRoot], {
        cwd: paths.workspaceRoot,
        env,
        detached: true,
        stdio: "ignore",
      });
      child.unref();
      spawned = true;
    } finally {
      fs.closeSync(lock);
      if (!spawned) removeOwnedStartupLock(paths);
    }
    while (Date.now() < deadline) {
      if (await daemonReady(paths)) return;
      const state = readState(paths);
      if (state && !processAlive(state.pid)) break;
      await sleep(25);
    }
  }
  throw cliError("daemonStartup", "Lab Interact daemon did not become ready within 15 seconds.");
}

function staleStartup(paths) {
  let lock;
  try { lock = JSON.parse(fs.readFileSync(paths.lock, "utf8")); } catch {
    try { return Date.now() - fs.statSync(paths.lock).mtimeMs > STARTUP_TIMEOUT_MS; } catch { return false; }
  }
  return !processAlive(lock.pid);
}

function removeOwnedStartupLock(paths) {
  try {
    const lock = JSON.parse(fs.readFileSync(paths.lock, "utf8"));
    if (lock.pid === process.pid) fs.rmSync(paths.lock, { force: true });
  } catch {}
}

function requestCurrent(paths, payload, timeoutMs = REQUEST_TIMEOUT_MS) {
  const state = readState(paths);
  if (!validIdentity(paths, state)) return Promise.reject(cliError("daemonIdentity", "No compatible Lab Interact daemon is ready."));
  return request(paths.socket, {
    protocolVersion: IPC_VERSION,
    daemonId: state.daemonId,
    capability: state.capability,
    ...payload,
  }, timeoutMs);
}

function request(socketPath, payload, timeoutMs) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(socketPath);
    socket.setEncoding("utf8");
    socket.setTimeout(timeoutMs, () => socket.destroy(cliError("requestTimeout", `Daemon request exceeded ${timeoutMs}ms.`)));
    let body = "";
    socket.once("connect", () => socket.write(`${JSON.stringify(payload)}\n`));
    socket.on("data", (chunk) => { body += chunk; });
    socket.once("error", reject);
    socket.once("end", () => {
      try { resolve(JSON.parse(body)); } catch { reject(cliError("invalidDaemonResponse", "Daemon returned invalid JSON.")); }
    });
  });
}

async function daemonReady(paths) {
  const state = readState(paths);
  if (!validIdentity(paths, state)) return false;
  try {
    const response = await requestCurrent(paths, { command: "status", input: {} }, 1_000);
    return response?.ok === true;
  } catch {
    return false;
  }
}

async function probeSocket(paths) {
  if (!fs.existsSync(paths.socket)) return { live: false, reason: "missing" };
  try {
    const response = await request(paths.socket, { protocolVersion: IPC_VERSION, probe: "lab-interact" }, 500);
    if (response?.ok === true && response?.probe?.protocolVersion === IPC_VERSION) {
      return { live: true, compatible: true, ...response.probe };
    }
    return { live: true, compatible: false, reason: "unexpectedResponse" };
  } catch (error) {
    if (["ECONNREFUSED", "ENOENT", "ENOTSOCK", "EINVAL"].includes(error?.code)) return { live: false, reason: error.code };
    return { live: true, compatible: false, reason: error?.code || "probeFailed" };
  }
}

async function confirmStopped(paths, requestError) {
  if (!fs.existsSync(paths.directory)) return;
  const state = readState(paths);
  if (processAlive(state?.pid)) throw requestError || cliError("daemonUnreachable", "A live Lab Interact daemon is not accepting shutdown.");
  const initialProbe = await probeSocket(paths);
  if (initialProbe.live) throw requestError || cliError("daemonOccupied", "A live process still owns the Lab Interact socket.");
  prepareRuntime(paths);
  let lock;
  let cleaned = false;
  try {
    lock = fs.openSync(paths.lock, "wx", 0o600);
    fs.writeFileSync(lock, `${JSON.stringify({ pid: process.pid, createdAt: Date.now() })}\n`);
  } catch (error) {
    if (error?.code === "EEXIST" && staleStartup(paths)) {
      fs.rmSync(paths.lock, { force: true });
      return confirmStopped(paths, requestError);
    }
    if (error?.code === "EEXIST") throw requestError || cliError("daemonStarting", "Lab Interact startup is still in progress.");
    throw error;
  }
  try {
    const owner = readState(paths);
    const probe = await probeSocket(paths);
    if (processAlive(owner?.pid) || probe.live) {
      throw requestError || cliError("daemonOccupied", "A live process still owns the Lab Interact runtime.");
    }
    fs.rmSync(paths.socket, { force: true });
    fs.rmSync(paths.state, { force: true });
    cleaned = true;
  } finally {
    fs.closeSync(lock);
    removeOwnedStartupLock(paths);
  }
  if (cleaned) {
    try { fs.rmdirSync(paths.directory); } catch (error) { if (error?.code !== "ENOENT" && error?.code !== "ENOTEMPTY") throw error; }
  }
}

function validIdentity(paths, state) {
  return state?.protocolVersion === IPC_VERSION &&
    typeof state.daemonId === "string" && state.daemonId.length >= 16 &&
    typeof state.capability === "string" && /^[a-f0-9]{64}$/.test(state.capability) &&
    state.workspaceRoot === paths.workspaceRoot && state.socket === paths.socket && processAlive(state.pid);
}

function gitRoot(cwd) {
  const result = spawnSync("git", ["rev-parse", "--show-toplevel"], { cwd, encoding: "utf8" });
  if (result.status !== 0) throw cliError("invalidWorkspace", "Run Lab Interact from a Git worktree.");
  return fs.realpathSync(result.stdout.trim());
}

function cliError(code, message) { return Object.assign(new Error(message), { code }); }

export async function main() {
  try {
    const response = await runCli();
    const stream = response.ok ? process.stdout : process.stderr;
    stream.write(`${JSON.stringify(response)}\n`);
    if (!response.ok) process.exitCode = 1;
  } catch (error) {
    process.stderr.write(`${JSON.stringify({ ok: false, error: { code: error.code || "cliFailed", message: String(error.message).slice(0, 1000) } })}\n`);
    process.exitCode = 1;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) void main();
