#!/usr/bin/env node

import assert from "node:assert/strict";
import { execFile, execFileSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const helper = path.join(repoRoot, "scripts", "ensure-node-deps.sh");
const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-node-deps-test-"));
const cacheRoot = path.join(fixtureRoot, "cache");
const fakeBin = path.join(fixtureRoot, "bin");
const installCounter = path.join(fixtureRoot, "npm-ci-count");

function writeManifest(worktree, dependencies) {
  fs.mkdirSync(worktree, { recursive: true });
  fs.writeFileSync(path.join(worktree, "package.json"), `${JSON.stringify({
    name: "node-deps-fixture",
    private: true,
    devDependencies: dependencies,
  }, null, 2)}\n`);
  fs.writeFileSync(path.join(worktree, "package-lock.json"), `${JSON.stringify({
    name: "node-deps-fixture",
    lockfileVersion: 3,
    fixtureDependencies: dependencies,
  }, null, 2)}\n`);
}

function runHelper(worktree) {
  return execFileSync("bash", [helper, "--repo", worktree, "--cache-dir", cacheRoot, "--quiet"], {
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: `${fakeBin}:${process.env.PATH}`,
      RTS_FAKE_NPM_COUNTER: installCounter,
      RTS_NODE_DEPS_WAIT_SECONDS: "5",
    },
  });
}

const children = new Set();
function startHelper(worktree, extraEnv = {}) {
  let child;
  const result = new Promise((resolve, reject) => {
    child = execFile("bash", [helper, "--repo", worktree, "--cache-dir", cacheRoot, "--quiet"], {
      encoding: "utf8",
      env: {
        ...process.env,
        PATH: `${fakeBin}:${process.env.PATH}`,
        RTS_FAKE_NPM_COUNTER: installCounter,
        RTS_NODE_DEPS_WAIT_SECONDS: "10",
        ...extraEnv,
      },
    }, (error, stdout, stderr) => {
      children.delete(child);
      error ? reject(Object.assign(error, { stdout, stderr })) : resolve(stdout);
    });
    children.add(child);
  });
  // Some cases deliberately kill a child before awaiting its result.
  result.catch(() => {});
  return { child, result };
}

async function waitForFile(filename) {
  const deadline = Date.now() + 10000;
  while (!fs.existsSync(filename)) {
    assert(Date.now() < deadline, `timed out waiting for ${filename}`);
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}

function cacheEntry(worktree) {
  const digest = crypto.createHash("sha256");
  for (const filename of ["package.json", "package-lock.json"]) {
    digest.update(fs.readFileSync(path.join(worktree, filename))).update("\0");
  }
  return path.join(cacheRoot, digest.digest("hex"));
}

function installCount() {
  return fs.existsSync(installCounter)
    ? fs.readFileSync(installCounter, "utf8").trim().split("\n").filter(Boolean).length
    : 0;
}

fs.mkdirSync(fakeBin, { recursive: true });
const fakeNpm = path.join(fakeBin, "npm");
fs.writeFileSync(fakeNpm, `#!/usr/bin/env node
const fs = require("node:fs");
const path = require("node:path");
if (process.argv[2] !== "ci") process.exit(2);
fs.appendFileSync(process.env.RTS_FAKE_NPM_COUNTER, "ci\\n");
const gate = process.env.RTS_FAKE_NPM_GATE;
if (gate) {
  fs.writeFileSync(gate + ".started", String(process.pid));
  while (!fs.existsSync(gate)) Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 10);
}
if (process.env.RTS_FAKE_NPM_FAIL === "1") process.exit(1);
const manifest = JSON.parse(fs.readFileSync("package.json", "utf8"));
for (const name of Object.keys(manifest.devDependencies || {})) {
  fs.mkdirSync(path.join("node_modules", ...name.split("/")), { recursive: true });
}
`);
fs.chmodSync(fakeNpm, 0o755);

try {
  const first = path.join(fixtureRoot, "first");
  const second = path.join(fixtureRoot, "second");
  const dependencies = { pngjs: "^7.0.0", typescript: "^5.8.0" };
  writeManifest(first, dependencies);
  writeManifest(second, dependencies);
  fs.mkdirSync(path.join(first, "node_modules", "local-only"), { recursive: true });

  // Hold the first install at a known boundary, rather than hoping two starts
  // overlap during an arbitrary 200 ms sleep on a busy CI runner.
  const gate = path.join(fixtureRoot, "first-install");
  const firstInstall = startHelper(first, { RTS_FAKE_NPM_GATE: gate });
  await waitForFile(`${gate}.started`);
  assert.equal(fs.existsSync(cacheEntry(first)), false, "in-progress install is not published");
  const timeout = startHelper(second, { RTS_NODE_DEPS_WAIT_SECONDS: "0" });
  await assert.rejects(timeout.result, /timed out waiting for dependency cache lock/);
  assert.equal(installCount(), 1, "a contender must not enter the live install");
  const secondInstall = startHelper(second);
  fs.writeFileSync(gate, "release");
  const installs = await Promise.allSettled([firstInstall.result, secondInstall.result]);
  for (const install of installs) {
    if (install.status === "rejected") throw install.reason;
  }
  fs.rmSync(`${gate}.started`);
  assert.equal(fs.lstatSync(path.join(first, "node_modules")).isSymbolicLink(), true);
  assert.equal(fs.existsSync(path.join(first, "node_modules", "pngjs")), true);
  assert.equal(installCount(), 1, "concurrent worktrees install the shared lockfile once");

  runHelper(second);
  assert.equal(fs.lstatSync(path.join(second, "node_modules")).isSymbolicLink(), true);
  assert.equal(fs.realpathSync(path.join(first, "node_modules")), fs.realpathSync(path.join(second, "node_modules")));
  assert.equal(installCount(), 1, "a second worktree reuses the same complete cache");

  fs.rmSync(path.join(second, "node_modules", "pngjs"), { recursive: true, force: true });
  runHelper(second);
  assert.equal(fs.existsSync(path.join(second, "node_modules", "pngjs")), true);
  assert.equal(installCount(), 2, "a cache missing a declared dependency is rebuilt");

  const previousCache = fs.realpathSync(path.join(second, "node_modules"));
  writeManifest(second, { ...dependencies, "new-package": "1.0.0" });
  runHelper(second);
  assert.notEqual(fs.realpathSync(path.join(second, "node_modules")), previousCache);
  assert.equal(fs.existsSync(path.join(second, "node_modules", "new-package")), true);
  assert.equal(installCount(), 3, "a changed lockfile gets a separate cache and relinks the worktree");

  // The kernel releases an abandoned lock; no PID files, ownerless grace
  // periods, or stale-directory reclamation can steal it from a live process.
  const interrupted = path.join(fixtureRoot, "interrupted");
  writeManifest(interrupted, { "interrupted-package": "1.0.0" });
  const interruptedGate = path.join(fixtureRoot, "interrupted-install");
  const interruptedInstall = startHelper(interrupted, { RTS_FAKE_NPM_GATE: interruptedGate });
  await waitForFile(`${interruptedGate}.started`);
  const npmPid = Number(fs.readFileSync(`${interruptedGate}.started`, "utf8"));
  interruptedInstall.child.kill("SIGKILL");
  await assert.rejects(interruptedInstall.result);
  // npm outlives its killed wrapper and must keep the lock until it exits.
  const orphanContender = startHelper(interrupted, { RTS_NODE_DEPS_WAIT_SECONDS: "1" });
  try {
    await assert.rejects(orphanContender.result, /timed out waiting for dependency cache lock/);
  } finally {
    process.kill(npmPid, "SIGKILL");
    fs.rmSync(`${interruptedGate}.started`);
  }
  runHelper(interrupted);
  assert.equal(fs.existsSync(path.join(interrupted, "node_modules", "interrupted-package")), true);
  assert.equal(installCount(), 5, "killed install is retried once after its processes exit");

  const failed = path.join(fixtureRoot, "failed");
  writeManifest(failed, { "retry-package": "1.0.0" });
  await assert.rejects(startHelper(failed, { RTS_FAKE_NPM_FAIL: "1" }).result, /npm ci failed/);
  assert.equal(fs.existsSync(cacheEntry(failed)), false, "failed install is never published");
  runHelper(failed);
  assert.equal(fs.existsSync(path.join(failed, "node_modules", "retry-package")), true);
  assert.equal(installCount(), 7, "failed install releases the lock for a successful retry");

  const unsafe = path.join(fixtureRoot, "unsafe");
  writeManifest(unsafe, dependencies);
  const unsafeNodeModules = path.join(unsafe, "node_modules");
  fs.mkdirSync(unsafeNodeModules);
  fs.writeFileSync(path.join(unsafeNodeModules, "keep.txt"), "keep\n");
  assert.throws(
    () => execFileSync("bash", [helper, "--repo", unsafe, "--cache-dir", path.join(unsafeNodeModules, "cache"), "--quiet"], {
      encoding: "utf8",
      env: { ...process.env, PATH: `${fakeBin}:${process.env.PATH}` },
    }),
    /cache directory must not be inside/,
  );
  assert.equal(fs.readFileSync(path.join(unsafeNodeModules, "keep.txt"), "utf8"), "keep\n");
  assert.equal(fs.lstatSync(unsafeNodeModules).isDirectory(), true, "unsafe cache placement leaves node_modules intact");

  const deletedCwd = path.join(fixtureRoot, "deleted-cwd");
  fs.mkdirSync(deletedCwd);
  execFileSync("bash", ["-c", `
cd "$1"
rmdir "$1"
exec bash "$2" --repo "$3" --cache-dir "$4" --quiet
`, "deleted-cwd-test", deletedCwd, helper, first, cacheRoot], {
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: `${fakeBin}:${process.env.PATH}`,
      RTS_FAKE_NPM_COUNTER: installCounter,
      RTS_NODE_DEPS_WAIT_SECONDS: "5",
    },
  });
} finally {
  // Kill gated installers as well as wrappers if an assertion fails mid-scenario.
  for (const name of ["first-install", "interrupted-install"]) {
    const started = path.join(fixtureRoot, `${name}.started`);
    if (fs.existsSync(started)) {
      try { process.kill(Number(fs.readFileSync(started, "utf8")), "SIGKILL"); }
      catch (error) { if (error.code !== "ESRCH") throw error; }
    }
  }
  for (const child of children) child.kill("SIGKILL");
  while (children.size) await new Promise((resolve) => setTimeout(resolve, 10));
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
}

console.log("ensure_node_deps: shared install, contention, crash recovery, failure retry, and relinking passed");
