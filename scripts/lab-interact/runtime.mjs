import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export const DEFAULT_IDLE_MS = 30 * 60_000;
export const MAX_REQUEST_BYTES = 1024 * 1024;
export const IPC_VERSION = 1;
export const REQUEST_TIMEOUT_MS = 120_000;

export function runtimePaths(workspaceRoot, { tmpDir = os.tmpdir() } = {}) {
  const root = fs.realpathSync(workspaceRoot);
  const key = crypto.createHash("sha256").update(root).digest("hex").slice(0, 24);
  const owner = typeof process.getuid === "function" ? process.getuid() : "user";
  const runtimeName = `rts-lab-interact-${owner}`;
  let parent = path.join(tmpDir, runtimeName);
  let directory = path.join(parent, key);
  // Darwin's sockaddr_un path is very short. Keep the private socket below the portable bound
  // even when os.tmpdir() expands to a long /var/folders path.
  if (Buffer.byteLength(path.join(directory, "daemon.sock")) > 96) {
    parent = path.join("/tmp", runtimeName);
    directory = path.join(parent, key);
  }
  return {
    workspaceRoot: root,
    parent,
    directory,
    socket: path.join(directory, "daemon.sock"),
    state: path.join(directory, "state.json"),
    lock: path.join(directory, "startup.lock"),
  };
}

export function configuredIdleMs(env = process.env) {
  if (env.RTS_LAB_INTERACT_IDLE_MS == null) return DEFAULT_IDLE_MS;
  const value = Number(env.RTS_LAB_INTERACT_IDLE_MS);
  if (!Number.isInteger(value) || value < 20 || value > DEFAULT_IDLE_MS) {
    throw new Error(`RTS_LAB_INTERACT_IDLE_MS must be an integer from 20 to ${DEFAULT_IDLE_MS}.`);
  }
  return value;
}

export function prepareRuntime(paths) {
  fs.mkdirSync(paths.parent, { recursive: true, mode: 0o700 });
  fs.chmodSync(paths.parent, 0o700);
  fs.mkdirSync(paths.directory, { recursive: true, mode: 0o700 });
  fs.chmodSync(paths.directory, 0o700);
}

export function writeState(paths, state) {
  const temporary = `${paths.state}.${process.pid}.tmp`;
  fs.writeFileSync(temporary, `${JSON.stringify(state)}\n`, { mode: 0o600 });
  fs.renameSync(temporary, paths.state);
}

export function readState(paths) {
  try {
    return JSON.parse(fs.readFileSync(paths.state, "utf8"));
  } catch {
    return null;
  }
}

export function processAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error?.code === "EPERM";
  }
}

export function cleanupRuntime(paths) {
  fs.rmSync(paths.directory, { recursive: true, force: true });
}

export function cleanupOwnedRuntime(paths, daemonId, pid = process.pid) {
  const state = readState(paths);
  if (state?.daemonId !== daemonId || state?.pid !== pid) return false;
  cleanupRuntime(paths);
  return true;
}

export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
