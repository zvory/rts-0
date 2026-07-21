#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "..");
const waitScript = path.join(projectRoot, "scripts", "wait-pr.sh");
const cleanupScript = path.join(projectRoot, "scripts", "cleanup-worktrees.sh");
const patchNoteScript = path.join(projectRoot, "scripts", "patch-note-pass.mjs");
const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-wait-pr-"));
const repo = path.join(fixtureRoot, "repo");
const origin = path.join(fixtureRoot, "origin.git");
const publisher = path.join(fixtureRoot, "publisher");
const worktreeRoot = path.join(fixtureRoot, "worktrees");
const targetRoot = path.join(fixtureRoot, "targets");

function run(command, args, options = {}) {
  return execFileSync(command, args, {
    cwd: options.cwd ?? repo,
    env: { ...process.env, ...(options.env ?? {}) },
    encoding: "utf8",
    stdio: options.stdio ?? "pipe",
  });
}

function git(args, options = {}) {
  return run("git", args, options);
}

function configureIdentity(cwd) {
  git(["config", "user.email", "agent@example.invalid"], { cwd });
  git(["config", "user.name", "Agent"], { cwd });
}

function commitFile(cwd, name, text) {
  fs.writeFileSync(path.join(cwd, name), `${text}\n`);
  git(["add", name], { cwd });
  git(["commit", "-m", text], { cwd });
}

function createTask(branch, directory, fileName) {
  const taskPath = path.join(worktreeRoot, directory);
  git(["worktree", "add", "-b", branch, taskPath, "origin/main"]);
  commitFile(taskPath, fileName, branch);
  git(["push", "-u", "origin", branch], { cwd: taskPath });
  return { taskPath, headSha: git(["rev-parse", "HEAD"], { cwd: taskPath }).trim() };
}

function addPatchNote(taskPath, branch, text) {
  const slug = branch.replace(/^zvorygin\//, "");
  const relativePath = path.join("patch-notes", "2026-07-20", `${slug}.md`);
  fs.mkdirSync(path.dirname(path.join(taskPath, relativePath)), { recursive: true });
  fs.writeFileSync(
    path.join(taskPath, relativePath),
    `<!-- rts-patch-note:v1 -->\n<!-- branch: ${branch} -->\n# Fixture\n\n## Changes\n\n- ${text}\n`,
  );
  git(["add", relativePath], { cwd: taskPath });
  git(["commit", "-m", "Add patch note"], { cwd: taskPath });
  git(["push", "origin", branch], { cwd: taskPath });
  return git(["rev-parse", "HEAD"], { cwd: taskPath }).trim();
}

function mergeTask(branch) {
  git(["fetch", "origin", branch], { cwd: publisher });
  git(["merge", "--no-ff", `origin/${branch}`, "-m", `Merge ${branch}`], { cwd: publisher });
  git(["push", "origin", "main"], { cwd: publisher });
}

function mergedViewJson(headSha, number, files = []) {
  return JSON.stringify({
    number,
    url: `https://example.invalid/pull/${number}`,
    state: "MERGED",
    mergedAt: "2026-07-11T00:00:00Z",
    headRefOid: headSha,
    headRefName: `zvorygin/wait-pr-${number}`,
    baseRefName: "main",
    autoMergeRequest: null,
    mergeStateStatus: "CLEAN",
    isDraft: false,
    files: files.map((filePath) => ({ path: filePath })),
  });
}

function waitEnvironment(headSha, number, files = []) {
  return {
    RTS_WAIT_PR_VIEW_JSON: mergedViewJson(headSha, number, files),
    RTS_WAIT_PR_CHECKS_JSON: "[]",
    RTS_WORKTREE_ROOT: worktreeRoot,
    RTS_CARGO_TARGET_BASE_DIR: targetRoot,
  };
}

fs.mkdirSync(path.join(repo, "scripts"), { recursive: true });
fs.mkdirSync(worktreeRoot, { recursive: true });
fs.copyFileSync(cleanupScript, path.join(repo, "scripts", "cleanup-worktrees.sh"));
fs.chmodSync(path.join(repo, "scripts", "cleanup-worktrees.sh"), 0o755);
fs.copyFileSync(patchNoteScript, path.join(repo, "scripts", "patch-note-pass.mjs"));
fs.chmodSync(path.join(repo, "scripts", "patch-note-pass.mjs"), 0o755);
git(["init", "--initial-branch=main"], { cwd: repo });
configureIdentity(repo);
commitFile(repo, "base.txt", "base");
commitFile(repo, "local-note.txt", "committed local note");
git(["add", "scripts/cleanup-worktrees.sh", "scripts/patch-note-pass.mjs"]);
git(["commit", "--amend", "--no-edit"]);
git(["config", "branch.main.mergeOptions", "--no-ff"]);

git(["init", "--bare", "--initial-branch=main", origin], { cwd: fixtureRoot });
git(["remote", "add", "origin", origin]);
git(["push", "-u", "origin", "main"]);
git(["clone", origin, publisher], { cwd: fixtureRoot });
configureIdentity(publisher);

try {
  const firstBranch = "zvorygin/wait-pr-41";
  const first = createTask(firstBranch, "wait-pr-41", "first.txt");
  first.headSha = addPatchNote(first.taskPath, firstBranch, "Merged fixture change.");
  mergeTask(firstBranch);
  fs.writeFileSync(path.join(repo, "local-note.txt"), "preserve me\n");

  const output = run("bash", [waitScript, "41"], {
    cwd: first.taskPath,
    env: waitEnvironment(first.headSha, 41, ["first.txt", "patch-notes/2026-07-20/wait-pr-41.md"]),
  });

  assert.match(output, /refreshing local main checkout/);
  assert.match(output, /Discord webhook not configured/);
  assert.ok(
    output.indexOf("Discord webhook not configured") < output.indexOf("refreshing local main checkout"),
    "patch-note delivery should happen after merge verification and before cleanup",
  );
  assert.match(output, /local main is current/);
  assert.equal(
    git(["rev-parse", "main"]).trim(),
    git(["rev-parse", "origin/main"]).trim(),
    "wait-pr should fast-forward the checked-out local main branch",
  );
  assert.equal(fs.readFileSync(path.join(repo, "local-note.txt"), "utf8"), "preserve me\n");
  assert.equal(fs.existsSync(first.taskPath), false, "post-merge cleanup should remove the task worktree");

  const secondBranch = "zvorygin/wait-pr-42";
  const second = createTask(secondBranch, "wait-pr-42", "second.txt");
  commitFile(repo, "local-main-only.txt", "local main divergence");
  mergeTask(secondBranch);

  assert.throws(
    () =>
      run("bash", [waitScript, "42"], {
        cwd: second.taskPath,
        env: waitEnvironment(second.headSha, 42),
      }),
    (error) => error.status !== 0,
    "a divergent local main must fail instead of creating a merge commit",
  );
  assert.notEqual(git(["rev-parse", "main"]).trim(), git(["rev-parse", "origin/main"]).trim());
  assert.equal(fs.existsSync(second.taskPath), true, "cleanup must not run after a failed fast-forward");
} finally {
  fs.rmSync(fixtureRoot, { recursive: true, force: true, maxRetries: 5, retryDelay: 50 });
}
