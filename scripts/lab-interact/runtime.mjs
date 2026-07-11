import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

export const DEFAULT_IDLE_MS = 30 * 60_000;
export const MAX_REQUEST_BYTES = 1024 * 1024;
export const IPC_VERSION = 1;
export const REQUEST_TIMEOUT_MS = 120_000;
// A one-minute wait can be followed by recorder flush, transcode, three auxiliary
// FFmpeg stages, probes, and browser cleanup. Keep the client deadline beyond the
// sum of those independently bounded stages so successful cleanup can reply.
export const RECORDING_REQUEST_TIMEOUT_MS = 420_000;
export const STARTUP_GRACE_MS = 15_000;

export function requestTimeoutMs(command) {
  return ["record-stop", "record-wait", "close", "shutdown"].includes(command)
    ? RECORDING_REQUEST_TIMEOUT_MS
    : REQUEST_TIMEOUT_MS;
}

export function checkoutCommit(workspaceRoot) {
  const result = spawnSync("git", ["rev-parse", "HEAD"], { cwd: workspaceRoot, encoding: "utf8" });
  const commit = String(result.stdout || "").trim().toLowerCase();
  if (result.status !== 0 || !/^[a-f0-9]{40}$/.test(commit)) {
    throw new Error("Lab Interact could not read the checkout commit.");
  }
  return commit;
}

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
    startupError: path.join(directory, "startup-error.json"),
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

export function writeStartupError(paths, error) {
  const temporary = `${paths.startupError}.${process.pid}.tmp`;
  fs.writeFileSync(temporary, `${JSON.stringify(error)}\n`, { mode: 0o600 });
  fs.renameSync(temporary, paths.startupError);
}

export function readStartupError(paths) {
  try {
    return JSON.parse(fs.readFileSync(paths.startupError, "utf8"));
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

export function readStartupLock(paths) {
  try {
    return JSON.parse(fs.readFileSync(paths.lock, "utf8"));
  } catch {
    return null;
  }
}

export function startupLockStale(paths, now = Date.now()) {
  const lock = readStartupLock(paths);
  let stat;
  try { stat = fs.statSync(paths.lock); } catch { return false; }
  const validRecord = /^[a-f0-9]{32}$/.test(lock?.nonce) &&
    ["cli", "daemon"].includes(lock?.role) && Number.isInteger(lock?.pid) && lock.pid > 0 &&
    Number.isFinite(lock?.createdAt) && lock.createdAt <= now + 1_000;
  if (!validRecord) return (stat.mtimeMs > now ? STARTUP_GRACE_MS + 1 : now - stat.mtimeMs) > STARTUP_GRACE_MS;
  return now - Number(lock.createdAt) > STARTUP_GRACE_MS && !processAlive(lock.pid);
}

export function claimStartupLock(paths, nonce) {
  const fd = fs.openSync(paths.lock, "r+");
  try {
    const current = JSON.parse(fs.readFileSync(fd, "utf8"));
    if (current?.nonce !== nonce || current?.role !== "cli") {
      throw new Error("Lab Interact startup lock nonce no longer belongs to this daemon.");
    }
    const claimed = { ...current, role: "daemon", pid: process.pid, claimedAt: Date.now() };
    fs.ftruncateSync(fd, 0);
    fs.writeSync(fd, `${JSON.stringify(claimed)}\n`, 0, "utf8");
    fs.fsyncSync(fd);
  } finally {
    fs.closeSync(fd);
  }
  if (!startupLockOwned(paths, nonce, process.pid, "daemon")) {
    throw new Error("Lab Interact startup lock changed while the daemon was claiming it.");
  }
}

export function startupLockOwned(paths, nonce, pid, role) {
  const lock = readStartupLock(paths);
  return lock?.nonce === nonce && lock?.pid === pid && lock?.role === role;
}

export function removeOwnedStartupLock(paths, nonce, pid, role) {
  if (!startupLockOwned(paths, nonce, pid, role)) return false;
  const moved = `${paths.lock}.release-${process.pid}-${crypto.randomBytes(4).toString("hex")}`;
  try {
    fs.renameSync(paths.lock, moved);
  } catch {
    return false;
  }
  if (startupLockOwned({ ...paths, lock: moved }, nonce, pid, role)) {
    fs.rmSync(moved, { force: true });
    return true;
  }
  if (!fs.existsSync(paths.lock)) {
    try { fs.renameSync(moved, paths.lock); } catch {}
  }
  return false;
}

export function reclaimStaleStartupLock(paths) {
  const expected = readStartupLock(paths);
  let expectedStat;
  try { expectedStat = fs.statSync(paths.lock); } catch { return false; }
  if (!startupLockStale(paths)) return false;
  const moved = `${paths.lock}.stale-${process.pid}-${crypto.randomBytes(4).toString("hex")}`;
  try {
    fs.renameSync(paths.lock, moved);
  } catch {
    return false;
  }
  const actual = readStartupLock({ ...paths, lock: moved });
  let actualStat;
  try { actualStat = fs.statSync(moved); } catch { return false; }
  const sameRecord = expected
    ? actual?.nonce === expected.nonce && actual?.pid === expected.pid && actual?.role === expected.role && actual?.createdAt === expected.createdAt
    : !actual && actualStat.dev === expectedStat.dev && actualStat.ino === expectedStat.ino && actualStat.mtimeMs === expectedStat.mtimeMs && actualStat.size === expectedStat.size;
  if (sameRecord) {
    fs.rmSync(moved, { force: true });
    return true;
  }
  if (!fs.existsSync(paths.lock)) {
    try { fs.renameSync(moved, paths.lock); } catch {}
  }
  return false;
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
