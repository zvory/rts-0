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

function runHelperAsync(worktree) {
  return new Promise((resolve, reject) => {
    execFile("bash", [helper, "--repo", worktree, "--cache-dir", cacheRoot, "--quiet"], {
      encoding: "utf8",
      env: {
        ...process.env,
        PATH: `${fakeBin}:${process.env.PATH}`,
        RTS_FAKE_NPM_COUNTER: installCounter,
        RTS_FAKE_NPM_DELAY_MS: "200",
        RTS_NODE_DEPS_WAIT_SECONDS: "5",
      },
    }, (error, stdout, stderr) => error ? reject(Object.assign(error, { stdout, stderr })) : resolve(stdout));
  });
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
const delay = Number(process.env.RTS_FAKE_NPM_DELAY_MS || 0);
if (delay) Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, delay);
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

  await Promise.all([runHelperAsync(first), runHelperAsync(second)]);
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

  writeManifest(second, { ...dependencies, "new-package": "1.0.0" });
  const staleLock = `${cacheEntry(second)}.lock`;
  fs.mkdirSync(staleLock, { recursive: true });
  fs.writeFileSync(path.join(staleLock, "owner"), "99999999-dead\n");
  runHelper(second);
  assert.equal(fs.existsSync(path.join(second, "node_modules", "new-package")), true);
  assert.equal(installCount(), 3, "a dead installer lock is reclaimed for the new cache entry");

  writeManifest(second, { ...dependencies, "ownerless-lock-package": "1.0.0" });
  fs.mkdirSync(`${cacheEntry(second)}.lock`, { recursive: true });
  runHelper(second);
  assert.equal(fs.existsSync(path.join(second, "node_modules", "ownerless-lock-package")), true);
  assert.equal(installCount(), 4, "an interrupted lock handoff without an owner is reclaimed");

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
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
}

console.log("ensure_node_deps: shared install, reuse, completeness, and relinking passed");
