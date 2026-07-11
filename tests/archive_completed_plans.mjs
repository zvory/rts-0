#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  archivePlanDirectories,
  findArchivablePlans,
  phaseMarkedDoneText,
  planNameFromActivePhasePath,
} from "../scripts/archive-completed-plans.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function run(command, args, cwd) {
  return execFileSync(command, args, { cwd, encoding: "utf8" }).trim();
}

function write(file, text) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
}

assert.equal(phaseMarkedDoneText("Status: Done.\n"), true);
assert.equal(phaseMarkedDoneText("## Status\n\nDone.\n"), true);
assert.equal(phaseMarkedDoneText("## Phase Status\n\n- [x] Done.\n"), true);
assert.equal(phaseMarkedDoneText("## Phase Status\n\n- [x] Done. Manual QA remains.\n"), true);
assert.equal(phaseMarkedDoneText("Status: in progress.\n"), false);
assert.equal(phaseMarkedDoneText("Status: Done-ish.\n"), false);
assert.equal(phaseMarkedDoneText("Status: Done? No.\n"), false);
assert.equal(planNameFromActivePhasePath("plans/example/phase-2.md"), "example");
assert.equal(planNameFromActivePhasePath("plans/lab/room/phase-2.5-bakeoff.md"), "lab/room");
assert.equal(planNameFromActivePhasePath("plans/archive/example/phase-2.md"), "");

const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-archive-completed-plans-"));
try {
  run("git", ["init", "-b", "main"], fixtureRoot);
  run("git", ["config", "user.email", "archive-test@example.invalid"], fixtureRoot);
  run("git", ["config", "user.name", "Archive Test"], fixtureRoot);

  for (const plan of ["ready", "stale", "incomplete"]) {
    write(path.join(fixtureRoot, "plans", plan, "plan.md"), `# ${plan}\n`);
  }
  write(path.join(fixtureRoot, "plans/ready/phase-1.md"), "Status: Done.\n");
  write(path.join(fixtureRoot, "plans/ready/phase-2.md"), "Status: Not started.\n");
  write(path.join(fixtureRoot, "plans/stale/phase-1.md"), "Status: Done.\n");
  write(path.join(fixtureRoot, "plans/incomplete/phase-1.md"), "Status: Not started.\n");
  write(path.join(fixtureRoot, "plans/incomplete/phase-2.md"), "Status: Done.\n");
  write(path.join(fixtureRoot, "plans/incomplete/subplan/phase-3.md"), "Status: Not started.\n");
  run("git", ["add", "plans"], fixtureRoot);
  run("git", ["commit", "-m", "Add plans"], fixtureRoot);
  run("git", ["checkout", "-b", "zvorygin/final-phase"], fixtureRoot);

  write(path.join(fixtureRoot, "plans/ready/phase-2.md"), "Status: Done.\n");
  fs.appendFileSync(path.join(fixtureRoot, "plans/stale/phase-1.md"), "\nTouched after completion.\n");
  write(path.join(fixtureRoot, "plans/incomplete/phase-1.md"), "Status: Done.\n");
  run("git", ["add", "plans"], fixtureRoot);
  run("git", ["commit", "-m", "Complete one plan"], fixtureRoot);

  assert.deepEqual(findArchivablePlans({ repoRoot: fixtureRoot, baseRef: "main" }), ["ready"]);
  const result = spawnSync(
    "node",
    [path.join(repoRoot, "scripts/archive-completed-plans.mjs"), "--base", "main", "--commit"],
    { cwd: fixtureRoot, encoding: "utf8" },
  );
  assert.equal(result.status, 0, `stdout:\n${result.stdout}\nstderr:\n${result.stderr}`);
  assert.match(result.stdout, /plans\/ready -> plans\/archive\/ready/);
  assert.equal(fs.existsSync(path.join(fixtureRoot, "plans/ready")), false);
  assert.equal(fs.existsSync(path.join(fixtureRoot, "plans/archive/ready/phase-2.md")), true);
  assert.equal(fs.existsSync(path.join(fixtureRoot, "plans/stale/phase-1.md")), true);
  assert.equal(fs.existsSync(path.join(fixtureRoot, "plans/incomplete/phase-1.md")), true);
  assert.equal(run("git", ["status", "--porcelain=v1"], fixtureRoot), "");
  assert.equal(run("git", ["log", "-1", "--format=%s"], fixtureRoot), "Archive completed plan: ready");

  const collisionRoot = path.join(fixtureRoot, "collision");
  write(path.join(collisionRoot, "plans/example/plan.md"), "# Active\n");
  write(path.join(collisionRoot, "plans/archive/example/plan.md"), "# Existing\n");
  assert.throws(
    () => archivePlanDirectories({ repoRoot: collisionRoot, planNames: ["example"] }),
    /archive destination already exists/,
  );
  assert.equal(fs.existsSync(path.join(collisionRoot, "plans/example/plan.md")), true);
} finally {
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
}

console.log("completed plan archival tests passed");
