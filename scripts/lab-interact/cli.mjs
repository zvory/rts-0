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
  let response = await requestCurrent(paths, { command, input }).catch(() => null);
  if (!response) {
    if (command === "shutdown") return { ok: true, result: { shuttingDown: false, alreadyStopped: true } };
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
      throw cliError("daemonIncompatible", "A live but incompatible Lab Interact daemon owns this worktree runtime.");
    }
    let lock;
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
      fs.rmSync(paths.socket, { force: true });
      fs.rmSync(paths.state, { force: true });
      const child = spawn(process.execPath, [path.join(scriptDir, "daemon.mjs"), paths.workspaceRoot], {
        cwd: paths.workspaceRoot,
        env,
        detached: true,
        stdio: "ignore",
      });
      child.unref();
    } finally {
      fs.closeSync(lock);
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
  try { lock = JSON.parse(fs.readFileSync(paths.lock, "utf8")); } catch { return true; }
  if (Date.now() - Number(lock.createdAt) > STARTUP_TIMEOUT_MS) return true;
  return !processAlive(lock.pid);
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
