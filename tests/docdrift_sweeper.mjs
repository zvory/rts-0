#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const script = path.join(repoRoot, "scripts", "docdrift-sweep.mjs");
const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-docdrift-sweeper-"));
const repo = path.join(fixtureRoot, "repo");

function run(command, args, options = {}) {
  return execFileSync(command, args, {
    cwd: options.cwd ?? repo,
    env: {
      ...process.env,
      GIT_AUTHOR_DATE: options.date ?? "2026-06-20T12:00:00Z",
      GIT_COMMITTER_DATE: options.date ?? "2026-06-20T12:00:00Z",
      ...(options.env ?? {}),
    },
    encoding: "utf8",
    stdio: options.stdio ?? "pipe",
  });
}

function git(args, options = {}) {
  return run("git", args, options).trim();
}

function writeRepoFile(name, text) {
  const absPath = path.join(repo, name);
  fs.mkdirSync(path.dirname(absPath), { recursive: true });
  fs.writeFileSync(absPath, text);
}

function commitFile(name, text, subject, body = "", options = {}) {
  writeRepoFile(name, text);
  git(["add", name]);
  const messageArgs = ["commit", "-m", subject];
  if (body) {
    messageArgs.push("-m", body);
  }
  git(messageArgs, options);
  return git(["rev-parse", "HEAD"]);
}

function sweep(args) {
  return run("node", [script, "--dry-run", "--repo", repo, ...args], {
    cwd: repoRoot,
  });
}

fs.mkdirSync(repo, { recursive: true });

try {
  git(["init", "--initial-branch=main"]);
  git(["config", "user.email", "agent@example.invalid"]);
  git(["config", "user.name", "Agent"]);

  writeRepoFile(
    "docs/doc-map.json",
    JSON.stringify(
      {
        version: 1,
        routes: [
          {
            source: ["server/crates/sim/src/game/**"],
            docs: ["docs/context/server-sim.md", "docs/design/server-sim.md"],
            notes: "Sim changes route to server sim docs.",
          },
          {
            source: ["tests/**", "scripts/check-*.mjs"],
            docs: ["docs/context/testing.md", "docs/design/testing.md"],
            notes: "Test and check changes route to testing docs.",
          },
        ],
      },
      null,
      2,
    ) + "\n",
  );
  writeRepoFile("docs/context/server-sim.md", "server sim capsule\n");
  writeRepoFile("docs/design/server-sim.md", "server sim design\n");
  writeRepoFile("docs/context/testing.md", "testing capsule\n");
  writeRepoFile("docs/design/testing.md", "testing design\n");
  git(["add", "."]);
  git(["commit", "-m", "Initial docs"]);
  const base = git(["rev-parse", "HEAD"]);
  writeRepoFile(
    "docs/docdrift-checkpoint.txt",
    [
      "# Documentation drift sweeper checkpoint.",
      "# Updated only after a sweep PR merges.",
      base,
      "",
    ].join("\n"),
  );
  git(["add", "docs/docdrift-checkpoint.txt"]);
  git(["commit", "-m", "Add docdrift checkpoint"], { date: "2026-06-20T12:00:30Z" });
  const checkpointCommit = git(["rev-parse", "HEAD"]);

  const simCommit = commitFile(
    "server/crates/sim/src/game/mod.rs",
    "pub struct Game;\n",
    "Change sim API",
    "Adds a new public helper that may need docs.",
    { date: "2026-06-20T12:01:00Z" },
  );
  commitFile(
    "docs/design/testing.md",
    "testing design\nmore detail\n",
    "Clarify testing docs",
    "",
    { date: "2026-06-20T12:02:00Z" },
  );
  git(["checkout", "-b", "feature-test-route"]);
  const testCommit = commitFile(
    "tests/server_integration.mjs",
    "console.log('test');\n",
    "Add integration coverage",
    "",
    { date: "2026-06-20T12:03:00Z" },
  );
  git(["checkout", "main"]);
  git(["merge", "--no-ff", "feature-test-route", "-m", "Merge feature test route"], {
    date: "2026-06-20T12:04:00Z",
  });
  const head = git(["rev-parse", "HEAD"]);

  const jsonReport = JSON.parse(sweep(["--base", checkpointCommit, "--head", head, "--format", "json"]));
  assert.equal(jsonReport.version, 1);
  assert.equal(jsonReport.mode, "dry-run");
  assert.equal(jsonReport.base.sha, checkpointCommit);
  assert.equal(jsonReport.head.sha, head);
  assert.equal(jsonReport.traceMap.path, "docs/doc-map.json");
  assert.deepEqual(jsonReport.summary, {
    totalCommits: 4,
    consideredCommits: 2,
    skippedMergeCommits: 1,
    skippedDocsOnlyCommits: 1,
    noCommits: false,
  });

  const simEntry = jsonReport.commits.find((commit) => commit.sha === simCommit);
  assert.ok(simEntry, "expected sim commit in report");
  assert.equal(simEntry.status, "considered");
  assert.equal(simEntry.body, "Adds a new public helper that may need docs.");
  assert.deepEqual(simEntry.traceDocs, ["docs/context/server-sim.md", "docs/design/server-sim.md"]);
  assert.equal(simEntry.docsTouched.anyDesign, false);

  const docsOnlyEntry = jsonReport.commits.find((commit) => commit.subject === "Clarify testing docs");
  assert.ok(docsOnlyEntry, "expected docs-only commit in report");
  assert.equal(docsOnlyEntry.status, "skipped");
  assert.equal(docsOnlyEntry.skipReason, "docs_only_churn");
  assert.equal(docsOnlyEntry.docsTouched.anyDesign, true);

  const testEntry = jsonReport.commits.find((commit) => commit.sha === testCommit);
  assert.ok(testEntry, "expected test commit in report");
  assert.deepEqual(testEntry.traceDocs, ["docs/context/testing.md", "docs/design/testing.md"]);

  const mergeEntry = jsonReport.commits.find((commit) => commit.skipReason === "merge_commit");
  assert.ok(mergeEntry, "expected merge commit skip");
  assert.equal(mergeEntry.parentCount, 2);

  const markdown = sweep(["--base", checkpointCommit, "--head", head]);
  assert.match(markdown, /# Documentation Drift Sweep Dry Run/);
  assert.match(markdown, /Considered commits: 2/);
  assert.match(markdown, /Change sim API/);
  assert.match(markdown, /docs\/design\/server-sim\.md/);
  assert.match(markdown, /docs_only_churn/);
  assert.match(markdown, /merge_commit/);

  const outDir = path.join(fixtureRoot, "out");
  sweep(["--base", checkpointCommit, "--head", head, "--out-dir", outDir]);
  assert.ok(fs.existsSync(path.join(outDir, "docdrift-sweep.md")));
  assert.ok(fs.existsSync(path.join(outDir, "docdrift-sweep.json")));
  assert.equal(JSON.parse(fs.readFileSync(path.join(outDir, "docdrift-sweep.json"), "utf8")).head.sha, head);

  const checkpointReport = JSON.parse(sweep(["--head", head, "--format", "json"]));
  assert.equal(checkpointReport.base.ref, base);
  assert.equal(checkpointReport.base.source, "docs/docdrift-checkpoint.txt");
  assert.equal(checkpointReport.summary.totalCommits, 5);
  assert.equal(checkpointReport.summary.consideredCommits, 2);

  const emptyReport = JSON.parse(sweep(["--base", head, "--head", head, "--format", "json"]));
  assert.equal(emptyReport.summary.noCommits, true);
  assert.match(sweep(["--base", head, "--head", head]), /No commits to sweep/);
} finally {
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
}

console.log("docdrift sweeper tests passed");
