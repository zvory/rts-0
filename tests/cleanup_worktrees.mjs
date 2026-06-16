#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-cleanup-worktrees-"));
const repo = path.join(fixtureRoot, "repo");
const origin = path.join(fixtureRoot, "origin.git");
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

function commitFile(name, text) {
  fs.writeFileSync(path.join(repo, name), `${text}\n`);
  git(["add", name]);
  git(["commit", "-m", text]);
}

function createWorktreeBranch(branchName, dirName, fileName) {
  const worktreePath = path.join(worktreeRoot, dirName);
  git(["worktree", "add", "-b", branchName, worktreePath, "main"]);
  fs.writeFileSync(path.join(worktreePath, fileName), `${branchName}\n`);
  git(["-C", worktreePath, "add", fileName]);
  git(["-C", worktreePath, "commit", "-m", branchName]);
  return worktreePath;
}

fs.mkdirSync(repo, { recursive: true });
fs.mkdirSync(worktreeRoot, { recursive: true });
git(["init", "--initial-branch=main"], { cwd: repo });
git(["config", "user.email", "agent@example.invalid"]);
git(["config", "user.name", "Agent"]);
commitFile("base.txt", "base");

git(["init", "--bare", origin], { cwd: fixtureRoot });
git(["remote", "add", "origin", origin]);
git(["push", "-u", "origin", "main"]);

try {
  const merged = createWorktreeBranch("zvorygin/merged-pr", "merged-pr", "merged.txt");
  git(["push", "-u", "origin", "zvorygin/merged-pr"]);
  git(["merge", "--no-ff", "zvorygin/merged-pr", "-m", "Merge branch zvorygin/merged-pr"]);
  git(["push", "origin", "main"]);
  git(["push", "origin", "--delete", "zvorygin/merged-pr"]);

  const unmerged = createWorktreeBranch("zvorygin/unmerged-pr", "unmerged-pr", "unmerged.txt");
  git(["push", "-u", "origin", "zvorygin/unmerged-pr"]);
  git(["push", "origin", "--delete", "zvorygin/unmerged-pr"]);

  const dirtyMerged = createWorktreeBranch(
    "zvorygin/dirty-merged-pr",
    "dirty-merged-pr",
    "dirty-merged.txt",
  );
  git(["merge", "--no-ff", "zvorygin/dirty-merged-pr", "-m", "Merge branch zvorygin/dirty-merged-pr"]);
  fs.writeFileSync(path.join(dirtyMerged, "uncommitted.txt"), "keep me\n");

  const cleanupScript = path.join(repoRoot, "scripts", "cleanup-worktrees.sh");
  const output = run("bash", [cleanupScript], {
    env: {
      RTS_WORKTREE_ROOT: worktreeRoot,
      RTS_CARGO_TARGET_BASE_DIR: targetRoot,
    },
  });

  assert.match(output, /removing merged clean worktree .*merged-pr/);
  assert.equal(fs.existsSync(merged), false, "clean merged PR worktree should be removed");
  assert.equal(fs.existsSync(unmerged), true, "unmerged worktree must be kept after remote deletion");
  assert.equal(fs.existsSync(dirtyMerged), true, "dirty merged worktree must be kept");
  assert.doesNotThrow(() => git(["rev-parse", "--verify", "--quiet", "zvorygin/unmerged-pr"]));
} finally {
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
}
