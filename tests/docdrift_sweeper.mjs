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

function classify(args) {
  return run("node", [script, "--classify", "--repo", repo, ...args], {
    cwd: repoRoot,
  });
}

function generateDocs(args) {
  return run("node", [script, "--generate-docs", "--repo", repo, ...args], {
    cwd: repoRoot,
  });
}

function fullSweep(args) {
  return run("node", [script, "--full", "--repo", repo, ...args], {
    cwd: repoRoot,
  });
}

function classifyFailure(args) {
  try {
    classify(args);
    assert.fail("expected classifier command to fail");
  } catch (error) {
    return error.stderr.toString();
  }
}

function writeExecutable(name, text) {
  const absPath = path.join(fixtureRoot, name);
  fs.writeFileSync(absPath, text, { mode: 0o755 });
  return absPath;
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
            docs: ["docs/context/server-sim.md", "docs/design/server-sim.md", "docs/design/testing.md"],
            notes: "Sim changes route primarily to server sim docs, with a broad testing-doc trace candidate.",
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
  assert.deepEqual(simEntry.traceDocs, ["docs/context/server-sim.md", "docs/design/server-sim.md", "docs/design/testing.md"]);
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

  const classifierCache = ".docdrift/test-classifier-cache";
  const classifierJson = JSON.parse(
    classify([
      "--base",
      checkpointCommit,
      "--head",
      head,
      "--no-codex",
      "--fixture",
      "classifier-basic",
      "--classifier-cache",
      classifierCache,
      "--format",
      "json",
    ]),
  );
  assert.equal(classifierJson.mode, "classify");
  assert.equal(classifierJson.classifier.promptVersion, "docdrift-classifier-v1");
  assert.equal(classifierJson.classifier.noCodex, true);
  assert.equal(classifierJson.classifier.fixture, "tests/fixtures/docdrift/classifier-basic.json");
  assert.equal(classifierJson.classifier.summary.totalDecisions, 2);
  assert.equal(classifierJson.classifier.summary.updateDocs, 1);
  assert.equal(classifierJson.classifier.summary.moveOn, 1);
  assert.equal(classifierJson.classifier.summary.cacheHits, 0);

  const simDecision = classifierJson.classifier.decisions.find((decision) => decision.commitSha === simCommit);
  assert.ok(simDecision, "expected sim classifier decision");
  assert.equal(simDecision.decision, "update_docs");
  assert.deepEqual(simDecision.likelyDocs, ["docs/design/server-sim.md"]);
  assert.equal(simDecision.codex.mode, "fixture");
  assert.equal(simDecision.cache.hit, false);

  const testDecision = classifierJson.classifier.decisions.find((decision) => decision.commitSha === testCommit);
  assert.ok(testDecision, "expected test classifier decision");
  assert.equal(testDecision.decision, "move_on");
  assert.deepEqual(testDecision.likelyDocs, ["docs/design/testing.md"]);

  const cachedClassifierJson = JSON.parse(
    classify([
      "--base",
      checkpointCommit,
      "--head",
      head,
      "--no-codex",
      "--fixture",
      "classifier-basic",
      "--classifier-cache",
      classifierCache,
      "--format",
      "json",
    ]),
  );
  assert.equal(cachedClassifierJson.classifier.summary.cacheHits, 2);
  assert.ok(cachedClassifierJson.classifier.decisions.every((decision) => decision.cache.hit));

  const classifierOutDir = path.join(fixtureRoot, "classify-out");
  classify([
    "--base",
    checkpointCommit,
    "--head",
    head,
    "--no-codex",
    "--fixture",
    "classifier-basic",
    "--classifier-cache",
    ".docdrift/out-cache",
    "--out-dir",
    classifierOutDir,
  ]);
  assert.ok(fs.existsSync(path.join(classifierOutDir, "docdrift-classify.md")));
  assert.ok(fs.existsSync(path.join(classifierOutDir, "docdrift-classify.json")));

  const docPatchCache = ".docdrift/test-doc-patch-cache";
  const generateJson = JSON.parse(
    generateDocs([
      "--base",
      checkpointCommit,
      "--head",
      head,
      "--no-codex",
      "--fixture",
      "classifier-basic",
      "--classifier-cache",
      classifierCache,
      "--doc-patch-cache",
      docPatchCache,
      "--format",
      "json",
    ]),
  );
  assert.equal(generateJson.mode, "generate-docs");
  assert.equal(generateJson.docPatch.promptVersion, "docdrift-doc-patch-v1");
  assert.equal(generateJson.docPatch.noCodex, true);
  assert.equal(generateJson.docPatch.fixture, "tests/fixtures/docdrift/classifier-basic.json");
  assert.equal(generateJson.docPatch.summary.updateDocsDecisions, 1);
  assert.equal(generateJson.docPatch.summary.patches, 1);
  assert.equal(generateJson.docPatch.summary.applied, 1);
  assert.deepEqual(generateJson.docPatch.records[0].docTargets, ["docs/design/server-sim.md"]);
  assert.equal(generateJson.docPatch.records[0].docTargetSource, "classifier_likely_docs");
  assert.match(
    fs.readFileSync(path.join(repo, "docs/design/server-sim.md"), "utf8"),
    /Game API helper changes should be reflected here/,
  );

  const idempotentGenerateJson = JSON.parse(
    generateDocs([
      "--base",
      checkpointCommit,
      "--head",
      head,
      "--no-codex",
      "--fixture",
      "classifier-basic",
      "--classifier-cache",
      classifierCache,
      "--doc-patch-cache",
      docPatchCache,
      "--format",
      "json",
    ]),
  );
  assert.equal(idempotentGenerateJson.docPatch.summary.applied, 0);
  assert.equal(idempotentGenerateJson.docPatch.summary.alreadyApplied, 1);
  assert.equal(idempotentGenerateJson.docPatch.summary.cacheHits, 1);
  assert.equal(idempotentGenerateJson.docPatch.budget.estimatedPromptTokens, 0);
  assert.equal(idempotentGenerateJson.docPatch.records[0].cache.reason, "already_applied_patch");

  const fakeCodexArgsPath = path.join(fixtureRoot, "fake-codex-args.json");
  const fakeCodex = writeExecutable(
    "fake-codex",
    [
      "#!/usr/bin/env node",
      "const fs = require('node:fs');",
      "const args = process.argv.slice(2);",
      `fs.writeFileSync(${JSON.stringify(fakeCodexArgsPath)}, JSON.stringify(args, null, 2));`,
      "const outputFlag = args.indexOf('--output-last-message');",
      "if (outputFlag < 0 || outputFlag + 1 >= args.length) process.exit(2);",
      "if (args.includes('--ask-for-approval')) process.exit(3);",
      "if (!args.includes('--json')) process.exit(4);",
      "if (!args.includes('-c') || !args.includes('approval_policy=\"never\"')) process.exit(5);",
      "fs.writeFileSync(args[outputFlag + 1], JSON.stringify({ decision: 'move_on', likelyDocs: [], evidenceNote: 'Fake Codex moved this commit on.' }));",
      "console.log(JSON.stringify({ type: 'turn.completed', usage: { input_tokens: 11, output_tokens: 7, total_tokens: 18 } }));",
      "",
    ].join("\n"),
  );
  const liveShapeJson = JSON.parse(
    classify([
      "--base",
      checkpointCommit,
      "--head",
      simCommit,
      "--codex-command",
      fakeCodex,
      "--classifier-cache",
      ".docdrift/fake-codex-cache",
      "--format",
      "json",
    ]),
  );
  assert.equal(liveShapeJson.classifier.summary.totalDecisions, 1);
  assert.equal(liveShapeJson.classifier.decisions[0].codex.mode, "codex_cli");
  assert.deepEqual(liveShapeJson.classifier.decisions[0].codex.usage, {
    inputTokens: 11,
    cachedInputTokens: null,
    outputTokens: 7,
    reasoningTokens: null,
    totalTokens: 18,
  });
  const fakeArgs = JSON.parse(fs.readFileSync(fakeCodexArgsPath, "utf8"));
  assert.ok(!fakeArgs.includes("--ask-for-approval"));
  assert.ok(fakeArgs.includes("--json"));
  assert.ok(fakeArgs.includes("-c"));
  assert.ok(fakeArgs.includes('approval_policy="never"'));

  const generateOutDir = path.join(fixtureRoot, "generate-out");
  generateDocs([
    "--base",
    checkpointCommit,
    "--head",
    head,
    "--no-codex",
    "--fixture",
    "classifier-basic",
    "--classifier-cache",
    classifierCache,
    "--doc-patch-cache",
    docPatchCache,
    "--out-dir",
    generateOutDir,
  ]);
  assert.ok(fs.existsSync(path.join(generateOutDir, "docdrift-generate.md")));
  assert.ok(fs.existsSync(path.join(generateOutDir, "docdrift-generate.json")));

  assert.match(
    classifyFailure([
      "--base",
      checkpointCommit,
      "--head",
      head,
      "--no-codex",
      "--fixture",
      "classifier-basic",
      "--max-commits",
      "1",
    ]),
    /classify budget exceeded: 2 considered commits exceeds --max-commits 1/,
  );

  const checkpointReport = JSON.parse(sweep(["--head", head, "--format", "json"]));
  assert.equal(checkpointReport.base.ref, base);
  assert.equal(checkpointReport.base.source, "docs/docdrift-checkpoint.txt");
  assert.equal(checkpointReport.summary.totalCommits, 5);
  assert.equal(checkpointReport.summary.consideredCommits, 2);

  const emptyReport = JSON.parse(sweep(["--base", head, "--head", head, "--format", "json"]));
  assert.equal(emptyReport.summary.noCommits, true);
  assert.match(sweep(["--base", head, "--head", head]), /No commits to sweep/);

  const fullPreviewOutDir = path.join(fixtureRoot, "full-preview");
  const fullPreview = JSON.parse(
    fullSweep([
      "--dry-run",
      "--base",
      head,
      "--head",
      head,
      "--out-dir",
      fullPreviewOutDir,
      "--run-id",
      "preview",
      "--format",
      "json",
    ]),
  );
  assert.equal(fullPreview.mode, "full");
  assert.equal(fullPreview.dryRun, true);
  assert.equal(fullPreview.sweep.action, "noop_no_commits");
  assert.equal(fullPreview.checkpoint.advanced, false);
  assert.ok(fullPreview.lifecycle.some((step) => step.name === "open or update owned PR"));
  assert.ok(fullPreview.lifecycle.some((step) => step.command.includes("scripts/wait-pr.sh")));
  assert.ok(fs.existsSync(path.join(fullPreviewOutDir, "docdrift-full.json")));
  assert.ok(fs.existsSync(path.join(fullPreviewOutDir, "docdrift-full.md")));

  const bareOrigin = path.join(fixtureRoot, "origin.git");
  run("git", ["init", "--bare", bareOrigin], { cwd: fixtureRoot });
  git(["remote", "add", "origin", bareOrigin]);
  git(["push", "-u", "origin", "main"]);
  const docsOnlyHead = commitFile(
    "docs/design/testing.md",
    "testing design\nmore detail\ncheckpoint-only cleanup\n",
    "Refresh testing docs again",
    "",
    { date: "2026-06-20T12:05:00Z" },
  );
  git(["push", "origin", "main"]);

  const fullNoopOutDir = path.join(fixtureRoot, "full-noop");
  const fullNoop = JSON.parse(
    fullSweep([
      "--base",
      head,
      "--head",
      "origin/main",
      "--out-dir",
      fullNoopOutDir,
      "--run-id",
      "noop",
      "--format",
      "json",
    ]),
  );
  assert.equal(fullNoop.mode, "full");
  assert.equal(fullNoop.dryRun, false);
  assert.equal(fullNoop.sweep.action, "noop_no_considered_commits");
  assert.equal(fullNoop.sweep.prNumber, null);
  assert.equal(fullNoop.checkpoint.advanced, true);
  assert.equal(fullNoop.checkpoint.after.sha, docsOnlyHead);
  assert.match(fs.readFileSync(path.join(repo, ".docdrift/checkpoint.txt"), "utf8"), new RegExp(docsOnlyHead));
  assert.ok(fs.existsSync(path.join(fullNoopOutDir, "docdrift-full.json")));
} finally {
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
}

console.log("docdrift sweeper tests passed");
